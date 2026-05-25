# Part 4: Auto-Tiling — From Hardcoded Indices to Data-Driven Visuals

In Part 3 we rendered a tilemap efficiently, but the visual tile for each cell was still hardcoded. A `match` expression examined grid coordinates and manually assigned atlas indices. In this part we replace that with an **auto-tiling system**: the map stores only *logical* tile types (`Grass`, `Path`), and a set of *rules* decides which sprite to draw based on each tile's neighbors.

This separation is powerful. It means you can change the map layout without touching visual code, and you can change the art style without touching map logic.

---

## Why auto-tiling matters

Consider two ways to build a road:

**Hardcoded indices (what we had):**
```rust
let tile_index = match (row, col) {
    (5, 0) => 102,   // left edge
    (5, 14) => 104,  // right edge
    (5, _) => 103,   // body
    ...
};
```

If you widen the road, add a curve, or swap the tileset, you rewrite the `match`. Every visual decision is baked into coordinate logic.

**Auto-tiling (what we build):**
```rust
let tile_index = rules.resolve(TileType::Path, pos, &map);
```

The map says "this cell is a path." The rule book says "a path with grass to the north and path to the south is a top edge tile." The renderer never sees the logic — it just receives the final index.

This mirrors how professional tools work: Tiled, LDTK, and custom editors all store logical layers and resolve visuals through rule sets.

---

## The design at a glance

We introduce three layers:

| Layer | Type | Purpose |
|---|---|---|
| **Map data** | `MapLayout` | A grid of `TileType` values — the ground truth |
| **Rules** | `TileRules` | For each `TileType`, an ordered list of neighbor patterns → atlas index |
| **Visuals** | `TilemapBundle` | The rendered output, produced by evaluating rules at spawn time |

The flow is:

```
MapLayout (Grass/Path) → TileRules.resolve(pos) → TileTextureIndex → GPU
```

---

## Core types

### `TileType` — The logical vocabulary

```rust
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Hash)]
enum TileType {
    Grass,
    Path,
}
```

`TileType` is deliberately small. It describes *what* a tile is, not *how it looks*. Every tile entity carries this as a component so gameplay systems can query it: towers can only be placed on `Grass`, enemies walk only on `Path`.

### `MapLayout` — The authoritative grid

```rust
#[derive(Resource)]
struct MapLayout {
    width: u32,
    height: u32,
    tiles: Vec<TileType>,
}
```

`MapLayout` is a `Resource`, not a component on each tile. This is important: it is a single flat array that represents the entire level design. In Part 5 it will become the deserialization target when we load maps from files. Tile entities are just a *view* of this data.

### `NeighborMatch` — Relative requirements

```rust
enum NeighborMatch {
    Same,       // neighbor must match center tile's type
    Different,  // neighbor must differ (or be out of bounds)
    Any,        // no requirement
}
```

Notice that `Same` and `Different` are **relative** to the center tile, not absolute types. A `Same` check for a `Path` tile succeeds if the neighbor is also `Path`. The same rule, applied to a `Grass` tile, succeeds if the neighbor is `Grass`. This makes rules **reusable across tile types**.

### `NeighborPattern` — 8-neighbor context

```rust
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
```

We define patterns for all 8 neighbors even though our current road only needs cardinals and a few diagonals. The structure is future-proof: adding T-junctions, curves, or blob-style transitions later only requires new rules, not new types.

### `TileRule` and `TileTypeRuleset`

```rust
struct TileRule {
    pattern: NeighborPattern,
    atlas_index: u32,
}

struct TileTypeRuleset {
    rules: Vec<TileRule>,
    fallback: u32,
}
```

Rules are checked **in order**; the first match wins. This is why corners are listed before edges, and edges before the body. The `fallback` index guarantees that even an incomplete rule set produces a valid tile instead of crashing.

### `TileRules` — The rule book

```rust
#[derive(Resource, Default)]
struct TileRules {
    rulesets: HashMap<TileType, TileTypeRuleset>,
}
```

The top-level collection maps each `TileType` to its `TileTypeRuleset`. Resolving a tile is a single lookup and a linear scan through (typically) fewer than a dozen rules.

---

## The rule book for our road

Our 3-tile-high horizontal road needs these visual variants:

```
┌─────────────────────────────┐
│  ↑    ↑    ↑    ↑    ↑    ↑ │  top edge    (index 80)
│ ←            ...           →│  body/edges  (102, 103, 104)
│  ↓    ↓    ↓    ↓    ↓    ↓ │  bottom edge (126)
└─────────────────────────────┘
     ↖ corners ↗  (79, 81)
     ↙ corners ↘  (125, 127)
```

The rules for `TileType::Path` encode these positions as neighbor patterns:

```rust
let same = NeighborMatch::Same;
let diff = NeighborMatch::Different;
let any = NeighborMatch::Any;

rules.add(TileType::Path, TileTypeRuleset {
    rules: vec![
        // Corners: one cardinal neighbor is Same, the other is Different
        rule(pat(diff, same, same, diff, any, diff, any, any), 79),   // upper-left
        rule(pat(diff, same, diff, same, diff, any, any, any), 81),   // upper-right
        rule(pat(same, diff, same, diff, any, any, any, diff), 125),  // bottom-left
        rule(pat(same, diff, diff, same, any, any, diff, any), 127),  // bottom-right
        
        // Edges: one cardinal neighbor is Different, the rest are Same or Any
        rule(pat(diff, same, same, same, any, any, any, any), 80),    // top
        rule(pat(same, diff, same, same, any, any, any, any), 126),   // bottom
        rule(pat(same, same, same, diff, any, any, any, any), 102),   // left
        rule(pat(same, same, diff, same, any, any, any, any), 104),   // right
    ],
    fallback: 103,  // body
});
```

**Why do edges require `same` for the parallel cardinals?**

In a 3-tile-high road, the top edge tile has path tiles to its south, east, and west. The rule `pat(diff, same, same, same, ...)` captures this: north is grass (`diff`), everything else along the road is path (`same`).

**Why are diagonals mostly `Any`?**

For a straight horizontal road, diagonal neighbors do not affect the visual. We include them in the pattern type for future expansion — T-junctions and curves will need diagonal checks.

---

## How resolution works

```rust
fn resolve(&self, tile_type: TileType, pos: TilePos, map: &MapLayout) -> u32 {
    self.rulesets
        .get(&tile_type)
        .map(|rs| rs.resolve(tile_type, pos, map))
        .unwrap_or_else(|| panic!("No ruleset for {:?}", tile_type))
}
```

For a `Path` tile at position `(1, 6)` in a 3-tile road:

1. Fetch the `Path` ruleset.
2. Check corner rules first. Does north=`diff` and west=`diff`? No — west is path.
3. Check edge rules. Does north=`diff` and south=`same`? Yes. Return `80` (top edge).

Boundary tiles (x=0, x=14) are handled naturally: `map.get()` returns `None` for out-of-bounds, which `NeighborMatch::Different` treats as "not the same type." So a path tile at the map edge correctly matches a left/right edge rule.

---

## Simplifications we made

This system is intentionally simpler than a production auto-tiler. Here is what we left out, why it is fine for now, and how you would extend it:

### 1. Static resolution at spawn time

```rust
// In setup(), once:
let visual_index = rules.resolve(tile_type, pos, &map);
```

**Assumption:** The map does not change after the game starts. No tiles are added, removed, or retyped at runtime.

**Why this is fine:** For a tower defense game with fixed maps, the level geometry is immutable. Towers sit on top of tiles; they do not replace them.

**How to extend:** If you need dynamic terrain (digging, building, destruction), add a system that listens for `TileType` changes and re-runs resolution:

```rust
fn update_tile_visuals(
    mut tiles: Query<(&TileType, &TilePos, &mut TileTextureIndex)>,
    map: Res<MapLayout>,
    rules: Res<TileRules>,
) {
    for (tile_type, pos, mut index) in &mut tiles {
        let new_index = rules.resolve(*tile_type, *pos, &map);
        index.0 = new_index;
    }
}
```

You would run this in `Update` or trigger it selectively via events to avoid per-frame overhead.

### 2. Single texture atlas

All rules produce indices into the same `towerDefense_tilesheet.png`.

**Assumption:** Every tile type variant lives in one atlas.

**Why this is fine:** The Kenney pack is already a unified atlas. `bevy_ecs_tilemap` draws one tilemap layer with one texture.

**How to extend:** For multi-atlas tilesets, `bevy_ecs_tilemap` supports `TilemapTexture::Vector(handles)` and `TileTextureIndex(texture_index, atlas_index)`. You would store a `(texture_id, atlas_index)` pair in each `TileRule` instead of a single `u32`.

### 3. Relative neighbor matching (`Same`/`Different`)

Rules check "is the neighbor the same type as me?" not "is the neighbor specifically `Grass`?"

**Assumption:** Edge transitions are uniform. A path edge always looks the same regardless of whether it borders grass, sand, or water.

**Why this is fine:** For our art style, the path-to-grass edge sprite works as a generic boundary.

**How to extend:** Add an `Is(TileType)` variant to `NeighborMatch`:

```rust
enum NeighborMatch {
    Same,
    Different,
    Is(TileType),  // new
    Any,
}
```

Then a water tile could have distinct edge sprites for water-to-grass vs water-to-path.

### 4. No bitmask compression

We store 8 `NeighborMatch` values per pattern and check them with boolean logic. A production system might compress this into an 8-bit bitmask for performance.

**Assumption:** Rule sets are small (fewer than 20 rules per type).

**Why this is fine:** A linear scan of 10 rules is microseconds of work, done once per tile at spawn time. For a `100×100` map that is 10,000 checks — negligible compared to asset loading and GPU upload.

**How to extend:** Precompute a `u8` bitmask from the 8 neighbors and use it as a key into a lookup table. This is the standard "blob tile" approach and is well-documented in game programming literature.

### 5. No runtime rule reloading

Rules are compiled Rust code in `build_rules()`.

**Assumption:** Tile art and transitions are fixed for the game's lifespan.

**How to extend:** Serialize rules to JSON/TOML and load them as assets. Then artists can tweak transitions without recompiling the game.

---

## Alternative: `bevy_map_editor`

The ecosystem around Bevy tilemaps is growing. One project worth watching is [`bevy_map_editor`](https://github.com/jbuehler23/bevy_map_editor), advertised as a "complete 2D tilemap editor and runtime for Bevy games." It includes built-in auto-tiling, an editor UI for painting tiles, and serialization support.

It is relatively new and not yet battle-tested, so we built our own auto-tiling system in this tutorial to understand the fundamentals. However, if you are building a game that needs a visual editor, rapid level iteration, or advanced tilemap features out of the box, `bevy_map_editor` may save you significant work. Evaluate it against your project's needs — especially stability and long-term maintenance — before adopting.

---

## Running the project

```bash
cargo run
```

You should see:

- A centered `15×10` grid.
- A 3-tile-high horizontal road across the middle.
- Corners, edges, and body tiles all correctly resolved from the rule book — no hardcoded coordinates.
- Grass filling every other cell via the `Grass` ruleset's fallback.

Try editing `build_demo_map()` to change the road width or position. The visuals update automatically because the rules inspect neighbors rather than coordinates.

---

## Recap

In this part we:

1. Separated **logical tile types** (`TileType`) from **visual atlas indices**.
2. Built an **auto-tiling engine** with `NeighborPattern`, `TileRule`, and `TileTypeRuleset`.
3. Used **relative neighbor matching** (`Same`/`Different`) to keep rules generic and reusable.
4. Defined an **ordered rule set** where specific patterns (corners) match before general ones (edges, body).
5. Stored `MapLayout` and `TileRules` as ECS resources for future systems.
6. Explicitly discussed the **simplifications** we made, the assumptions behind them, and how to evolve the system.

In **Part 5** we will extract `MapLayout` from code into an external data file, completing the separation of level design from game logic.
