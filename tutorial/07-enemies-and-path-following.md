# Part 7: Enemies and Path Following

> **Time to read:** ~30 minutes  
> **New concepts:** Systems, Queries, `Update` / `FixedUpdate`, system chaining, position-based movement  
> **Prerequisite:** Part 6 (external TOML levels with `LevelData`, `MapLayout`, and auto-tiling)

---

## Recap: What We Already Have

Our level loads from an external TOML file. `setup()` reads `level_01.toml`, builds a `MapLayout` with width-3 path expansion, spawns the tilemap with auto-tiling, and inserts `LevelData` as an ECS resource. Everything is static — the map renders correctly, but nothing moves.

---

## Goal: What We Will Build

We will add moving enemies that follow the path waypoints from spawn to base:

1. **Split `setup()`** into two chained startup systems: one for data loading, one for tilemap spawning.
2. **Spawn a test enemy** as a sprite at the first waypoint.
3. **Implement path following** with position-based movement toward stored `Vec2` targets.
4. **Rotate the sprite** to face its direction of travel.
5. **Despawn enemies** that reach the final waypoint.

We intentionally spawn only one enemy because the focus is the movement system. Wave scheduling comes in a later part.

---

## New Bevy APIs & Concepts

### Systems

A **system** in Bevy is just a function with special parameters. When you register it with `.add_systems(Update, my_system)`, Bevy calls that function every frame, passing in the current state of the world. The parameters tell Bevy what data the system needs:

- `mut commands: Commands` — spawn or despawn entities, insert resources.
- `mut query: Query<&mut Transform>` — find all entities with a `Transform` component.
- `time: Res<Time>` — read the global time resource.

Until now, we have only used `setup` as a one-shot startup function. In this part, `move_enemies` runs every `FixedUpdate` tick and actually queries the world for entities to modify. That is the heart of ECS programming: systems process sets of entities based on their components.

### Queries

A **query** is how a system asks the ECS "give me all entities that have these components." Bevy returns an iterator you can loop over.

```rust
Query<(Entity, &mut Transform), With<Enemy>>
```

This query matches every entity that has:
- An `Entity` ID (always available)
- A `Transform` component (mutable, so we can move the sprite)
- The `Enemy` marker component (`With<Enemy>` is a query filter)

**Pitfall — the reader-writer rule:** Bevy borrows component data just like Rust borrows references. A system can have any number of *immutable* readers of a component, or *one* mutable writer — but never multiple writers, nor a writer and a reader at the same time. This applies per-component-type within a single system.

```rust
// OK: many immutable references
Query<&Transform, With<Enemy>>
Query<&Transform, With<Tower>>

// OK: one mutable reference
Query<&mut Transform, With<Enemy>>

// NOT OK: two mutable references
Query<&mut Transform, With<Enemy>>
Query<&mut Transform, With<Tower>>

// NOT OK: one mutable + one immutable
Query<&mut Transform, With<Enemy>>
Query<&Transform, With<Tower>>   // Transform is borrowed mutably above
```

If you need to modify `Transform` on both enemies and towers in the same system, use a single query with an `Or` filter or split the logic into two systems.

This is a hard topic. The exact rules have subtleties — Bevy can sometimes prove non-overlapping queries are safe, and the borrow checker interacts with query filters in ways that take practice to predict. It is fine if the details do not click yet. The only thing you need to carry forward is: **queries are how systems ask Bevy for world state**, and the borrow-like rules exist to prevent data races. We will revisit this with concrete examples in later parts.

### `Update` and `FixedUpdate`

Every system you register must belong to a **schedule** — a named collection of systems that Bevy runs at a specific point in the frame. The two schedules you will use most often are:

- **`Update`** — runs once per rendered frame. Its delta time varies with the frame rate (16 ms at 60 FPS, 33 ms at 30 FPS).
- **`FixedUpdate`** — runs at a fixed timestep (60 Hz by default), independent of the renderer. If the game drops to 30 FPS, `FixedUpdate` still runs twice per frame to catch up.

If you register `move_enemies` in `Update`, a stutter (frame taking 100 ms) would make the enemy jump forward by `speed * 0.1` in a single frame. In `FixedUpdate`, the same stutter is handled as six 16 ms ticks, producing six small, smooth steps.

```rust
.add_systems(Update, animate_ui)        // tied to frame rate
.add_systems(FixedUpdate, move_enemies) // deterministic timestep
```

This is the same distinction Unity makes with `Update()` (frame-rate dependent) and `FixedUpdate()` (physics timestep). The difference is that in Bevy you pick the schedule explicitly when you register the system; there is no implicit default.

**When to use which:**
- Use `FixedUpdate` for gameplay logic that must be deterministic: movement, collision, damage ticks.
- Use `Update` for visual polish that should feel smooth at high frame rates: UI animations, camera follow, particle effects.

### System Chaining

Systems registered in the same schedule run in parallel by default. You can force ordering with `.chain()`:

```rust
.add_systems(Startup, (load_data, spawn_tilemap, spawn_enemy).chain())
```

Bevy guarantees `load_data` finishes before `spawn_tilemap` begins, and `spawn_tilemap` finishes before `spawn_enemy` begins. This is essential when later systems depend on resources inserted by earlier ones.

You can also chain logical systems in `Update` or `FixedUpdate` when order matters for correctness. For example, you might chain `apply_damage` before `despawn_dead_enemies` so the death check sees the final health value.

### Position-Based Movement

There are two common ways to move an entity along a path:

1. **Progress-based:** Store a float `progress` (0.0 to 1.0) along the current segment. Compute position from progress every frame.
2. **Position-based:** Store the target position. Move toward it every frame using the current `Transform` as the single source of truth.

We choose position-based because it is more flexible. If you later add knockback, slows, or dashes, the movement system recovers naturally on the next frame because `Transform` is the single source of truth. A progress-based system would need special handling for every external force.

**Pitfall:** Do not store both progress and position and try to keep them in sync. Pick one source of truth. Storing both creates a maintenance burden — any code that moves the entity must update both values, or they drift apart.

---

## Walkthrough

### Step 1: Split the startup system

Our `setup()` in Part 6 did everything: load files, build data structures, spawn the camera, and create the entire tilemap. As the game grows, this becomes hard to reason about. More importantly, we want level data available as ECS resources *before* any rendering system runs.

We split `setup` into two startup systems and chain them:

```rust
.add_systems(Startup, (load_level_data, spawn_tilemap).chain())
```

#### `load_level_data`

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

#### `spawn_tilemap`

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

**Why this matters:** In future parts we will reload a level without restarting the app, show a loading screen while `load_level_data` runs, or skip rendering if the level file is malformed. All of these are easier when loading and spawning are independent systems.

---

### Step 2: Design the enemy module

Before writing code, let's decide what an enemy *is* in ECS terms.

An enemy needs:
- **A sprite** — it is a visible object on screen. Bevy provides `Sprite` and `Transform` for this.
- **An identity** — towers and projectiles must be able to say "that thing is an enemy." A marker component `Enemy` serves this purpose.
- **Navigation state** — it must know which path to follow, which waypoint it is heading toward, and the world position of that waypoint. We group this into a `PathFollower` component.

> **Design decision: pre-compute `target`.** You could derive the target position every frame from `path_id` and `waypoint_index` via a HashMap lookup and `tile_to_world` conversion. But `move_enemies` runs every tick on every enemy, so we store the world-space `Vec2` directly in `PathFollower`. This trades a small amount of memory for a measurable performance win — no lookups, no conversions, just a vector subtraction every frame.
- **Speed** — different enemies may move at different speeds. A `MoveSpeed` component holds this value.

We also need systems that act on these components:
- `move_enemies` — runs every `FixedUpdate` tick, reads `PathFollower.target` and `MoveSpeed`, and updates `Transform` accordingly.
- `advance_waypoint` — a helper called when an enemy reaches its target; increments the waypoint index and computes the next target position.
- `cleanup_finished_enemies` — runs after movement, despawns enemies that have reached the final waypoint.

Create `src/enemy.rs`:

```rust
use bevy::prelude::*;
use crate::level::LevelData;

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
- **`MoveSpeed`** is a newtype wrapper around `f32`. This is an ECS idiom: it makes the value queryable by type and self-documenting.

---

### Step 3: Spawn a test enemy

Now we need a startup system that creates the enemy entity. What does it need to do?

1. **Load the sprite** — we reuse the same Kenney tilesheet from Part 2, so we need `AssetServer` and `TextureAtlasLayout`.
2. **Find the spawn point** — read `level.paths["main_road"].waypoints[0]` from `LevelData`.
3. **Convert to world space** — the waypoints are grid coordinates; the sprite needs a world-space `Vec3` for its `Transform`.
4. **Compute the first target** — the enemy starts at waypoint 0 and immediately heads toward waypoint 1. We pre-compute the world position of waypoint 1 and store it in `PathFollower.target`.
5. **Spawn the entity** — bundle `Sprite`, `Transform`, `Enemy`, `PathFollower`, and `MoveSpeed` together.

Add this as a third chained startup system in `main.rs`:

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

**Key details:**

- **`z = 1.0`** places the enemy above the tilemap (which is at `z = 0`).
- **`waypoint_index = 1`** means the enemy is already heading *toward* the second waypoint. The first waypoint is where it starts.
- **`MoveSpeed(192.0)`** is `3 tiles/second × 64 pixels/tile`. Speed is in world units so the movement code never needs to know about tile sizes.

---

### Step 4: Add `tile_to_world`

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

### Step 5: Implement path following

Now for the core movement system. What must it do every tick?

1. **Query all enemies** — find every entity with `Enemy`, `PathFollower`, `Transform`, and `MoveSpeed`.
2. **Compute direction** — subtract current position (from `Transform`) from `PathFollower.target` to get the vector to travel.
3. **Handle arrival** — if the enemy is already at (or extremely close to) the target, advance to the next waypoint immediately.
4. **Move and rotate** — otherwise, move `speed * dt` units toward the target and set `Transform.rotation` so the sprite faces its direction of travel.
5. **Handle overshoot** — if the enemy would reach or pass the target this frame, snap to the target position, then advance the waypoint.

Add `move_enemies` to `src/enemy.rs`:

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

**Movement strategy:**

Notice what this system does **not** do: it does not store a `progress` field or compute how far along a segment the enemy has traveled. Instead, it treats the enemy's `Transform` as the single source of truth.

Every frame:
1. Read current world position from `Transform`.
2. Compute vector to `target`.
3. If close enough (`<= 0.01`), snap and advance to the next waypoint.
4. Otherwise, move `speed * dt` units toward the target and rotate to face it.

This design is resilient to external forces. If you later add knockback, slows, or dashes, they can modify the `Transform` directly and the movement system recovers naturally on the next frame. A progress-based system would desync.

**Why not carry over overshoot?**

When `distance <= step`, we snap to the waypoint and advance. We do not carry the remaining distance into the next segment, nor do we try to land exactly on the waypoint — a tiny overshoot is fine. At 60 FPS with a speed of 192 units/sec, the maximum overshoot per frame is ~3.2 units (5% of a tile). The enemy moves slowly compared to the tick rate, so the visual difference is imperceptible, and the code is much simpler to follow.

**Facing direction:**

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

---

### Step 6: Advance waypoints and clean up

When an enemy reaches its target, we need to advance it to the next waypoint. If there are no more waypoints, the enemy has reached the base. Instead of despawning immediately, we remove the `PathFollower` component. This decouples "reached base" from "what happens next" — a future system can deduct lives before cleanup runs.

#### `advance_waypoint`

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

A separate cleanup system runs after movement in the same `FixedUpdate` schedule. It queries for entities that still have `Enemy` but no longer have `PathFollower` — the exact state produced when `advance_waypoint` removes the component at the final waypoint.

#### `cleanup_finished_enemies`

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

This system queries for entities that have the `Enemy` marker but no longer have `PathFollower` — meaning they reached the base. In future parts, this same query will be used to deduct player lives before despawning.

---

### Step 7: Wire everything together

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

### Step 8: Verify

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

## Simplifications and future work

| Simplification | Future extension |
|---|---|
| **One test enemy only** | Wave scheduling with spawn timers and multiple enemy types. |
| **No health / lives** | Enemies reach the base and despawn. Future parts will deduct player lives. |
| **No collision** | Enemies walk through each other. In a dense swarm, this is acceptable for a 2D tower defense. |
| **Fixed speed** | Different enemy types will have different `MoveSpeed` values. |
| **No spawn animation** | Enemies appear instantly. A fade-in or drop-in animation could be added. |

---

## Summary

- We **refactored startup** into `load_level_data` and `spawn_tilemap` — separated data loading from rendering.
- We **spawned an enemy sprite** using `Sprite::from_atlas_image`, reusing the atlas technique from Part 2.
- We created `src/enemy.rs` with `Enemy`, `PathFollower`, and `MoveSpeed` components.
- We implemented **position-based path following** — the enemy moves toward a stored `Vec2` target, advances waypoints on arrival, and rotates to face its direction.
- We added **cleanup** via the `Without<PathFollower>` query pattern, running in `FixedUpdate` alongside movement.
- We made `tile_to_world` public for reuse by towers in Part 8.

In **Part 8** we will add towers that players can place on grass tiles by clicking. Targeting and damage come in Part 9; projectiles come in a later part.
