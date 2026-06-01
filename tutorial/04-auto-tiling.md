# Part 4: Auto-Tiling — From Hardcoded Indices to Data-Driven Visuals

> **Time to read:** ~25 minutes  
> **New concepts:** Auto-tiling  
> **Prerequisite:** Part 3 (a `bevy_ecs_tilemap` grid rendered with hardcoded atlas indices)

---

## Recap: What We Already Have

We have a `15×10` tilemap rendered efficiently through `bevy_ecs_tilemap`. The map is built inside `setup()` by a `match` expression that examines grid coordinates and manually assigns atlas indices. Every tile carries a `MapTile` marker; road tiles additionally carry `PathTile`.

---

## Goal: What We Will Build

We will replace the hardcoded `match` with an **auto-tiling system**:

1. Introduce logical tile types (`Grass`, `Path`) stored in a `MapLayout` resource.
2. Build a rule book (`TileRules`) that maps neighbor patterns to atlas indices.
3. Resolve visuals at spawn time by inspecting each tile's neighbors instead of its coordinates.

This separation means you can change the road's position or width by editing the map data, and the visuals will adapt automatically. It also means you can swap the tileset without touching map logic.

---

## New Bevy APIs & Concepts

### Auto-Tiling

**Auto-tiling** is the practice of deriving a tile's visual appearance from its logical type and its neighbors, rather than assigning visuals directly during level design. You store only *what* each tile is (`Grass`, `Path`, `Water`), and a rule book decides *which sprite* to draw based on context: a path tile surrounded by grass on three sides becomes a corner; the same path tile surrounded by path on all sides becomes a body tile.

This separation is common in professional tools (Tiled, LDTK) and production games because it lets designers paint logical maps without micromanaging every atlas index.

**Pitfall:** Auto-tiling only works when the rule book is complete. If you add a new map shape (a T-junction, a diagonal road) but forget to add the corresponding rule, the tile will fall back to a generic body sprite and look wrong.

---

## Walkthrough

### Designing the feature

Before we write code, let's design the interface our auto-tiling system will expose. Since auto-tiling is a developer-facing feature, "what the player sees" is unchanged — the same centered `15×10` grid with a dirt road. What we are designing is the **contract** between the map data and the renderer.

Our system has three layers:

| Layer | Type | Purpose |
|---|---|---|
| **Map data** | `MapLayout` | A grid of `TileType` values — the ground truth |
| **Rules** | `TileRules` | For each `TileType`, an ordered list of neighbor patterns → atlas index |
| **Visuals** | `TilemapBundle` | The rendered output, produced by evaluating rules at spawn time |

The flow is:

```
MapLayout (Grass/Path) → TileRules.resolve(pos) → TileTextureIndex → GPU
```

**Design choices we are making:**

1. **8-directional neighbors.** We check all 8 surrounding cells (cardinals + diagonals). This is more expressive than 4-directional checking and matches the Unity tilemap rule-tile format, though we will only use a subset for our straight road.

2. **Relative matching (`Same` / `Different` / `Any`).** Rules ask "is the neighbor the same type as me?" rather than "is the neighbor specifically `Grass`?" This keeps rules reusable across tile types. We deliberately omit an `Is(TileType)` variant to keep the system simple.

3. **No rotation or flipping.** Rules map one pattern to one atlas index. In a full editor you might rotate a rule to reuse a corner sprite in all four orientations; here we write four explicit corner rules.

4. **Ordered evaluation, first match wins.** This is the standard approach in Unity rule tiles and Tiled automapping: list the most specific patterns first (corners), then general ones (edges), then a fallback (body).

---

### Step 1: Define logical tile types

Open `src/main.rs` and add `TileType`, our logical vocabulary:

```rust
use std::collections::HashMap;

/// Logical tile types. Gameplay systems query these, not visual atlas indices.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Hash)]
enum TileType {
    Grass,
    Path,
}
```

`TileType` is deliberately small. It describes *what* a tile is, not *how it looks*. Every tile entity will carry this as a component so gameplay systems can query it: towers can only be placed on `Grass`, enemies walk only on `Path`.

We also keep `MapTile` and `PathTile` from Part 3. `MapTile` tags every tile; `PathTile` tags only road tiles.

---

### Step 2: Build the map layout

`MapLayout` is a `Resource` (a global singleton in the ECS world, like `AssetServer` from Part 2) that stores the entire level design in a single flat array. Tile entities are just a *view* of this data.

```rust
#[derive(Resource)]
struct MapLayout {
    width: u32,
    height: u32,
    tiles: Vec<TileType>,
}

impl MapLayout {
    fn get(&self, x: u32, y: u32) -> Option<TileType> {
        if x < self.width && y < self.height {
            Some(self.tiles[(y * self.width + x) as usize])
        } else {
            None
        }
    }
}
```

`get` converts 2D coordinates into a 1D index. It returns `None` for out-of-bounds positions, which our auto-tiler will treat as "not the same type" — this naturally handles map edges.

Now add a helper that builds the demo map:

```rust
fn build_demo_map() -> MapLayout {
    let width: u32 = 15;
    let height: u32 = 10;
    let path_y = height / 2;

    let mut tiles = vec![TileType::Grass; (width * height) as usize];

    let has_top = path_y > 0;
    let has_bot = path_y < height - 1;
    let w = width as usize;

    for x in 0..width {
        let idx = (path_y * width + x) as usize;
        tiles[idx] = TileType::Path;
        if has_top { tiles[idx - w] = TileType::Path; }
        if has_bot { tiles[idx + w] = TileType::Path; }
    }

    MapLayout { width, height, tiles }
}
```

This produces a 3-tile-high horizontal strip of `Path` centered in a field of `Grass`. Notice that no atlas indices appear anywhere — the map is purely logical.

---

### Step 3: Build the auto-tiling rule book

This is the heart of the system. We need four types:

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NeighborMatch {
    Same,       // neighbor must match center tile's type
    Different,  // neighbor must differ (or be out of bounds)
    Any,        // no requirement
}

#[derive(Default, Clone)]
struct NeighborPattern {
    north: NeighborMatch,
    south: NeighborMatch,
    east: NeighborMatch,
    west: NeighborMatch,
    north_east: NeighborMatch,
    north_west: NeighborMatch,
    south_east: NeighborMatch,
    south_west: NeighborMatch,
}

#[derive(Clone)]
struct TileRule {
    pattern: NeighborPattern,
    atlas_index: u32,
}

#[derive(Clone)]
struct TileTypeRuleset {
    rules: Vec<TileRule>,
    fallback: u32,
}
```

`NeighborMatch::Same` and `Different` are **relative** to the center tile. A `Same` check for a `Path` tile succeeds if the neighbor is also `Path`. The same rule applied to a `Grass` tile succeeds if the neighbor is `Grass`. This makes rules reusable across tile types.

We define patterns for all 8 neighbors even though our current road only needs cardinals and a few diagonals. The structure is future-proof: adding T-junctions or curves later only requires new rules, not new types.

Add two small helpers that make the rule book readable:

```rust
fn pat(
    n: NeighborMatch, s: NeighborMatch, e: NeighborMatch, w: NeighborMatch,
    ne: NeighborMatch, nw: NeighborMatch, se: NeighborMatch, sw: NeighborMatch,
) -> NeighborPattern {
    NeighborPattern { north: n, south: s, east: e, west: w,
        north_east: ne, north_west: nw, south_east: se, south_west: sw }
}

fn rule(pattern: NeighborPattern, atlas_index: u32) -> TileRule {
    TileRule { pattern, atlas_index }
}
```

Now build the rule book:

```rust
#[derive(Resource, Default)]
struct TileRules {
    rulesets: HashMap<TileType, TileTypeRuleset>,
}

impl TileRules {
    fn add(&mut self, tile_type: TileType, ruleset: TileTypeRuleset) {
        self.rulesets.insert(tile_type, ruleset);
    }

    fn resolve(&self, tile_type: TileType, pos: TilePos, map: &MapLayout) -> u32 {
        self.rulesets
            .get(&tile_type)
            .map(|rs| rs.resolve(tile_type, pos, map))
            .unwrap_or_else(|| panic!("No ruleset for {:?}", tile_type))
    }
}

fn build_rules() -> TileRules {
    let mut rules = TileRules::default();

    let same = NeighborMatch::Same;
    let diff = NeighborMatch::Different;
    let any  = NeighborMatch::Any;

    rules.add(
        TileType::Path,
        TileTypeRuleset {
            rules: vec![
                // Corners: grass on two sides, path on the other two.
                rule(pat(diff, same, same, diff, any, diff, any, any), 79),   // upper-left
                rule(pat(diff, same, diff, same, diff, any, any, any), 81),   // upper-right
                rule(pat(same, diff, same, diff, any, any, any, diff), 125),  // bottom-left
                rule(pat(same, diff, diff, same, any, any, diff, any), 127),  // bottom-right
                // Edges: grass on one side, path on the other three.
                rule(pat(diff, same, same, same, any, any, any, any), 80),    // top
                rule(pat(same, diff, same, same, any, any, any, any), 126),   // bottom
                rule(pat(same, same, same, diff, any, any, any, any), 102),   // left
                rule(pat(same, same, diff, same, any, any, any, any), 104),   // right
            ],
            fallback: 103,  // body
        },
    );

    rules.add(
        TileType::Grass,
        TileTypeRuleset {
            rules: vec![],
            fallback: 129,
        },
    );

    rules
}
```

Rules are checked in order; the first match wins. Corners are listed before edges, and edges before the body fallback. The `fallback` guarantees that even an incomplete rule set produces a valid tile.

**Why do edges require `same` for the parallel cardinals?**

In a 3-tile-high road, the top edge tile has path tiles to its south, east, and west. The rule `pat(diff, same, same, same, ...)` captures this: north is grass (`diff`), everything else along the road is path (`same`).

**Why are diagonals mostly `Any`?**

For a straight horizontal road, diagonal neighbors do not affect the visual. We include them in the pattern type for future expansion.

**Our rule book is small on purpose.** A complete 8-neighbor "blob tileset" contains 47 unique transition tiles, covering every possible neighbor combination. Our Kenney pack provides only a handful of road variants — corners, edges, and a body tile — so our rules match exactly what the art allows. If you swap in a larger tileset with more transitions (T-junctions, inner corners, end caps), you can add more rules inside `build_rules()`. Remember that order still matters: a new specific rule may need to sit above an existing general one to match first.

---

### Step 4: Resolve visuals and spawn tiles

Replace the body of `setup()` with the new approach. The flow is the same as Part 3 — create a tilemap parent, loop over the grid, spawn each tile, and finalize with a `TilemapBundle` — but the spawn loop now uses our auto-tiling rule book instead of a hardcoded `match`.

This system uses two parameters:

- `mut commands: Commands` — to spawn the camera, tilemap parent, and tile entities.
- `asset_server: Res<AssetServer>` — to load the tilesheet texture.

The spawn loop works in four steps:

1. **Read the map.** For each grid coordinate `(x, y)`, look up the logical `tile_type` from `MapLayout`.
2. **Resolve the visual.** Ask `TileRules` which atlas index matches this tile's neighbor pattern. This replaces the Part 3 `match` on coordinates.
3. **Spawn the tile.** Create a `TileBundle` with the resolved `TileTextureIndex`, exactly as we did in Part 3. Attach `tile_type` as a component so gameplay systems can query it, plus `MapTile` on every tile and `PathTile` only on road tiles.
4. **Register in storage.** Call `tile_storage.set(&pos, tile_entity)` so the tilemap parent knows about the new tile.

After the loop, insert the completed `TilemapBundle` on the parent entity. Finally, insert both `map` and `rules` as ECS resources so later systems can read them.

```rust
fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let map = build_demo_map();
    let rules = build_rules();

    commands.spawn(Camera2d);

    let texture_handle = asset_server.load("Tilesheet/towerDefense_tilesheet.png");

    let map_size = TilemapSize { x: map.width, y: map.height };
    let tile_size = TilemapTileSize { x: 64.0, y: 64.0 };
    let grid_size = tile_size.into();
    let map_type = TilemapType::Square;

    let tilemap_entity = commands.spawn_empty().id();
    let mut tile_storage = TileStorage::empty(map_size);

    for x in 0..map.width {
        for y in 0..map.height {
            let pos = TilePos { x, y };
            let tile_type = map.get(x, y).unwrap();

            // Resolve the visual atlas index from the rule book.
            let visual_index = rules.resolve(tile_type, pos, &map);

            let tile_entity = commands
                .spawn((
                    TileBundle {
                        position: pos,
                        tilemap_id: TilemapId(tilemap_entity),
                        texture_index: TileTextureIndex(visual_index),
                        ..Default::default()
                    },
                    tile_type,
                    MapTile,
                ))
                .id();

            if tile_type == TileType::Path {
                commands.entity(tile_entity).insert(PathTile);
            }

            tile_storage.set(&pos, tile_entity);
        }
    }

    commands.entity(tilemap_entity).insert(TilemapBundle {
        grid_size,
        map_type,
        size: map_size,
        storage: tile_storage,
        texture: TilemapTexture::Single(texture_handle),
        tile_size,
        anchor: TilemapAnchor::Center,
        ..Default::default()
    });

    // Make map and rules available to future systems.
    commands.insert_resource(map);
    commands.insert_resource(rules);
}
```

> **Run the game now.** The visual output should be identical to Part 3: a centered `15×10` grid with a horizontal dirt road across the middle. Try editing `build_demo_map()` — change `path_y` or add more path rows — and the visuals will update automatically because the rules inspect neighbors rather than coordinates.

---

### Simplification: Static resolution at spawn time

For now, tile visuals are resolved once inside `setup()` and never changed again. That is fine because our tower defense maps are immutable: towers sit on top of tiles but do not replace them.

In a game with dynamic terrain (digging, building, destruction), you would add a system that re-runs resolution whenever a `TileType` changes. Such a system would iterate over tiles with changed `TileType` components, call `rules.resolve()` again, and write the new index into `TileTextureIndex`.

---

## What stayed the same

- The atlas indices (`79`, `80`, `81`, `102`, `103`, `104`, `125`, `126`, `127`, `129`) are unchanged.
- `MapTile` and `PathTile` marker components are preserved on tile entities.
- The tilemap parent entity, `TileStorage`, and `TilemapBundle` are identical to Part 3.

---

## What we gained

| Aspect | Part 3 (hardcoded) | Part 4 (auto-tiling) |
|---|---|---|
| Map definition | `match` on coordinates | `MapLayout` resource with `TileType` grid |
| Visual logic | Baked into spawn code | Separated into `TileRules` resource |
| Changing road position | Edit `match` arms | Edit `build_demo_map()` |
| Changing art style | Edit `match` arms | Edit `TileRules` |
| Reusability | None | Same rules work for any `Path` shape |

---

## Summary

- We separated **logical tile types** (`TileType`) from **visual atlas indices**.
- We built an **auto-tiling engine** with `NeighborPattern`, `TileRule`, and `TileTypeRuleset`.
- We used **relative neighbor matching** (`Same`/`Different`) to keep rules generic and reusable.
- We stored `MapLayout` and `TileRules` as ECS `Resource`s for future systems.
- We preserved `MapTile` and `PathTile` so existing gameplay queries continue to work.

In **Part 5** we will refactor the growing `main.rs` into focused modules (`map.rs` and `tiling.rs`), so each file has a single responsibility before we add more features.
