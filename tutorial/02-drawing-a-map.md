# Part 2: Drawing a Map — From One Sprite to a Grid of Tiles

> **Time to read:** ~20 minutes  
> **New concepts:** `AssetServer`, `Res`, `ResMut`, `TextureAtlasLayout`, `TextureAtlas`, `Component`, `Transform`  
> **Prerequisite:** Part 1 (a minimal Bevy app with a window, camera, and red square)

---

## Recap: What We Already Have

We have a minimal Bevy app that opens a window, spawns a 2D camera, and renders a red square at the origin. We know how to create an `App`, register `DefaultPlugins`, and run a `Startup` system that uses `Commands` to spawn entities.

---

## Goal: What We Will Build

We will replace the single red square with a real map: a `15×10` grid of tiles centered on the screen. The grid will have:

- **Grass** filling the background.
- A **3-tile-high dirt road** cutting horizontally across the middle.
- **Corner, edge, and body tiles** so the road looks like a continuous path rather than a solid rectangle.

Later in the series, enemies will travel along that road.

---

## New Bevy APIs & Concepts

### `Res<T>` and `ResMut<T>`

In Bevy, a **resource** is a global singleton that lives in the ECS world but is not attached to any specific entity. Any number of systems can read the same resource at the same time, making resources the natural place for data that needs to be shared across your game: the game clock, the current score, the asset loader, or the storage that holds all your texture atlases.

When you write a system that needs to read a resource — for example, asking the `AssetServer` to load a texture — you add `Res<AssetServer>` as a parameter. Bevy notices the type and injects the correct resource automatically.

When you need to *change* a resource — for example, registering a newly created `TextureAtlasLayout` into the asset storage — you use `ResMut<Assets<TextureAtlasLayout>>` instead. The `Mut` tells Bevy you need write access, and Bevy's scheduler ensures no other system tries to write to the same resource at the same time.

In this part we will use both: `Res<AssetServer>` to load the tilesheet, and `ResMut<Assets<TextureAtlasLayout>>` to store the atlas layout we build from it.
### `AssetServer`

`AssetServer` is a Bevy *resource* that handles loading images, sounds, fonts, and other files. Because it is a resource, you access it inside a system with `Res<AssetServer>`. It runs on a background thread so your game does not stall while a texture uploads to the GPU. The path you pass to `load(...)` is relative to the `assets/` folder.

Because loading is asynchronous, the texture is not immediately available in GPU memory. For simple 2D sprites, Bevy queues the draw commands and renders the sprite as soon as the asset finishes loading.
### `TextureAtlasLayout` and `TextureAtlas`

A **tilesheet** is one large image that contains many smaller images arranged in a grid. Our tilesheet has 23 columns and 13 rows of 64×64 tiles. To use individual tiles, we need to tell Bevy how the sheet is divided so it can extract the right piece for each sprite.

`TextureAtlasLayout` does exactly that: it stores the "recipe" for slicing the sheet — tile size, columns, rows, and any padding between tiles. You create the layout, register it in Bevy's asset storage (`Assets<TextureAtlasLayout>`), and get back a handle.

`TextureAtlas` is the component you attach to each sprite entity. It holds two things: a handle to the layout (so the sprite knows which sheet to look at) and an index (so it knows which tile to extract). When Bevy renders the sprite, it uses the layout to calculate which rectangle of the source image to draw.

**Pitfall:** Indices are counted left-to-right, top-to-bottom. Index `0` is the top-left tile, index `22` is the top-right of the first row, and index `23` is the leftmost tile of the second row. It is easy to miscount when eyeballing a tilesheet.

### `Component`

A *component* is a Rust struct or enum that gets attached to an entity. Components are plain data — no behavior, just state. Bevy's ECS lets you query for all entities that have a specific set of components.

You define a component by deriving the `Component` trait:

```rust
#[derive(Component)]
struct MapTile;
```

Marker components like `MapTile` have no fields; their mere presence on an entity is enough to identify it in a query.

### `Transform`

`Transform` describes where an entity is in the world: its translation (position), rotation, and scale. For 2D games we mostly care about `Transform::from_xyz(x, y, z)`, where `z` controls draw order (higher values draw on top). We will use `Transform` to place each tile at its calculated grid position.

---

## Walkthrough

### Step 1: Download the Asset Pack

We will use the [Tower Defense (Top Down)](https://kenney.nl/assets/tower-defense-top-down) asset pack by Kenney. Place the `Tilesheet/towerDefense_tilesheet.png` file into your project's `assets/` folder so Bevy can find it.

The tilesheet is a single `1472×832` PNG that contains a `23×13` grid of `64×64` tiles. Rather than loading hundreds of individual textures, we load the sheet once and tell Bevy how to slice it.

### Step 2: Load the Tilesheet and Build the Atlas Layout

Our `setup` system now needs two new parameters to handle assets. Bevy will inject them automatically because their types are registered as resources:

> **Note:** We are replacing the single red square from Part 1 with a full grid of tiles. Remove the `Sprite::from_color` spawn from `setup` — the grid loop below will handle all rendering.

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

`TextureAtlasLayout::from_grid` takes five arguments:

| Argument | Value | Meaning |
|---|---|---|
| `tile_size` | `UVec2::splat(64)` | Each tile is 64×64 pixels. |
| `columns` | `23` | 23 tiles across. |
| `rows` | `13` | 13 tiles down. |
| `padding` | `None` | No gaps between tiles. |
| `offset` | `None` | Starts at the top-left corner. |

Bevy counts indices left-to-right, top-to-bottom. Index `0` is the top-left tile, index `22` is the top-right of the first row, index `23` is the leftmost tile of the second row, and so on.

`asset_server.load(...)` returns a cheap `Handle<Image>` — cloning it later just copies an internal ID, not the texture data. Same for the atlas layout handle.

### Step 3: Spawn Tiles in a Grid

With the atlas ready, we loop over every cell in the grid, compute its screen position, decide which tile it should display, and spawn a sprite there. We will build this in three pieces: the grid constants, the position math, and the tile selection.

#### Grid constants and centering

First, define the grid size and tile size. We then compute offsets so the grid is centered on the world origin — the same point our camera looks at:

```rust
    let cols = 15;
    let rows = 10;
    let tile_size = 64.0;

    let offset_x = -(cols as f32 * tile_size) / 2.0 + tile_size / 2.0;
    let offset_y = -(rows as f32 * tile_size) / 2.0 + tile_size / 2.0;
```

The offsets shift the grid so its center sits at `(0, 0)`. Without them, the grid would start at the origin and extend only into positive coordinates, leaving most of it off the top-right of the screen.

#### Position and tile selection

For each row and column, we calculate the tile's world position. We also pick a `tile_index` based on which row we're in: three rows form the road, and everything else is grass.

The road sits in the middle of the grid. `path_mid` is the center row; the rows above and below it become the road edges. In Bevy, **positive Y points up**, so `path_mid + 1` is visually *above* the road and `path_mid - 1` is *below* it. If you swap these, the road will appear upside-down.

```rust
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
```

The `match` uses guard clauses like `if r == path_mid` to select the right atlas index. Corner tiles are at the left and right ends of the edge rows; body tiles fill the middle. The catch-all `_ => 129` covers every cell that is not part of the road.

#### Spawning each tile

Finally, we spawn a sprite at the calculated position. `Sprite::from_atlas_image` takes the texture handle and a `TextureAtlas` that points to our layout and the chosen index. We also attach a `Transform` to place the sprite in the world:

```rust
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

`texture.clone()` and `atlas_layout.clone()` copy only the internal handle IDs — not the actual image data — so calling them inside the loop is cheap.


### Step 4: Remember Which Tiles Are Roads

Spawning 150 bare sprites works for rendering, but our gameplay systems will eventually need to distinguish walkable road from buildable grass. There are several ways to store this information.

One approach is to keep a separate data structure — a 2D array or a `HashMap<(i32, i32), TileKind>` — where you look up a tile's type by its grid coordinates. This is fast for point queries: "is position (5, 3) a path tile?"

Another approach, which fits Bevy's ECS naturally, is to attach **marker components** to the entities themselves. The strength of this approach is not point lookups but **set queries**: "give me *all* path tiles" or "give me every map tile with a transform." Bevy can answer these efficiently because the ECS stores components in contiguous arrays.

You can even maintain both if your game needs both patterns. In this tutorial we will use marker components as a clean, ECS-idiomatic way to label our tiles. We will not use them immediately in the next few parts — enemies will spawn at predefined locations and towers will be placed by checking the tile under the mouse — but the pattern is essential once you start writing gameplay systems that need to reason about the map.

Add these definitions at the top of `main.rs`:

```rust
#[derive(Component)]
struct MapTile;

#[derive(Component)]
struct PathTile;
```

`commands.spawn(...)` returns an `EntityCommands` handle — a temporary reference to the entity we just created. We can use that handle to queue additional changes before the frame ends. Here we spawn every tile with `MapTile`, then conditionally add `PathTile` only for the road tiles:

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

### Simplification: Individual Sprites vs. Tilemaps

For a `15×10` grid, spawning one sprite entity per tile is fine. Every `commands.spawn(...)` creates an ECS entity with a `Sprite`, `Transform`, and other rendering components. That is 150 entities going through the full sprite pipeline.

For a larger map — say `100×100` — this would be wasteful. A production tower defense game would use a **tilemap**, which renders an entire grid in a single draw call. We will introduce a tilemap plugin in Part 3. For now, individual sprites keep the code transparent: you can see exactly how each tile becomes an entity.

---

## Running the Project

```bash
cargo run
```

You should see:

- A centered grid of grass tiles.
- A horizontal dirt road across the middle row.
- Corner tiles at both ends of the road, edge tiles along the top and bottom of the road strip.

If the road looks upside-down, double-check that your upper edge tiles (79, 80, 81) are on the `path_mid + 1` row and your lower edge tiles (125, 126, 127) are on the `path_mid - 1` row.

---

## Summary

In this part we:

1. Downloaded the [Tower Defense (Top Down)](https://kenney.nl/assets/tower-defense-top-down) tilesheet by Kenney.
2. Used `AssetServer` to load the tilesheet asynchronously.
3. Built a `TextureAtlasLayout` to slice the tilesheet into addressable tiles.
4. Computed world positions from row/column indices to center the grid on screen.
5. Internalized that **Bevy 2D uses Y-up coordinates**.
6. Added `MapTile` and `PathTile` marker components so tiles are queryable by future systems.
7. Saw that 150 individual sprite entities work for a small grid, but hinted at the need for a more efficient pipeline (tilemaps) later.

In **Part 3** we will move from hand-rolled spawning to a data-driven approach: loading map layouts from external files and rendering them with a tilemap plugin.
