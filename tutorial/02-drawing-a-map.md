# Part 2: Drawing a Map — From One Sprite to a Grid of Tiles

In Part 1 we got a single red square on screen. In this part we replace it with a real map: a grass field with a dirt road running through it. Along the way, we will learn how Bevy handles sprite atlases, how 2D coordinates work, and why spawning 150 individual sprites is only the first step toward a proper tilemap pipeline.

---

## What we will build

A `15×10` grid of tiles centered on screen:

- **Grass** fills the background.
- A **3-tile-high dirt road** cuts horizontally across the middle.
- The road uses **corner, edge, and body tiles** so it looks like a continuous path rather than a solid rectangle.

Later in the series, enemies will travel along that road.

---

## Asset housekeeping

Before we write code, we clean up the raw asset pack. Asset packs usually ship with individual slices, vector sources, and alternate resolutions. For our project we only need:

```
assets/
├── License.txt
├── Preview.png
└── Tilesheet/
    └── towerDefense_tilesheet.png
```

Removing the `PNG/` folder (299 individual tiles) and the `Vector/` folder keeps the project focused. If you ever need the raw slices, the original Kenney pack is still available online.

---

## Texture atlases: one image, many sprites

The tilesheet is a single `1472×832` PNG that contains a `23×13` grid of `64×64` tiles. Rather than loading 299 separate textures, we load the sheet once and tell Bevy how to slice it.

A **texture atlas** is exactly that: one GPU texture plus a lookup table of rectangles. Each rectangle is an **index** you can reference when spawning a sprite.

---

## Updating `setup`

Our `setup` system now needs to do more than spawn a camera. It needs to:

1. Load the tilesheet texture.
2. Build an atlas layout describing the grid.
3. Loop over grid coordinates and spawn the right tile at each position.

### Loading the texture and building the atlas layout

```rust
fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    commands.spawn(Camera2d);

    let texture = asset_server.load("Tilesheet/towerDefense_tilesheet.png");
    let layout = TextureAtlasLayout::from_grid(UVec2::splat(64), 23, 13, None, None);
    let atlas_layout = texture_atlas_layouts.add(layout);
```

Let's unpack the new parameters.

### `asset_server: Res<AssetServer>`

`AssetServer` is a Bevy resource that handles loading images, sounds, fonts, and other assets. It runs on a background thread so your game does not stall while a texture uploads to the GPU. The path you pass is relative to the `assets/` folder.

`Res<AssetServer>` gives us read-only access. Because loading is asynchronous, the texture is not immediately available in GPU memory, but for simple 2D sprites Bevy queues the draw commands and renders the sprite as soon as the asset finishes loading.

### `texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>`

`Assets<T>` is Bevy's generic asset storage. `TextureAtlasLayout` is not an image — it is pure data that describes how a texture is divided into sub-rectangles. We create one with `from_grid(...)`, then register it in the asset storage with `.add(...)`. The returned handle is what we attach to each sprite so it knows which rectangle to sample.

### `TextureAtlasLayout::from_grid`

```rust
TextureAtlasLayout::from_grid(
    tile_size,      // UVec2::splat(64)  → 64×64 pixels
    columns,        // 23 tiles across
    rows,           // 13 tiles down
    padding,        // None  → no gaps between tiles
    offset,         // None  → starts at the top-left corner
)
```

Bevy counts indices left-to-right, top-to-bottom. So index `0` is the top-left tile, index `22` is the top-right of the first row, index `23` is the leftmost tile of the second row, and so on.

---

## Spawning tiles in a grid

With the atlas ready, we iterate over rows and columns, compute a world position for each tile, and decide which atlas index to use.

```rust
    let cols = 15;
    let rows = 10;
    let tile_size = 64.0;

    let offset_x = -(cols as f32 * tile_size) / 2.0 + tile_size / 2.0;
    let offset_y = -(rows as f32 * tile_size) / 2.0 + tile_size / 2.0;

    let path_mid = rows / 2; // 5 for a 10-row grid

    for row in 0..rows {
        for col in 0..cols {
            let x = offset_x + col as f32 * tile_size;
            let y = offset_y + row as f32 * tile_size;

            let tile_index = match (row, col) {
                // Lower road edge (visually below the road body)
                (r, 0) if r == path_mid - 1 => 125,   // bottom-left corner
                (r, c) if r == path_mid - 1 && c == cols - 1 => 127, // bottom-right corner
                (r, _) if r == path_mid - 1 => 126,    // bottom edge

                // Road body
                (r, 0) if r == path_mid => 102,       // left edge
                (r, c) if r == path_mid && c == cols - 1 => 104, // right edge
                (r, _) if r == path_mid => 103,       // road body

                // Upper road edge (visually above the road body)
                (r, 0) if r == path_mid + 1 => 79,    // upper-left corner
                (r, c) if r == path_mid + 1 && c == cols - 1 => 81, // upper-right corner
                (r, _) if r == path_mid + 1 => 80,    // top edge

                // Everything else is grass
                _ => 129,
            };

            commands.spawn((
                Sprite::from_atlas_image(
                    texture.clone(),
                    TextureAtlas {
                        layout: atlas_layout.clone(),
                        index: tile_index,
                    },
                ),
                Transform::from_xyz(x, y, 0.0),
            ));
        }
    }
}
```

---

## Understanding the math

### Centering the grid

`offset_x` and `offset_y` shift the entire grid so its center sits at the world origin `(0, 0)` — the same point our `Camera2d` looks at by default.

Without this offset, the grid would start at `(0, 0)` and extend only into positive coordinates, leaving most of it off the bottom-left of the screen.

### Bevy's Y-is-up coordinate system

This is the most common source of confusion for newcomers:

- **Positive Y points UP.**
- **Negative Y points DOWN.**
- **Positive X points RIGHT.**
- **Negative X points LEFT.**

That means if `row` increases, the tile moves **higher** on screen. Our road has three rows:

| Code row | Visual position | Tiles |
|---|---|---|
| `path_mid + 1` (row 6) | **Above** the road center | 79, 80, 81 (upper edge) |
| `path_mid` (row 5) | Road center | 102, 103, 104 (body) |
| `path_mid - 1` (row 4) | **Below** the road center | 125, 126, 127 (lower edge) |

If you swap the upper and lower edge rows, the road appears upside-down — corners and shading will look wrong.

### Why `texture.clone()` and `atlas_layout.clone()`?

`asset_server.load(...)` returns a cheap `Handle<Image>` — cloning it just copies an internal ID, not the texture data. Same for the atlas layout handle. We clone inside the loop so every tile references the same underlying assets.

---

## Tagging our tiles

Spawning 150 bare sprites works, but we cannot tell which ones are path tiles and which are grass without searching by position. Let's attach marker components so future systems can query them easily.

```rust
#[derive(Component)]
struct MapTile;

#[derive(Component)]
struct PathTile;
```

Then attach them during spawning:

```rust
            let mut entity = commands.spawn((
                Sprite::from_atlas_image(...),
                Transform::from_xyz(x, y, 0.0),
                MapTile,
            ));

            if tile_index != 129 {
                entity.insert(PathTile);
            }
```

Now any system can ask the ECS:

- "Give me all path tiles" → `Query<&Transform, With<PathTile>>`
- "Give me all map tiles" → `Query<&Transform, With<MapTile>>`

This will be essential when we spawn enemies that must follow the road, or when we let players place towers only on grass.

---

## What this code really does under the hood

Every `commands.spawn(...)` call creates one ECS entity with:

- `Sprite` — tells the renderer to draw a quad with the atlas sub-rectangle.
- `Transform` — where in the world the quad is placed.
- `GlobalTransform` — computed automatically from `Transform`.
- `Visibility` — defaults to visible.
- `Handle<Image>` and atlas data — which texture and which rectangle.

That is **150 entities**, each going through the full sprite rendering pipeline. For a `15×10` grid this is fine. For a `100×100` map it would be wasteful — hence the tilemap plugin we will introduce in Part 3, which renders an entire grid in a single draw call.

---

## Running the project

```bash
cargo run
```

You should see:

- A centered grid of grass tiles.
- A horizontal dirt road across the middle row.
- Corner tiles at both ends of the road, edge tiles along the top and bottom of the road strip.

If the road looks upside-down, double-check that your upper edge tiles (79, 80, 81) are on the `path_mid + 1` row and your lower edge tiles (125, 126, 127) are on the `path_mid - 1` row.

---

## Recap

In this part we:

1. Cleaned up the asset folder to keep only what we need.
2. Learned what a **texture atlas** is and how Bevy slices it with `TextureAtlasLayout`.
3. Used `AssetServer` to load the tilesheet asynchronously.
4. Built a grid by computing world positions from row/column indices.
5. Internalized that **Bevy 2D uses Y-up coordinates**.
6. Added marker components so tiles are queryable by future systems.
7. Saw that 150 individual sprite entities work but hint at the need for a more efficient pipeline.

In **Part 3** we will move from hand-rolled spawning to a data-driven approach: loading map layouts from external files and rendering them with a tilemap plugin.
