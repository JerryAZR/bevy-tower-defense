# Part 3: Tilemap Rendering — From Sprites to a Dedicated Plugin

> **Time to read:** ~20 minutes  
> **New concepts:** `TilemapPlugin`, `TilemapBundle`, `TileBundle`, `TileStorage`, `TileTextureIndex`, `TilePos`, `TilemapId`, `ImagePlugin::default_nearest()`  
> **Prerequisite:** Part 2 (a grid of atlas sprites with `MapTile` and `PathTile` markers)

---

## Recap: What We Already Have

We have a `15×10` grid of tiles rendered as 150 individual sprite entities. Every tile carries a `Sprite`, `Transform`, and a `MapTile` marker component. Road tiles additionally have a `PathTile` marker. The map is centered on screen and the camera looks at the world origin.

---

## Goal: What We Will Build

We will replace the 150 individual sprite spawns with a single `bevy_ecs_tilemap` tilemap. The visual result will be identical — a centered grid with a horizontal dirt road — but the rendering will happen in one GPU draw call instead of ~150. Along the way we will learn:

- How a community plugin integrates into Bevy's plugin system.
- How `bevy_ecs_tilemap` stores tile data in GPU-friendly buffers while keeping tiles queryable ECS entities.
- Why nearest-neighbor filtering matters for pixel art.

This sets up the rendering foundation for larger maps and external level data in later parts.

---

## New Bevy APIs & Concepts

### `TilemapPlugin`

`TilemapPlugin` is a community plugin (from `bevy_ecs_tilemap`) that registers a specialized render pipeline for tilemaps. Bevy has no built-in tilemap renderer, so this plugin fills the gap. It provides shaders, GPU instancing, chunked culling, and support for square, hexagonal, and isometric grids.

**Pitfall:** `bevy_ecs_tilemap` is deliberately **rendering-only**. It does not load Tiled or LDTK files, it does not auto-tile, and it does not implement pathfinding. You bring your own map logic and hand the results to the plugin for display.

### `TilemapBundle` and `TileBundle`

`TilemapBundle` is a bundle attached to a **single parent entity** that describes the entire grid: its dimensions, texture, tile size, and a `TileStorage` lookup table. It is what makes the grid visible to the renderer.

`TileBundle` is attached to **each individual tile entity** and carries:
- `TilePos` — the tile's grid coordinate.
- `TilemapId` — a handle pointing back to the parent tilemap entity.
- `TileTextureIndex` — which cell of the atlas to draw.

**Pitfall:** You must spawn the parent entity first, then spawn all tile children, then insert `TilemapBundle` on the parent. The order matters because each `TileBundle` needs a valid `TilemapId`.

### `TileStorage`

`TileStorage` is a dense grid that maps `TilePos` to `Entity` IDs. It acts as the bridge between the tilemap parent and its children. The renderer uses it to know which tiles exist, and your systems can use it to look up neighbors or modify specific cells at runtime.

### `ImagePlugin::default_nearest()`

`ImagePlugin::default_nearest()` configures Bevy to use **nearest-neighbor filtering** when scaling textures. The default is bilinear filtering, which blends adjacent pixels and blurs crisp pixel art. Nearest-neighbor keeps edges sharp.

**Pitfall:** Without this, your `64×64` tiles will look blurry at non-integer zoom levels or when the window is resized.

---

## Walkthrough

### Designing the feature

Before we change any code, let's define what the player should see: **nothing different**. The map should still show a centered `15×10` grid with a horizontal dirt road across the middle. The change is entirely under the hood.

What changes on the data side:
1. We stop spawning `Sprite` + `Transform` bundles for each tile.
2. We spawn one parent entity with a `TilemapBundle`.
3. We spawn each tile as a child entity with a `TileBundle`, preserving our `MapTile` and `PathTile` markers.
4. The plugin renders the entire grid as a single mesh.

---

### Step 1: Add the dependency

```bash
cargo add bevy_ecs_tilemap
```

The crate version tracks Bevy's release cycle. Bevy 0.18 pairs with `bevy_ecs_tilemap` 0.18.

---

### Step 2: Register the plugin and configure texture filtering

Open `src/main.rs` and add the plugin import:

```rust
use bevy_ecs_tilemap::prelude::*;
```

Then update `main()` to register `TilemapPlugin` and configure `ImagePlugin`:

```rust
fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins(TilemapPlugin)
        .add_systems(Startup, setup)
        .run();
}
```

`TilemapPlugin` registers the render pipeline, shaders, and extraction systems that make tilemaps visible. Without it, any `TilemapBundle` you spawn will exist in the ECS world but never reach the screen.

`ImagePlugin::default_nearest()` tells Bevy to use nearest-neighbor filtering. This is the correct choice for pixel-art tilesets like ours; without it, the GPU's default bilinear filtering would blur the crisp `64×64` tiles.

> **Run the game now.** The window should still show the Part 2 grid because we haven't rewritten `setup` yet. If the window is black, check that `TilemapPlugin` is registered and the import is present.

---

### Step 3: Prepare the tilemap parent and storage

Replace the body of `setup` with the tilemap approach. The system still needs `Commands` and `AssetServer`, but no longer needs `ResMut<Assets<TextureAtlasLayout>>` — `bevy_ecs_tilemap` handles atlas slicing internally.

This system uses two parameters:
- `mut commands: Commands` — to spawn the tilemap parent and tile entities.
- `asset_server: Res<AssetServer>` — to load the tilesheet texture.

```rust
fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2d);

    let texture_handle = asset_server.load("Tilesheet/towerDefense_tilesheet.png");

    let map_size = TilemapSize { x: 15, y: 10 };
    let tile_size = TilemapTileSize { x: 64.0, y: 64.0 };
    let grid_size = tile_size.into();
    let map_type = TilemapType::Square;

    let tilemap_entity = commands.spawn_empty().id();
    let mut tile_storage = TileStorage::empty(map_size);
```

`TileStorage::empty(map_size)` creates a dense lookup table with one slot per grid cell. We will fill it as we spawn tiles.

---

### Step 4: Spawn tiles with `TileBundle`

The tile selection logic is identical to Part 2. We loop over every cell, decide which atlas index to use, and spawn a tile entity. The only difference is the spawning API: `TileBundle` replaces the `(Sprite, Transform)` pair.

Note that `bevy_ecs_tilemap` uses `TilePos { x, y }` where `x` is the column and `y` is the row, so the loop order and match variables are renamed from Part 2 but the logic is unchanged:

```rust
    let path_mid = map_size.y / 2;

    for x in 0..map_size.x {
        for y in 0..map_size.y {
            let tile_pos = TilePos { x, y };

            let tile_index = match (y, x) {
                // Upper road edge (visually above the road body)
                (r, 0) if r == path_mid + 1 => 79,
                (r, c) if r == path_mid + 1 && c == map_size.x - 1 => 81,
                (r, _) if r == path_mid + 1 => 80,

                // Road body
                (r, 0) if r == path_mid => 102,
                (r, c) if r == path_mid && c == map_size.x - 1 => 104,
                (r, _) if r == path_mid => 103,

                // Lower road edge (visually below the road body)
                (r, 0) if r == path_mid - 1 => 125,
                (r, c) if r == path_mid - 1 && c == map_size.x - 1 => 127,
                (r, _) if r == path_mid - 1 => 126,

                // Everything else is grass
                _ => 129,
            };

            let tile_entity = commands
                .spawn((
                    TileBundle {
                        position: tile_pos,
                        tilemap_id: TilemapId(tilemap_entity),
                        texture_index: TileTextureIndex(tile_index),
                        ..Default::default()
                    },
                    MapTile,
                ))
                .id();

            if tile_index != 129 {
                commands.entity(tile_entity).insert(PathTile);
            }

            tile_storage.set(&tile_pos, tile_entity);
        }
    }
```

`MapTile` and `PathTile` are preserved as extra components in the spawn tuple. Future gameplay systems can query them exactly as before.

---

### Step 5: Finalize the tilemap

After the loop, we insert `TilemapBundle` on the parent entity. This is what turns the empty parent into a renderable map:

```rust
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
}
```

Key fields:

| Field | Purpose |
|---|---|
| `size` | Grid dimensions in tiles. |
| `storage` | The `TileStorage` populated during the loop. |
| `texture` | The source image. `Single` means one texture atlas. |
| `tile_size` | Size of one cell in the texture, in pixels. |
| `grid_size` | Distance between tile centers in world units. Usually equal to `tile_size`. |
| `map_type` | `Square`, `Hexagon`, or `Isometric`. |
| `anchor` | Where the map's origin sits. `Center` places `(0,0)` at the map's center, matching our `Camera2d`. |

> **Run the game now.** The visual output should be identical to Part 2: a centered `15×10` grid with a horizontal dirt road across the middle. If the road looks upside-down, double-check that `79/80/81` are on `path_mid + 1` and `125/126/127` are on `path_mid - 1`, just as in Part 2.

---

### Simplification: Hardcoded map logic

For now, the map dimensions (`15×10`) and tile indices are still hardcoded in the `setup` function. That keeps the code readable while we learn the tilemap API. In **Part 4** we will replace the hardcoded `match` with an auto-tiling system that derives visual tiles from logical map types.

---

## What stayed the same

- The atlas indices (`79`, `80`, `81`, `102`, `103`, `104`, `125`, `126`, `127`, `129`) are unchanged.
- The Y-up coordinate awareness is unchanged — `path_mid + 1` is still visually above the road body.
- `MapTile` and `PathTile` components are preserved on individual tile entities, ready for gameplay queries.

---

## What we lost and gained

| Aspect | Part 2 (individual sprites) | Part 3 (`bevy_ecs_tilemap`) |
|---|---|---|
| Entities spawned | 150 sprites + 1 camera | 150 tiles + 1 tilemap parent + 1 camera |
| Render draw calls | ~150 (batched by texture) | 1 |
| ECS queryable tiles | Yes | Yes |
| Custom components per tile | Yes | Yes |
| Atlas setup | Manual `TextureAtlasLayout` | Automatic via `tile_size` |
| Transform control per tile | Full `Transform` component | Position baked into `TilePos`; z-offset via tilemap transform |

The trade-off is minimal for our use case. We gain rendering efficiency and cleaner atlas management. We lose nothing we need — we can still query tiles, attach components, and animate them by mutating `TileTextureIndex`.

---

## Summary

- We added `bevy_ecs_tilemap` as a rendering dependency and registered `TilemapPlugin`.
- We replaced 150 individual `Sprite` spawns with `TileBundle` entities managed by a single `TilemapBundle`.
- We learned that `bevy_ecs_tilemap` is **rendering-only** — our map logic remains ours.
- We saw how `TileStorage` links grid coordinates to tile entities.
- We configured `ImagePlugin::default_nearest()` to keep pixel-art tiles crisp.

In **Part 4** we will replace hardcoded atlas indices with an auto-tiling system: the map will store only logical tile types (`Grass`, `Path`), and rules will decide which sprite to draw based on each tile's neighbors.
