# Part 7: Enemies and Path Following

In Part 6 we loaded levels from external TOML files. In this part we finally get things moving on the map: enemies spawn at the entrance, follow the path waypoints to the base, and face the direction they are traveling. We also refactor our startup logic so data loading and visual spawning live in separate systems.

---

## What we will build

- A **single test enemy** (a soldier sprite) that spawns at the first waypoint.
- **Path-following logic** that moves the enemy along the waypoints defined in `level_01.toml`.
- **Facing direction** — the soldier sprite rotates to point where it is going.
- **Cleanup** — when the enemy reaches the last waypoint, it is despawned.

We intentionally keep spawning simple (one enemy at startup) because the focus of this part is the movement system. Timed wave spawning comes later.

---

## Splitting the monolithic startup system

Our `setup` function in Part 6 did everything: load files, build data structures, spawn the camera, and create the entire tilemap. As the game grows, this becomes hard to reason about. More importantly, we want the level data available as ECS resources *before* any rendering system runs.

We split `setup` into two startup systems and chain them:

```rust
.add_systems(Startup, (load_level_data, spawn_tilemap).chain())
```

### `load_level_data`

```rust
fn load_level_data(mut commands: Commands) {
    let level = load_level("assets/levels/level_01.toml");
    let map = build_map_from_level(&level);
    let rules = build_rules();

    commands.insert_resource(map);
    commands.insert_resource(rules);
    commands.insert_resource(level);
}
```

This system has no rendering dependencies. It purely reads files, constructs Rust data, and inserts `MapLayout`, `TileRules`, and `LevelData` into the ECS as resources.

### `spawn_tilemap`

```rust
fn spawn_tilemap(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    map: Res<MapLayout>,
    rules: Res<TileRules>,
) {
    commands.spawn(Camera2d);

    let texture_handle: Handle<Image> =
        asset_server.load("Tilesheet/towerDefense_tilesheet.png");

    // ... tilemap spawning (same as Part 6) ...
}
```

This system consumes the resources produced by the first system. Because they are chained with `.chain()`, Bevy guarantees `load_level_data` finishes before `spawn_tilemap` begins.

### Why this matters

Separating data from rendering is not just aesthetics. In future parts we will:

- Reload a level without restarting the app.
- Show a loading screen while `load_level_data` runs.
- Skip rendering entirely if the level file is malformed.

All of these are easier when loading and spawning are independent systems.

---

## Spawning an enemy sprite

Our enemy is a moving entity, not a static tile. We render it the same way we rendered tiles in Part 2: as a sprite with a texture atlas. The soldier sprite lives at atlas index `245` in the Kenney tilesheet.

### `spawn_test_enemy`

```rust
fn spawn_test_enemy(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    level: Res<LevelData>,
) {
    let texture = asset_server.load("Tilesheet/towerDefense_tilesheet.png");
    let layout = TextureAtlasLayout::from_grid(UVec2::splat(64), 23, 13, None, None);
    let atlas_layout = texture_atlas_layouts.add(layout);

    let waypoints = &level.paths["main_road"].waypoints;
    let spawn_tile = waypoints[0];
    let target_tile = waypoints[1];

    let tile_size = 64.0;
    let map_width = level.map.width as f32;
    let map_height = level.map.height as f32;
    let origin_x = -map_width * tile_size / 2.0 + tile_size / 2.0;
    let origin_y = -map_height * tile_size / 2.0 + tile_size / 2.0;

    let x = origin_x + spawn_tile[0] as f32 * tile_size;
    let y = origin_y + spawn_tile[1] as f32 * tile_size;
    let target = Vec2::new(
        origin_x + target_tile[0] as f32 * tile_size,
        origin_y + target_tile[1] as f32 * tile_size,
    );

    commands.spawn((
        Sprite::from_atlas_image(
            texture,
            TextureAtlas {
                layout: atlas_layout,
                index: 245,
            },
        ),
        Transform::from_xyz(x, y, 1.0),
        Enemy,
        PathFollower {
            path_id: "main_road".to_string(),
            waypoint_index: 1,
            target,
        },
        MoveSpeed(192.0),
    ));
}
```

### Key details

- **`z = 1.0`** places the enemy above the tilemap (which is at `z = 0`).
- **`waypoint_index = 1`** means the enemy is already heading *toward* the second waypoint. The first waypoint is where it starts.
- **`target: Vec2`** stores the world position of the target waypoint directly in the component. This avoids a HashMap lookup and tile-to-world conversion every frame.
- **`MoveSpeed(192.0)`** is `3 tiles/second × 64 pixels/tile`. Speed is in world units so the movement code never needs to know about tile sizes.

### `tile_to_world`

This helper converts grid coordinates to world positions using the same centering math from Part 2:

```rust
pub fn tile_to_world(tile: [u32; 2], map_width: f32, map_height: f32) -> Vec2 {
    let tile_size = 64.0;
    let origin_x = -map_width * tile_size / 2.0 + tile_size / 2.0;
    let origin_y = -map_height * tile_size / 2.0 + tile_size / 2.0;
    Vec2::new(
        origin_x + tile[0] as f32 * tile_size,
        origin_y + tile[1] as f32 * tile_size,
    )
}
```

We make it `pub` because towers (Part 8) will need to convert tile coordinates to world positions for range checks and projectile targeting.

---

## The enemy module

We create `src/enemy.rs` to hold all enemy-related logic.

### Components

```rust
#[derive(Component)]
pub struct Enemy;

#[derive(Component)]
pub struct PathFollower {
    pub path_id: String,
    pub waypoint_index: usize,
    pub target: Vec2,
}

#[derive(Component)]
pub struct MoveSpeed(pub f32);
```

- **`Enemy`** is a plain marker. Future systems (tower targeting, projectile collision) will query `With<Enemy>` without caring about path data.
- **`PathFollower`** holds the navigation state. When the enemy reaches the base, we *remove* this component rather than despawning immediately. This lets a separate system handle cleanup and (later) deduct player lives.
- **`MoveSpeed`** is a new type wrapper around `f32`. This is an ECS idiom: it makes the value queryable by type and self-documenting.

### `move_enemies`

```rust
pub fn move_enemies(
    mut commands: Commands,
    mut query: Query<(Entity, &mut PathFollower, &mut Transform, &MoveSpeed), With<Enemy>>,
    level: Res<LevelData>,
    time: Res<Time>,
) {
    let map_width = level.map.width as f32;
    let map_height = level.map.height as f32;

    for (entity, mut follower, mut transform, speed) in query.iter_mut() {
        let current = transform.translation.truncate();
        let to_target = follower.target - current;
        let distance = to_target.length();

        if distance <= 0.01 {
            // Snap distance: avoid division by zero / atan2 instability when
            // the enemy is already on (or extremely close to) the target tile.
            advance_waypoint(&mut commands, entity, &mut follower, &level, map_width, map_height);
            continue;
        }

        let direction = to_target / distance;
        let step = speed.0 * time.delta_secs();
        let angle = direction.y.atan2(direction.x);

        if distance <= step {
            // Reach (or would overshoot) the waypoint this frame.
            transform.translation.x = follower.target.x;
            transform.translation.y = follower.target.y;
            transform.rotation = Quat::from_rotation_z(angle);
            advance_waypoint(&mut commands, entity, &mut follower, &level, map_width, map_height);
        } else {
            let new_pos = current + direction * step;
            transform.translation.x = new_pos.x;
            transform.translation.y = new_pos.y;
            transform.rotation = Quat::from_rotation_z(angle);
        }
    }
}
```

### Movement strategy

Notice what this system does **not** do: it does not store a `progress` field or compute how far along a segment the enemy has traveled. Instead, it treats the enemy's `Transform` as the single source of truth.

Every frame:
1. Read current world position from `Transform`.
2. Compute vector to `target`.
3. If close enough (`<= 0.01`), snap and advance to the next waypoint.
4. Otherwise, move `speed * dt` units toward the target and rotate to face it.

This design is resilient to external forces. In Part 8, towers might apply knockback by modifying the `Transform` directly. A progress-based system would desync; a position-based system recovers naturally on the next frame.

### Why not carry over overshoot?

When `distance <= step`, we snap to the waypoint and advance. We do not carry the remaining distance into the next segment. At 60 FPS with a speed of 192 units/sec, the maximum unspent distance per frame is ~3.2 units (5% of a tile). The visual difference is imperceptible, and the code is much simpler to follow.

### Facing direction

The soldier sprite in the Kenney pack faces right by default. We rotate it with:

```rust
let angle = direction.y.atan2(direction.x);
transform.rotation = Quat::from_rotation_z(angle);
```

`atan2(y, x)` returns the angle from the positive X axis. For a sprite facing right:
- Moving right → `0` radians → no rotation
- Moving up → `π/2` radians → 90° counter-clockwise
- Moving left → `π` radians → 180°
- Moving down → `-π/2` radians → 90° clockwise

### `advance_waypoint`

```rust
fn advance_waypoint(
    commands: &mut Commands,
    entity: Entity,
    follower: &mut PathFollower,
    level: &LevelData,
    map_width: f32,
    map_height: f32,
) {
    let waypoints = &level.paths[&follower.path_id].waypoints;
    follower.waypoint_index += 1;

    if follower.waypoint_index >= waypoints.len() {
        commands.entity(entity).remove::<PathFollower>();
    } else {
        follower.target = tile_to_world(waypoints[follower.waypoint_index], map_width, map_height);
    }
}
```

When the last waypoint is reached, we remove `PathFollower`. The enemy stops moving and becomes eligible for cleanup.

### `cleanup_finished_enemies`

```rust
pub fn cleanup_finished_enemies(
    mut commands: Commands,
    query: Query<Entity, (With<Enemy>, Without<PathFollower>)>,
) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}
```

This system runs in `FixedUpdate` alongside `move_enemies`. It queries for entities that have the `Enemy` marker but no longer have `PathFollower` — meaning they reached the base. In future parts, this same query will be used to deduct player lives before despawning.

---

## Wiring it all together

```rust
fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .add_plugins(TilemapPlugin)
        .add_systems(Startup, (load_level_data, spawn_tilemap, spawn_test_enemy).chain())
        .add_systems(FixedUpdate, (move_enemies, cleanup_finished_enemies))
        .run();
}
```

The three startup systems are chained so they execute in order: data → tilemap → enemy. `move_enemies` and `cleanup_finished_enemies` both run in `FixedUpdate` at a fixed timestep independent of frame rate.

---

## Running the project

```bash
cargo run
```

You should see:

- The same `15×10` map from Part 6.
- A **soldier sprite** (index 245) at the spawn point `(2, 9)`.
- The soldier moves down to `y=5`, then right to `x=12`, then down to `y=1`.
- The sprite **rotates** to face its current direction of travel.
- The soldier **disappears** when it reaches the final waypoint.

At `192.0` units/sec, the full journey takes roughly 6 seconds.

---

## Design decisions

| Decision | Rationale |
|---|---|
| **Position-based movement** | Transform is the single source of truth. Knockback and other external forces work without special handling. |
| **Store `target: Vec2` in component** | Avoids HashMap lookup + tile-to-world conversion every frame. Only updated at waypoint crossings. |
| **Remove `PathFollower` instead of despawning** | Decouples "reached base" detection from "what happens next." Future parts will deduct lives here. |
| **Newtype `MoveSpeed(f32)`** | Self-documenting and queryable by type in the ECS. |
| **Cleanup in `FixedUpdate`** | Movement and cleanup share the same schedule — gameplay logic stays together. |
| **One test enemy only** | Validates the system without adding wave-scheduling complexity. Waves are a separate design problem for a future part. |

---

## Recap

In this part we:

1. **Refactored startup** into `load_level_data` and `spawn_tilemap` — separated data loading from rendering.
2. **Spawned an enemy sprite** using `Sprite::from_atlas_image`, reusing the atlas technique from Part 2.
3. Created `src/enemy.rs` with `Enemy`, `PathFollower`, and `MoveSpeed` components.
4. Implemented **position-based path following** — the enemy moves toward a stored `Vec2` target, advances waypoints on arrival, and rotates to face its direction.
5. Added **cleanup** via the `Without<PathFollower>` query pattern, running in `FixedUpdate` alongside movement.
6. Made `tile_to_world` public for reuse by towers in Part 8.

In **Part 8** we will add towers that players can place on grass tiles, with range detection and projectiles that home in on enemies.
