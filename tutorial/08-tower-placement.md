# Part 8: Tower Placement — Click to Build

In Part 7 enemies walked a path to the base. Now we give the player a way to stop them: placing towers on the map. This part focuses on **click-and-place** — hovering shows a ghost preview, clicking on a valid grass tile places a tower. No targeting or combat yet.

---

## What we will build

- **Hover detection** — moving the mouse over the map highlights the tile under the cursor.
- **Placement preview** — a semi-transparent ghost of the tower base and turret appears when hovering a valid tile.
- **Placement on click** — left-click on a grass tile spawns a permanent tower sprite.
- **Occupancy tracking** — you cannot place two towers on the same tile.
- **Duplicate prevention** — you cannot place towers on the path.

The tower sprite uses base #180 and turret top #203 from the Kenney tilesheet. The turret is spawned as a separate entity so it can rotate independently when targeting is added in Part 9.

---

## Why placement before targeting?

Towers that shoot are two features in one: spatial interaction (click map → place thing) plus combat logic (find enemies → fire projectiles). Splitting them into separate parts keeps each tutorial focused.

By the end of this part you will have a working click-to-place system with hover feedback. The tower sits on the map as two entities (base + turret); Part 9 will give the turret a target to track.

---

## New module: `src/tower.rs`

### Components

```rust
#[derive(Component)]
pub struct Tower;

#[derive(Component)]
struct TowerTurret;

#[derive(Component)]
struct TowerPreview;
```

- **`Tower`** is a plain marker component. No data needed — tower position is tracked by `Transform`.
- **`TowerTurret`** marks the turret entity (spawned separately from the base). Part 9 will query `With<TowerTurret>` to rotate it toward targets.
- **`TowerPreview`** is `pub(crate)` — visible within the crate so the `Query<..., With<TowerPreview>>` type compiles. Two preview entities exist (base + turret), both share this marker.

### Occupancy resource

```rust
#[derive(Resource, Default)]
pub struct PlacedTowers(pub HashSet<[u32; 2]>);
```

O(1) occupancy checks via `HashSet`. Initialized with `.init_resource::<PlacedTowers>()` in `main()`.

### Atlas resource

Both placement systems need the same tilesheet texture and atlas layout. Instead of loading them repeatedly, we create a resource once at startup:

```rust
#[derive(Resource)]
pub struct TowerAtlas {
    texture: Handle<Image>,
    layout: Handle<TextureAtlasLayout>,
}

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

This eliminates the `asset_server.load` + `from_grid` + `add` boilerplate. Systems now just read `Res<TowerAtlas>`.

### Shared tile validation

Both the preview and click-to-place systems need the same cursor-to-tile logic with the same validity checks. We extract it into a helper:

```rust
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

Returns `Some(tile)` only if the cursor is over an unoccupied grass tile. Otherwise `None`. This cuts ~20 lines of duplicated validation.

### Preview startup

Two preview entities are created once at startup — base (#180) and turret (#203). Both start hidden and share the `TowerPreview` marker:

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

### Preview update system

The update system toggles `Visibility` and repositions both preview entities via a `Query` loop:

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

**Key details:**
- **`Visibility` toggle** replaces spawn/despawn. Entities live forever, just hidden when the cursor is invalid.
- **`Single<...>` for window and camera**: there is exactly one of each. Direct access without `.single()`.
- **`Query<...>` for preview**: there are now two preview entities (base + turret). A loop toggles both together.
- **z = 2.0 / 2.1**: preview renders above enemies (z = 1.0). Turret on top of base.

### Click-to-place system

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

    // Base (static)
    commands.spawn((
        Tower,
        Sprite::from_atlas_image(
            atlas.texture.clone(),
            TextureAtlas { layout: atlas.layout.clone(), index: 180 },
        ),
        Transform::from_xyz(pos.x, pos.y, 2.0),
    ));

    // Turret (rotates in Part 9)
    commands.spawn((
        TowerTurret,
        Sprite::from_atlas_image(
            atlas.texture.clone(),
            TextureAtlas { layout: atlas.layout.clone(), index: 203 },
        ),
        Transform::from_xyz(pos.x, pos.y, 2.1),
    ));
}
```

### Why two entities?

| Entity | Sprite | z | Why separate? |
|---|---|---|---|
| Base | #180 | 2.0 | Never moves or rotates |
| Turret | #203 | 2.1 | Rotates to track target (Part 9) |

The turret is at `z = 2.1` so it renders above the base. They share the same `(x, y)` world position.

### Why the click system ignores the preview

The preview is owned by `update_placement_preview`. When a tower is placed, `placed.0.insert(tile)` marks the tile as occupied. On the **next frame**, `hovered_placeable_tile` returns `None` for that tile, and the preview hides automatically. No cross-system coordination needed.

### `world_to_tile` helper

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

The `.round()` prevents flickering when hovering near tile borders.

---

## Wiring in `main.rs`

```rust
use tower::{PlacedTowers, setup_tower_atlas, spawn_placement_preview, update_placement_preview, place_tower_on_click};

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

- **`setup_tower_atlas`** loads the texture and atlas layout once at startup. Runs after `load_level_data`, before rendering.
- **`init_resource::<PlacedTowers>()`** creates an empty occupancy set.
- **Gameplay in `FixedUpdate`**: `move_enemies` at a fixed timestep.
- **Input in `Update`**: preview and click placement at the render rate for responsiveness.

---

## Running the project

```bash
cargo run
```

You should see:

- The same `15×10` map and moving enemy from Part 7.
- **Hover a grass tile**: semi-transparent ghosts of the tower base **and turret** appear.
- **Hover the path or off-map**: ghosts disappear.
- **Click a grass tile**: the ghosts solidify into a permanent tower (base #180 + turret #203).
- **Hover the placed tower**: ghosts do not appear (occupancy check).
- The turret sits on top of the base; it does not rotate yet.

---

## Design decisions

| Decision | Rationale |
|---|---|
| **`PlacedTowers` as a `HashSet` resource** | O(1) occupancy check. Teaches the denormalized ECS pattern. |
| **`TowerAtlas` resource** | Eliminates repeated texture loading. All tower systems share one handle pair. |
| **`hovered_placeable_tile` helper** | Cursor→tile conversion + validation extracted once, called by both systems. |
| **Two entities per tower** | Base never moves; turret rotates. Separate entities = separate transforms. |
| **Preview uses `Visibility`** | Entities live forever, toggled via `Query` loop. Cheaper than spawn/despawn. |
| **No economy** | Towers cost nothing. Resource management in a future part. |

---

## Intentional simplifications

These will be addressed in future parts when the code needs them:

| Simplification | Why | Future direction |
|---|---|---|
| **One tower type** | Only one type exists; no selection UI or cost system. | A future part will add a tower palette, economy, and per-type data. |
| **Hardcoded sprite indices** | Constants `TOWER_BASE = 180`, `TOWER_TOP = 203`. | Future rocket launchers may need per-type spawn functions with different entity counts and layer orders. |
| **Preview = placed tower sprites** | The ghost uses the same #180/#203 as the final tower. | For complex multi-sprite towers (rockets), the preview may use a pre-combined sprite while placed towers need separate entities. |
| **No turret rotation** | `TowerTurret` entities exist but don't rotate. | Part 9: targeting system queries `With<TowerTurret>` and applies rotation. |
| **No projectile sprites** | Fire effect (#295) and rockets (#252) are not rendered. | Part 9+: spawn, animate, and track projectiles from turret to enemy. |

---

## Recap

In this part we:

1. Created `src/tower.rs` with `Tower`, `TowerTurret`, `TowerPreview`, `TowerAtlas`, and `PlacedTowers`.
2. Extracted `hovered_placeable_tile` — a shared cursor-to-valid-tile helper used by both placement systems.
3. Added `setup_tower_atlas` to preload the tower texture/atlas layout at startup.
4. Built a **hover preview** system that shows tinted ghosts of both base and turret on valid grass tiles.
5. Built a **click-to-place** system that spawns **two entities** (base + turret) and updates the occupancy set.
6. Ensured towers cannot be placed on paths, off-map, or on already-occupied tiles.

In **Part 9** we will add targeting: each turret finds the nearest enemy in range, rotates toward it, and fires projectiles.
