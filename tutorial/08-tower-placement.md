# Part 8: Tower Placement — Click to Build

> **Time to read:** ~25 minutes  
> **New concepts:** `Single<>`, `Visibility`, `ButtonInput`, cursor-to-world conversion, shared handles as resources  
> **Prerequisite:** Part 7 (external TOML levels with path-following enemies)

---

## Recap: What We Already Have

Enemies spawn at the first waypoint and walk the path to the base, rotating to face their direction of travel. The map renders from an external TOML file, and the startup chain loads data before spawning anything. There is still no way for the player to interact with the game.

---

## Goal: What We Will Build

We will give the player a way to stop those enemies: placing towers on the map by clicking.

1. **Hover detection** — moving the mouse over the map highlights the tile under the cursor.
2. **Placement preview** — a semi-transparent ghost of the tower base and turret appears when hovering a valid tile.
3. **Placement on click** — left-click on a grass tile spawns a permanent tower sprite.
4. **Occupancy tracking** — you cannot place two towers on the same tile, nor on the path.

No targeting or combat yet. Towers that shoot are two features in one: spatial interaction (click map → place thing) plus combat logic (find enemies → fire projectiles). Splitting them keeps each tutorial focused. Part 9 will give turrets something to shoot at.

---

## New Bevy APIs & Concepts

### `Single<>`

Bevy provides `Single<&T>` (and `Single<(&T, &U)>`) as a query parameter when you know exactly one entity matches. Unlike `Query<&T>` which returns an iterator, `Single` gives direct access and panics at runtime if the count is wrong. It is convenient for the primary window or the main camera, where there is guaranteed to be exactly one.

**Pitfall:** If zero or multiple entities match, `Single` panics. Use it only when the count is architecturally guaranteed (e.g., one window, one camera). For "zero or one," use `Option<Single<&T>>`.

### `Visibility`

`Visibility::Visible` and `Visibility::Hidden` control whether an entity is rendered. Unlike spawning and despawning, toggling visibility keeps the entity alive in the ECS. This is cheaper for ephemeral UI or preview objects that flicker on and off every frame.

### `ButtonInput<MouseButton>`

Bevy aggregates input into resource types. `Res<ButtonInput<MouseButton>>` lets you check `mouse.just_pressed(MouseButton::Left)` to react to a single click event, or `pressed(...)` for held state. Input is read in `Update` for responsiveness; gameplay logic that depends on it can still run in `FixedUpdate` if deterministic timing matters.

### Cursor-to-world conversion

The mouse cursor lives in screen space (pixels from the window corner). To know which tile the cursor is over, we must:

1. Read cursor position in screen space (`window.cursor_position()`).
2. Convert to world space via the camera (`camera.viewport_to_world_2d(camera_transform, cursor)`).
3. Convert world space to grid coordinates (`world_to_tile`).

This three-step pipeline is common in any game where the player clicks on the world.

### Shared handles as resources

Both the preview system and the click-to-place system need the same texture and atlas layout. Rather than loading them in every system, we load once at startup and store the handles in a custom resource. Systems then read `Res<TowerAtlas>`. This pattern is the ECS equivalent of dependency injection: data is produced once and consumed everywhere.

---

## Walkthrough

### Designing the feature

Before writing code, think about what the player should see and what data that requires.

**Player-visible behavior:**

1. Hover over a grass tile → a tinted ghost of a tower appears at that tile.
2. Hover over the path or an occupied tile → the ghost disappears.
3. Left-click a grass tile → a permanent tower appears where the ghost was.
4. The tower consists of a static base and a separate turret that will rotate in Part 9.

**ECS data needed:**

- `Tower`, `TowerTurret`, and `TowerPreview` marker components to distinguish base, turret, and ghost entities.
- `PlacedTowers` resource — a `HashSet` of occupied tile coordinates for O(1) placement validation.
- `TowerAtlas` resource — shared texture and atlas layout handles.
- `world_to_tile` helper — the inverse of `tile_to_world`, converting world-space mouse position back to grid coordinates.
- `hovered_placeable_tile` helper — shared validation logic used by both the preview and placement systems.

### Step 1: Create `src/tower.rs` with components and resources

We need three marker components. `Tower` is public because other systems (targeting, projectiles) will query for it. `TowerTurret` and `TowerPreview` are `pub(crate)` — visible to other modules in the crate but not outside it — because no external code needs to spawn them directly.

```rust
use bevy::prelude::*;
use std::collections::HashSet;

use crate::enemy::tile_to_world;
use crate::map::{MapLayout, TileType};

const TOWER_BASE: usize = 180;
const TOWER_TOP: usize = 203;

#[derive(Component)]
pub struct Tower;

#[derive(Component)]
pub(crate) struct TowerTurret;

#[derive(Component)]
pub(crate) struct TowerPreview;
```

> **Why `pub(crate)`?** In Rust, visibility defaults to private. `pub(crate)` is like package-private in Java or internal in C#: other modules in the same crate can use the type, but external code cannot. This keeps the module's public surface small while still allowing `main.rs` to write `Query<..., With<TowerPreview>>`.

We also need two resources. `PlacedTowers` tracks which grid squares already have a tower. `TowerAtlas` holds the preloaded texture and layout handles.

```rust
#[derive(Resource)]
pub struct TowerAtlas {
    pub texture: Handle<Image>,
    pub layout: Handle<TextureAtlasLayout>,
}

#[derive(Resource, Default)]
pub struct PlacedTowers(pub HashSet<[u32; 2]>);
```

> **Why a `HashSet` and not a component on tiles?** The tilemap entities from `bevy_ecs_tilemap` are owned by the plugin and do not have our custom components. Maintaining a separate set is simpler than trying to attach occupancy data to foreign entities. In a larger game you might store tower entities in a spatial hash or quadtree; a `HashSet` is enough for a small grid.

### Step 2: Preload the atlas at startup

Both the preview and placement systems need the same tilesheet. Loading it in every system would be wasteful and repetitive. Instead, one startup system loads the texture, builds the atlas layout, and inserts `TowerAtlas` as a resource.

```rust
pub fn setup_tower_atlas(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let texture = asset_server.load("Tilesheet/towerDefense_tilesheet.png");
    let layout = TextureAtlasLayout::from_grid(UVec2::splat(64), 23, 13, None, None);
    commands.insert_resource(TowerAtlas {
        texture,
        layout: texture_atlas_layouts.add(layout),
    });
}
```

After this system runs, any system can read `Res<TowerAtlas>` and spawn sprites without touching `AssetServer` or `Assets<TextureAtlasLayout>`.

### Step 3: Add `world_to_tile`

Part 7 gave us `tile_to_world`, which converts grid coordinates to world-space pixel positions. Placement needs the inverse: given a mouse position in world space, which tile is under the cursor?

The math is the reverse of `tile_to_world`. `tile_to_world` places sprites at the *center* of each tile, so `world_to_tile` must find the tile *center* nearest to the cursor. We subtract the origin offset, divide by tile size, and **round** to the nearest integer index. `.round()` is the correct inverse of center-based coordinates; `.floor()` would incorrectly bias toward lower-index tiles when the cursor is in the right half of a tile.

```rust
fn world_to_tile(world: Vec2, map_width: u32, map_height: u32) -> Option<[u32; 2]> {
    let tile_size = 64.0;
    let origin_x = -(map_width as f32) * tile_size / 2.0 + tile_size / 2.0;
    let origin_y = -(map_height as f32) * tile_size / 2.0 + tile_size / 2.0;

    let tx = ((world.x - origin_x) / tile_size).round() as i32;
    let ty = ((world.y - origin_y) / tile_size).round() as i32;

    if tx >= 0 && tx < map_width as i32 && ty >= 0 && ty < map_height as i32 {
        Some([tx as u32, ty as u32])
    } else {
        None
    }
}
```

The function returns `Option<[u32; 2]>` because the cursor may be off the map. Callers handle `None` by treating the position as unplaceable.

### Step 4: Extract shared validation

Both the preview system and the click-to-place system must answer the same question: "Is the cursor currently over an unoccupied grass tile?" Rather than duplicating the cursor→world→tile→validation logic, we extract it into a helper.

```rust
/// Returns the tile under the cursor only if it is an unoccupied grass tile.
fn hovered_placeable_tile(
    window: &Window,
    camera: &Camera,
    camera_transform: &GlobalTransform,
    map_layout: &MapLayout,
    placed: &PlacedTowers,
) -> Option<[u32; 2]> {
    let tile = window
        .cursor_position()
        .and_then(|cursor| camera.viewport_to_world_2d(camera_transform, cursor).ok())
        .and_then(|world| world_to_tile(world, map_layout.width, map_layout.height))?;

    let is_grass = map_layout.get(tile[0], tile[1]) == Some(TileType::Grass);
    if is_grass && !placed.0.contains(&tile) {
        Some(tile)
    } else {
        None
    }
}
```

The helper chains three fallible operations with `and_then`: get cursor position, convert to world space, convert to tile coordinates. If any step fails, the chain returns `None`. The final check verifies grass + vacancy.

### Step 5: Spawn the placement preview

The preview consists of two entities — base and turret — created once at startup. Both start hidden and share the `TowerPreview` marker so a single query can update them together.

We tint the preview sprites to 50% opacity so the player understands they are not yet real towers.

```rust
pub fn spawn_placement_preview(
    mut commands: Commands,
    atlas: Res<TowerAtlas>,
) {
    let tinted = |index: usize| Sprite {
        color: Color::srgba(1.0, 1.0, 1.0, 0.5),
        ..Sprite::from_atlas_image(
            atlas.texture.clone(),
            TextureAtlas { layout: atlas.layout.clone(), index },
        )
    };

    commands.spawn((
        TowerPreview,
        tinted(TOWER_BASE),
        Transform::from_xyz(0.0, 0.0, 2.0),
        Visibility::Hidden,
    ));
    commands.spawn((
        TowerPreview,
        tinted(TOWER_TOP),
        Transform::from_xyz(0.0, 0.0, 2.1),
        Visibility::Hidden,
    ));
}
```

> **Why persistent entities instead of spawn/despawn?** The preview toggles every frame as the mouse moves. Spawning and despawning entities involves allocator overhead and fragmenting the ECS archetype tables. Toggling `Visibility` is a single boolean write. For objects that appear and disappear rapidly, visibility is the idiomatic choice.

### Step 6: Update the preview on hover

This system runs every frame in `Update`. It asks: "Is the cursor over a valid tile?" If yes, it moves both preview entities to that tile's world position and shows them. If no, it hides them.

What does it query?
- `Single<&Window>` — the one and only window.
- `Single<(&Camera, &GlobalTransform)>` — the one 2D camera and its world transform.
- `Res<MapLayout>` and `Res<PlacedTowers>` — for validation.
- `Query<(&mut Transform, &mut Visibility), With<TowerPreview>>` — the two preview entities to reposition and show/hide.

```rust
pub fn update_placement_preview(
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform)>,
    map_layout: Res<MapLayout>,
    placed: Res<PlacedTowers>,
    mut preview_q: Query<(&mut Transform, &mut Visibility), With<TowerPreview>>,
) {
    let (cam, cam_transform) = *camera;

    let Some(tile) = hovered_placeable_tile(
        &window, &cam, &cam_transform, &map_layout, &placed,
    ) else {
        for (_, mut vis) in preview_q.iter_mut() {
            *vis = Visibility::Hidden;
        }
        return;
    };

    let pos = tile_to_world(tile, map_layout.width as f32, map_layout.height as f32);

    for (mut transform, mut vis) in preview_q.iter_mut() {
        transform.translation.x = pos.x;
        transform.translation.y = pos.y;
        *vis = Visibility::Visible;
    }
}
```

The `let-else` block handles the "cursor is not over a valid tile" case early: hide everything and return. If validation passes, the loop updates both preview entities to the same `(x, y)` position. The base stays at `z = 2.0` and the turret at `z = 2.1`, preserving their relative layering.

### Step 7: Place towers on click

This system also runs in `Update`. It checks for a left-click, validates the cursor position through the same helper, then spawns two permanent entities: a `Tower` base and a `TowerTurret` top.

```rust
pub fn place_tower_on_click(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform)>,
    map_layout: Res<MapLayout>,
    mut placed: ResMut<PlacedTowers>,
    atlas: Res<TowerAtlas>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    let (cam, cam_transform) = *camera;

    let Some(tile) = hovered_placeable_tile(
        &window, &cam, &cam_transform, &map_layout, &placed,
    ) else {
        return;
    };

    placed.0.insert(tile);
    let pos = tile_to_world(tile, map_layout.width as f32, map_layout.height as f32);

    // Base (static, never rotates)
    commands.spawn((
        Tower,
        Sprite::from_atlas_image(
            atlas.texture.clone(),
            TextureAtlas { layout: atlas.layout.clone(), index: TOWER_BASE },
        ),
        Transform::from_xyz(pos.x, pos.y, 2.0),
    ));

    // Turret (will rotate toward enemies in Part 9)
    commands.spawn((
        TowerTurret,
        Sprite::from_atlas_image(
            atlas.texture.clone(),
            TextureAtlas { layout: atlas.layout.clone(), index: TOWER_TOP },
        ),
        Transform::from_xyz(pos.x, pos.y, 2.1),
    ));
}
```

> **Why two entities per tower?** The base never moves or rotates. The turret must rotate independently to track enemies. If they were one entity, rotating the turret would also rotate the base. Separate entities mean separate `Transform` components, each updated by its own systems.
>
> **Why does the click system not touch the preview?** After `placed.0.insert(tile)`, the tile is occupied. On the next frame, `hovered_placeable_tile` returns `None` for that tile, and `update_placement_preview` hides the ghost automatically. No cross-system coordination is needed because both systems read the same `PlacedTowers` resource.

### Step 8: Wire everything in `main.rs`

Add the module declaration, imports, resource initialization, and system registration:

```rust
mod tower;

use tower::{
    PlacedTowers, setup_tower_atlas, spawn_placement_preview,
    update_placement_preview, place_tower_on_click,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .add_plugins(TilemapPlugin)
        .init_resource::<PlacedTowers>()
        .add_systems(Startup, (
            load_level_data,
            setup_tower_atlas,
            spawn_tilemap,
            spawn_test_enemy,
            spawn_placement_preview,
        ).chain())
        .add_systems(FixedUpdate, (move_enemies, cleanup_finished_enemies))
        .add_systems(Update, (update_placement_preview, place_tower_on_click))
        .run();
}
```

Key scheduling decisions:
- `setup_tower_atlas` is chained in `Startup` so the `TowerAtlas` resource exists before any render frame.
- `.init_resource::<PlacedTowers>()` creates an empty `HashSet` automatically; no startup system needed.
- Gameplay (`move_enemies`, `cleanup_finished_enemies`) stays in `FixedUpdate` for deterministic timestep.
- Input (`update_placement_preview`, `place_tower_on_click`) runs in `Update` for frame-rate-responsive feedback.

### Step 9: Verify

```bash
cargo run
```

You should see:

- The same `15×10` map and moving enemy from Part 7.
- **Hover a grass tile**: semi-transparent ghosts of the tower base and turret appear.
- **Hover the path or off-map**: ghosts disappear.
- **Click a grass tile**: the ghosts solidify into a permanent tower (base #180 + turret #203).
- **Hover the placed tower**: ghosts do not appear (occupancy check).
- The turret sits on top of the base; it does not rotate yet.

### Simplifications

| Simplification | Why it works | Future direction |
|---|---|---|
| **One tower type** | No selection UI or cost system needed yet. | A future part will add a tower palette, economy, and per-type data. |
| **Hardcoded sprite indices** | Constants `TOWER_BASE = 180`, `TOWER_TOP = 203`. | Future rocket launchers may need per-type spawn functions with different entity counts and layer orders. |
| **Preview uses same sprites as placed tower** | The ghost is just a tinted version of the final art. | For complex multi-sprite towers, the preview may use a pre-combined sprite while placed towers need separate entities. |
| **No economy** | Towers cost nothing; the focus is spatial interaction. | Resource management (gold, energy) in a future part. |
| **`PlacedTowers` as a `HashSet` resource** | O(1) occupancy check on a small grid. | A larger game might use a spatial hash or attach occupancy directly to tile entities. |

---

## Summary

- We created `src/tower.rs` with `Tower`, `TowerTurret`, and `TowerPreview` marker components.
- We added `TowerAtlas` (shared handles) and `PlacedTowers` (occupancy tracking) as resources.
- We implemented `world_to_tile`, the inverse of Part 7's `tile_to_world`, for cursor-to-grid conversion.
- We extracted `hovered_placeable_tile` to share validation between the preview and placement systems.
- We built a **hover preview** using persistent entities with toggled `Visibility`.
- We built a **click-to-place** system that spawns two entities (base + turret) and updates the occupancy set.
- We scheduled input systems in `Update` for responsiveness and gameplay systems in `FixedUpdate` for determinism.

In **Part 9** we will add targeting and damage: turrets will find the nearest enemy in range, rotate toward it, and apply instant damage. Projectiles come in a later part.
