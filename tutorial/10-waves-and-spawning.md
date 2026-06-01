# Part 10: Waves and Enemy Spawning

> **Time to read:** ~25 minutes  
> **New concepts:** `VecDeque`, `Time::delta_secs()` in `FixedUpdate`, `#[serde(default)]`  
> **Prerequisite:** Part 9 (targeting and damage with a single test enemy)

---

## Recap: What We Already Have

Towers shoot enemies, dealing damage on a cooldown and despawning them when health reaches zero. But we only have one hardcoded test enemy that spawns at startup. There is no concept of enemy variety, timed waves, or multiple simultaneous spawns.

---

## Goal: What We Will Build

We will replace the single test enemy with a data-driven wave system defined entirely in the level TOML:

1. **Enemy types** — define sprite, speed, and health for each type in TOML.
2. **Wave definitions** — each wave has a start time, path, spawn interval, and a list of enemy groups.
3. **Flattened spawn schedule** — at load time, all waves are expanded into a single sorted queue of spawn events. One system consumes events as time passes.
4. **Overlapping waves** — two waves can interleave naturally because the schedule is sorted by time, not by wave identity.

---

## New Bevy APIs & Concepts

### `VecDeque`

`VecDeque` is Rust's double-ended queue. Unlike `Vec`, which has O(1) `push`/`pop` only at the back, `VecDeque` offers O(1) `push_back`, `pop_back`, `push_front`, and `pop_front`. For a spawn schedule where we repeatedly remove the earliest event, `pop_front()` is ideal — no element shifting, no index bookkeeping.

> **Pitfall:** `VecDeque::pop_front()` returns `Option<T>`. In Bevy, resources like `SpawnSchedule` are never accessed concurrently by multiple systems — the scheduler ensures that two systems mutably borrowing the same resource run sequentially, not in parallel. The `Option` return type is not a concurrency guard; it simply reflects the API design (the queue may be empty). You still need to handle `None` because `pop_front` on an empty queue returns `None`.

### `Time::delta_secs()` in `FixedUpdate`

When a system runs in `FixedUpdate`, `Res<Time>` returns the **fixed timestep** duration (e.g., ~0.0167 seconds at 60 Hz), not the wall-clock frame time. If the renderer drops to 30 FPS, Bevy runs `FixedUpdate` twice per frame to catch up, and `time.delta_secs()` returns the same fixed value each time.

This is exactly what we want for spawn scheduling: `elapsed` accumulates at a constant simulation rate, unaffected by frame rate stutters.

### `#[serde(default)]`

When deserializing with `serde`, `#[serde(default)]` on a field means: if the field is missing from the source data, use the type's `Default` implementation instead of failing. This keeps level files backward-compatible — old levels without `enemy_types` or `waves` still parse successfully.

---

## Walkthrough

### Designing the feature

Before writing code, think about what the player should see and what data that requires.

**Player-visible behavior:**

1. Enemies spawn in groups at timed intervals.
2. Different enemy types move at different speeds and have different health.
3. Multiple waves can overlap — a second wave may start before the first finishes.
4. The game runs indefinitely until all waves are exhausted.

**ECS data needed:**

- TOML schema additions: `enemy_types` table and `waves` array.
- Rust structs: `EnemyTypeDef`, `WaveDef`, `WaveEnemyGroup` for deserialization.
- `SpawnEvent` — a plain data struct holding one enemy spawn (time, sprite, speed, health, path).
- `SpawnSchedule` resource — a `VecDeque` of events plus an `elapsed` accumulator.
- `build_spawn_schedule` — a function that flattens all waves into events and sorts them.
- `spawn_wave_enemies` — a `FixedUpdate` system that pops due events and spawns enemies.
- `setup_spawn_schedule` — a startup system that builds the schedule and inserts it as a resource.

**Design decision: flattened schedule.** There are two common architectures for wave spawning:

| Approach | Runtime state | Overlap logic | Flexibility |
|---|---|---|---|
| **Per-wave entities** | Each wave is an entity tracking its own timer and remaining spawns | Natural — each wave spawns independently when its timer fires | High: can pause, modify, or cancel individual waves at runtime |
| **Flattened schedule** | None — just a queue and an elapsed timer | Implicit when events interleave | Low: schedule is baked at load time; no per-wave identity |

We choose the flattened schedule because we don't need per-wave identity yet (no "wave survived" UI, no per-wave rewards). Flattening is simpler — the spawn system just pops events from a queue without tracking per-wave state — and overlapping waves happen naturally because interleaving is solved by the sort order. Both approaches use a single system; the difference is whether the spawn system queries wave entities for their next spawn time, or peeks at a pre-sorted queue.

### Step 1: Add enemy types and waves to the TOML

Open `assets/levels/level_01.toml` and add two new sections.

**Enemy types** define the four human soldier variants from the Kenney tilesheet:

```toml
[enemy_types.soldier]
sprite = 245
speed = 192.0
health = 100.0

[enemy_types.runner]
sprite = 246
speed = 320.0
health = 60.0

[enemy_types.heavy]
sprite = 247
speed = 96.0
health = 300.0

[enemy_types.scout]
sprite = 248
speed = 160.0
health = 80.0
```

**Waves** define when and what spawns:

```toml
[[waves]]
start_time = 0.0
path = "main_road"
spawn_interval = 1.0
enemies = [
    { type = "soldier", count = 3 },
    { type = "runner", count = 2 },
]

[[waves]]
start_time = 12.0
path = "main_road"
spawn_interval = 1.2
enemies = [
    { type = "soldier", count = 2 },
    { type = "scout", count = 2 },
    { type = "heavy", count = 1 },
]
```

Key fields:
- `start_time` — seconds since the level began. Wave 1 starts immediately; wave 2 at 12 seconds.
- `spawn_interval` — gap between individual enemies within a wave.
- `enemies` — ordered list of groups. Soldiers spawn first, then runners, etc.

### Step 2: Add deserialization structs

In `src/level.rs`, add structs for the new TOML data:

```rust
#[derive(Debug, Deserialize, Clone)]
pub struct EnemyTypeDef {
    pub sprite: usize,
    pub speed: f32,
    pub health: f32,
}

#[derive(Debug, Deserialize)]
pub struct WaveDef {
    pub start_time: f32,
    pub path: String,
    #[serde(default = "default_spawn_interval")]
    pub spawn_interval: f32,
    pub enemies: Vec<WaveEnemyGroup>,
}

fn default_spawn_interval() -> f32 { 1.0 }

#[derive(Debug, Deserialize)]
pub struct WaveEnemyGroup {
    #[serde(rename = "type")]
    pub enemy_type: String,
    pub count: u32,
}
```

`WaveEnemyGroup` uses `#[serde(rename = "type")]` because `type` is a Rust keyword and cannot be a raw field name. The `default_spawn_interval` function provides a fallback when `spawn_interval` is omitted from the TOML.

Update `LevelData` to include the new fields:

```rust
#[derive(Debug, Deserialize, Resource)]
pub struct LevelData {
    pub map: MapData,
    pub paths: HashMap<String, PathData>,
    #[serde(default)]
    pub enemy_types: HashMap<String, EnemyTypeDef>,
    #[serde(default)]
    pub waves: Vec<WaveDef>,
}
```

`#[serde(default)]` means existing level files without `enemy_types` or `waves` will parse successfully — the fields default to empty collections. This keeps old levels backward-compatible.

### Step 3: Define the spawn event and schedule

In `src/enemy.rs`, add a plain data struct for a single spawn event and a resource to hold the schedule:

```rust
use std::collections::VecDeque;

pub struct SpawnEvent {
    time: f32,
    sprite: usize,
    speed: f32,
    health: f32,
    path: String,
}

#[derive(Resource)]
pub struct SpawnSchedule {
    events: VecDeque<SpawnEvent>,
    elapsed: f32,
    texture: Handle<Image>,
    atlas: Handle<TextureAtlasLayout>,
}
```

`SpawnEvent` is not a component — it is never attached to an entity. It exists only as temporary data consumed by the schedule builder. `SpawnSchedule` is a resource because it must persist across frames and be accessed by the spawn system.

> **Why store texture handles in the schedule?** The spawn system needs to create sprites every time an enemy appears. By preloading the texture and atlas layout once at startup and storing the handles in `SpawnSchedule`, we avoid calling `asset_server.load()` every frame.

### Step 4: Build the schedule at load time

Now we write the function that turns the raw wave data into a ready-to-consume queue. What must it do?

1. **Iterate all waves** — for each wave, start at `wave.start_time`.
2. **Expand enemy groups** — for each `{ type, count }` group, look up the `EnemyTypeDef` to get sprite, speed, and health.
3. **Create individual events** — produce one `SpawnEvent` per enemy, advancing a local `time` accumulator by `spawn_interval` after each spawn.
4. **Sort globally** — collect all events from all waves into one `Vec`, then sort by `time`. This is what makes overlapping waves work: events from different waves interleave naturally in the sorted order.
5. **Build the resource** — wrap the sorted events in a `VecDeque`, set `elapsed` to `0.0`, and preload the texture and atlas layout handles so the spawn system never touches `AssetServer`.

Add `build_spawn_schedule` to `src/enemy.rs`:

```rust
pub fn build_spawn_schedule(
    level: &LevelData,
    asset_server: &AssetServer,
    texture_atlas_layouts: &mut Assets<TextureAtlasLayout>,
) -> SpawnSchedule {
    let mut events: Vec<SpawnEvent> = Vec::new();

    for wave in &level.waves {
        let mut time = wave.start_time;
        for group in &wave.enemies {
            let def = &level.enemy_types[&group.enemy_type];
            for _ in 0..group.count {
                events.push(SpawnEvent {
                    time,
                    sprite: def.sprite,
                    speed: def.speed,
                    health: def.health,
                    path: wave.path.clone(),
                });
                time += wave.spawn_interval;
            }
        }
    }

    events.sort_by(|a, b| a.time.total_cmp(&b.time));

    SpawnSchedule {
        events: events.into(),
        elapsed: 0.0,
        texture: asset_server.load("Tilesheet/towerDefense_tilesheet.png"),
        atlas: texture_atlas_layouts.add(
            TextureAtlasLayout::from_grid(UVec2::splat(64), 23, 13, None, None)
        ),
    }
}
```

The nested loops produce one `SpawnEvent` per enemy. The outer wave loop sets the base time; the group loop looks up the enemy type definition; the count loop creates individual events, advancing `time` by `spawn_interval` after each one.

Sorting by `time` is what enables overlapping waves. Consider two waves:

```
Wave A: 0.0 soldier, 1.0 soldier, 2.0 soldier
Wave B: 1.5 runner, 2.5 runner
→ after sort: 0.0 soldier, 1.0 soldier, 1.5 runner, 2.0 soldier, 2.5 runner
```

The spawn system simply processes events in order without knowing which wave they came from.

### Step 5: Spawn enemies from the schedule

Now for the system that actually creates enemies at runtime. What must it do every tick?

1. **Advance the clock** — add `time.delta_secs()` to `schedule.elapsed`.
2. **Peek at the queue front** — check the earliest event's spawn time.
3. **Pop all due events** — while the front event's time ≤ elapsed, remove it from the queue and spawn the enemy.
4. **Spawn with correct stats** — each event carries sprite, speed, and health. The enemy starts at waypoint 0 and immediately targets waypoint 1 (same as the old test enemy).

What does it query?
- `ResMut<SpawnSchedule>` — to advance elapsed time and pop events.
- `Res<LevelData>` — to read waypoints for spawn position and first target.
- `Res<Time>` — to accumulate the fixed timestep.
- `Commands` — to spawn enemy entities with the right components.

Add `spawn_wave_enemies` to `src/enemy.rs`:

```rust
pub fn spawn_wave_enemies(
    mut schedule: ResMut<SpawnSchedule>,
    mut commands: Commands,
    level: Res<LevelData>,
    time: Res<Time>,
) {
    schedule.elapsed += time.delta_secs();

    while let Some(event) = schedule.events.front() {
        if event.time > schedule.elapsed {
            break;
        }
        let event = schedule.events.pop_front().unwrap();

        let waypoints = &level.paths[&event.path].waypoints;
        let spawn_tile = waypoints[0];
        let target_tile = waypoints[1];

        let map_width = level.map.width as f32;
        let map_height = level.map.height as f32;
        let tile_size = 64.0;
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
                schedule.texture.clone(),
                TextureAtlas {
                    layout: schedule.atlas.clone(),
                    index: event.sprite,
                },
            ),
            Transform::from_xyz(x, y, 1.0),
            Enemy,
            PathFollower {
                path_id: event.path,
                waypoint_index: 1,
                target,
            },
            MoveSpeed(event.speed),
            Health(event.health),
        ));
    }
}
```
The `while let` loop handles multiple enemies spawning on the same tick. If `spawn_interval` is shorter than the fixed timestep, several events may become due simultaneously.

### Step 6: Wire everything in `main.rs`

Replace the old enemy imports:

```rust
use enemy::{
    build_spawn_schedule, spawn_wave_enemies,
    move_enemies, cleanup_finished_enemies,
};
```

`Enemy`, `Health`, `PathFollower`, and `MoveSpeed` are no longer used directly in `main.rs` — they are only consumed internally by `spawn_wave_enemies`.

Add a startup system that builds the schedule:

```rust
fn setup_spawn_schedule(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    level: Res<LevelData>,
) {
    let schedule = build_spawn_schedule(
        &level, &asset_server, &mut texture_atlas_layouts,
    );
    commands.insert_resource(schedule);
}
```

Update the startup chain to include `setup_spawn_schedule` (replacing `spawn_test_enemy`):

```rust
.add_systems(Startup, (
    load_level_data,
    setup_tower_atlas,
    setup_spawn_schedule,
    spawn_tilemap,
    spawn_placement_preview,
).chain())
```

Update the `FixedUpdate` chain to include `spawn_wave_enemies` first:

```rust
.add_systems(FixedUpdate, (
    spawn_wave_enemies,
    move_enemies,
    attack_enemies,
    cleanup_finished_enemies,
).chain())
```

`spawn_wave_enemies` must run before `move_enemies`. If a spawn happened after movement, the new enemy would sit motionless for one frame. With `.chain()`, newly spawned enemies are picked up by `move_enemies` in the same tick.

Delete the old `spawn_test_enemy` function — all its logic has moved into `spawn_wave_enemies`.

### Step 7: Verify

```bash
cargo run
```

You should see:

- The same `15×10` map and click-to-place from Part 9.
- **Wave 1 starts immediately** — 3 soldiers (slow, 100 HP) followed by 2 runners (fast, 60 HP). One spawns every 1.0 seconds.
- **At t = 12.0, wave 2 starts** — 2 soldiers, 2 scouts (medium speed, 80 HP), then 1 heavy (very slow, 300 HP). Spawn interval 1.2 seconds.
- **Multiple enemies can be on screen simultaneously.** They move independently; towers target the nearest.
- The game runs indefinitely — waves complete, enemies die or reach the base, no "game over" trigger yet.

### Simplifications

| Simplification | Why it works | Future direction |
|---|---|---|
| **One path only** | All waves use `"main_road"`. | Future levels may have multiple paths; `path` field is already in the TOML schema. |
| **No spawn limit** | Waves spawn until exhausted; no cap on living enemies. | A spawn cap or pool could prevent memory issues with dense swarms. |
| **No wave completion feedback** | The player sees enemies stop spawning. | UI notifications, per-wave rewards, or "wave survived" tracking in a future part. |
| **No difficulty scaling** | Enemy stats are static per type. | Future parts could scale health or speed based on wave number. |
| **Preloaded texture in schedule** | Avoids `asset_server.load()` per spawn. | A more robust asset system might use `Handle<Image>` references from a central asset manager. |
| **Linear scan for targeting** | Still O(n) per turret from Part 9. | A dense swarm would need a spatial hash for efficient nearest-enemy search. |

---

## Summary

- We defined 4 **enemy types** (soldier, runner, heavy, scout) with sprite, speed, and health in `level_01.toml`.
- We defined **waves** with start times, spawn intervals, and ordered enemy groups.
- We added `EnemyTypeDef`, `WaveDef`, and `WaveEnemyGroup` to `src/level.rs` for deserialization, using `#[serde(default)]` for backward compatibility.
- We built a **flattened spawn schedule** — all waves expanded into a sorted `VecDeque<SpawnEvent>` at load time.
- We replaced `spawn_test_enemy` with `spawn_wave_enemies`, a `FixedUpdate` system that pops due events and spawns enemies with the correct stats.
- We chained `spawn_wave_enemies` before `move_enemies` so fresh enemies move on their first tick.
- We demonstrated **overlapping waves** via interleaved event times in the sorted queue.

In **Part 11** we will add win/lose conditions: the base will have hit points, reaching enemies will cost lives, and the game will end when lives run out or all enemies are defeated.
