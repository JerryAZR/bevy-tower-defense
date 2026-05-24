# Part 3: Tilemap Rendering — From Sprites to a Dedicated Plugin

In Part 2 we drew a grid by spawning 150 individual sprite entities. It works, but it is not how production games render large maps. In this part we switch to `bevy_ecs_tilemap`, a community plugin that renders an entire grid in a single GPU draw call while preserving the ECS architecture we already built.

---

## Why switch?

Our hand-rolled approach from Part 2 creates one ECS entity per tile. Each entity carries a full `Sprite` component, a `Transform`, and visibility bookkeeping. The renderer processes every one of them separately. For a `15×10` grid this is negligible, but for a `100×100` map it means:

- 10,000 entities in the world.
- 10,000 transform updates every frame.
- 10,000 separate draw calls (or at best, batching overhead).

A **tilemap plugin** solves this by treating the grid as a single renderable object. It stores tile data (position, texture index, color) in GPU-friendly buffers and draws everything in one pass. The tiles are still queryable ECS entities — you can attach components, run systems on them, and mutate them at runtime — but the renderer no longer treats each one as an independent sprite.

---

## Plugin choice: `bevy_ecs_tilemap`

Bevy has no built-in tilemap. The ecosystem offers several options; `bevy_ecs_tilemap` is the most widely used and actively maintained. It is deliberately **rendering-only**:

- It does **not** load map files (no Tiled or LDTK integration out of the box).
- It does **not** implement auto-tiling or pathfinding.
- It **does** give you chunked culling, GPU instancing, animated tiles, hex/iso support, and multiple layers.

This matches our architecture perfectly. We keep our own map generation logic — the `match` expression that decides which atlas index each cell uses — and hand the results to `bevy_ecs_tilemap` for efficient display.

---

## Adding the dependency

```bash
cargo add bevy_ecs_tilemap
```

The crate version tracks Bevy's own release cycle. Bevy 0.18 pairs with `bevy_ecs_tilemap` 0.18.

---

## How `bevy_ecs_tilemap` works in practice

You create **one parent entity** that carries a `TilemapBundle`. This bundle holds the texture, grid size, and a `TileStorage` — a lookup table from grid coordinates to entity IDs. Then you spawn **one child entity per tile**, each with a `TileBundle` containing its `TilePos` (grid coordinate), `TileTextureIndex` (which atlas cell to draw), and a `TilemapId` pointing back to the parent. The plugin registers a specialized render pipeline that reads all tile data in bulk, uploads it to the GPU, and draws the entire grid as a single mesh. Each tile is still a real ECS entity — you can query it, attach components, and mutate its `TileTextureIndex` at runtime — but the renderer treats the collection as one object.

---

## What changes in the code

### Plugin registration

```rust
use bevy_ecs_tilemap::prelude::*;

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

`ImagePlugin::default_nearest()` tells Bevy to use **nearest-neighbor filtering** when scaling textures. This is the correct choice for pixel-art tilesets like ours; without it, the GPU's default bilinear filtering would blur the crisp `64×64` tiles.

### No more `TextureAtlasLayout`

In Part 2 we explicitly built a `TextureAtlasLayout` and passed it to every sprite. `bevy_ecs_tilemap` handles atlas slicing internally. You give it:

- A single texture handle (`TilemapTexture::Single`).
- A `TilemapTileSize` telling it how big each cell is in pixels.

The plugin's shader computes UV coordinates at runtime using the formula:

```
sprite_sheet_x = tile_index % columns * tile_size
sprite_sheet_y = tile_index / columns * tile_size
```

This is exactly the same left-to-right, top-to-bottom indexing we used manually. The difference is that the GPU does the math per-vertex instead of us setting up atlas rectangles on the CPU.

### The spawn flow

`bevy_ecs_tilemap` requires a two-phase spawn:

1. **Create an empty entity** that will become the tilemap parent.
2. **Spawn each tile** as a child entity, linking it to the parent via `TilemapId`.
3. **Insert `TilemapBundle`** on the parent, supplying the completed `TileStorage`.

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

`TileStorage` is the bridge. It is a dense grid that maps `TilePos { x, y }` to the `Entity` ID of the tile at that coordinate. The renderer uses it to know which tile data exists, and your systems can use it to look up neighbors or modify specific cells.

### Spawning tiles

```rust
    let path_mid = map_size.y / 2;

    for x in 0..map_size.x {
        for y in 0..map_size.y {
            let tile_pos = TilePos { x, y };

            let tile_index = match (y, x) {
                // Lower road edge (visually below the road body)
                (r, 0) if r == path_mid - 1 => 125,
                (r, c) if r == path_mid - 1 && c == map_size.x - 1 => 127,
                (r, _) if r == path_mid - 1 => 126,

                // Road body
                (r, 0) if r == path_mid => 102,
                (r, c) if r == path_mid && c == map_size.x - 1 => 104,
                (r, _) if r == path_mid => 103,

                // Upper road edge (visually above the road body)
                (r, 0) if r == path_mid + 1 => 79,
                (r, c) if r == path_mid + 1 && c == map_size.x - 1 => 81,
                (r, _) if r == path_mid + 1 => 80,

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

The tile selection logic is **identical** to Part 2. What changes is the spawning API:

- `TileBundle` replaces the `(Sprite, Transform)` pair. It carries `TilePos`, `TilemapId`, `TileTextureIndex`, and internal render state.
- `TileTextureIndex(tile_index)` tells the shader which sub-rectangle of the texture to sample.
- `MapTile` and `PathTile` are still our own marker components, attached as extra tuple elements in the spawn. Future systems can query them exactly as before.

### Finalizing the tilemap

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

`TilemapBundle` is the component set that turns the empty parent entity into a renderable map. Key fields:

| Field | Purpose |
|---|---|
| `size` | Grid dimensions in tiles. |
| `storage` | The `TileStorage` populated during the loop. |
| `texture` | The source image. `Single` means one texture atlas. |
| `tile_size` | Size of one cell in the texture, in pixels. |
| `grid_size` | Distance between tile centers in world units. Usually equal to `tile_size`. |
| `map_type` | `Square`, `Hexagon`, or `Isometric`. |
| `anchor` | Where the map's origin sits. `Center` places `(0,0)` at the map's center, matching our `Camera2d`. |

---

## What stayed the same

- The atlas indices (79, 80, 81, 102, 103, 104, 125, 126, 127, 129) are unchanged.
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

## Running the project

```bash
cargo run
```

The visual output should be identical to Part 2: a centered `15×10` grid with a horizontal dirt road across the middle. The difference is entirely under the hood.

---

## Recap

In this part we:

1. Added `bevy_ecs_tilemap` as a rendering dependency.
2. Replaced 150 individual `Sprite` spawns with `TileBundle` entities managed by a single `TilemapBundle`.
3. Learned that `bevy_ecs_tilemap` is **rendering-only** — our map logic remains ours.
4. Saw how `TileStorage` links grid coordinates to tile entities.
5. Understood that `TileTextureIndex` maps to atlas sub-rectangles via the same grid math we used manually.
6. Preserved `MapTile` and `PathTile` markers so gameplay systems can still query the grid.

In **Part 4** we will extract the map definition from code into an external data file, making it possible to design levels without recompiling.
