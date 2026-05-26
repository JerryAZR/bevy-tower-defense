# Part 10: Waves and Enemy Spawning

Part 9 gave us the core combat loop. But we only had one hardcoded test enemy. In this part, we replace the manual enemy spawn with a data-driven wave system — multiple enemy types, timed waves, overlapping spawns, all defined in the level TOML.

---

## What we will build

- **Enemy types** defined in TOML — sprite, speed, health. Four types: soldier, runner, heavy, scout.
- **Wave definitions** — each wave has a start time, path, spawn interval, and a list of `{ type, count }` enemy groups.
- **Flattened spawn schedule** — all waves are expanded at load time into a sorted `VecDeque<SpawnEvent>`. One system consumes events as time passes.
- **Overlapping waves** — two waves can interleave their spawns naturally because the schedule is sorted by time.

---

## Why flatten waves at load time?

There are two common architectures:

| Approach | Per-wave state | Overlap logic | Systems needed |
|---|---|---|---|
| Per-wave entities | Each wave is an entity with its own timer | Must coordinate across entities | Multiple |
| **Flattened schedule** | None — just a queue | Implicit when events interleave | One |

We don't need per-wave identity yet (no "wave survived" UI, no per-wave rewards). Flattening is simpler and handles overlapping waves for free.

---

## TOML schema

### Enemy types

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

Four enemy types using the 4 single-sprite human characters from the Kenney tilesheet. The only differences are speed and health — same components, different values.

### Waves

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

Key details:
- `start_time` is seconds since the level began. Wave 1 starts immediately; wave 2 at 12 seconds.
- `spawn_interval` is the gap between individual enemies within a wave.
- `enemies` is an ordered list — soldier group spawns first, then runner, etc.

---

## Rust structures (`src/level.rs`)

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

`LevelData` gains two new fields:

```rust
#[serde(default)]
pub enemy_types: HashMap<String, EnemyTypeDef>,

#[serde(default)]
#[allow(dead_code)]
pub waves: Vec<WaveDef>,
```

The `#[serde(default)]` means existing levels without these fields won't break.

`WaveEnemyGroup` uses `#[serde(rename = "type")]` because `type` is a Rust keyword.

---

## Spawn schedule (`src/enemy.rs`)

### The event struct

```rust
pub struct SpawnEvent {
    time: f32,
    sprite: usize,
    speed: f32,
    health: f32,
    path: String,
}
```

Not a component, not a resource — just a plain data struct consumed by the schedule builder.

### The schedule resource

```rust
#[derive(Resource)]
pub struct SpawnSchedule {
    events: VecDeque<SpawnEvent>,
    elapsed: f32,
    texture: Handle<Image>,
    atlas: Handle<TextureAtlasLayout>,
}
```

`VecDeque` gives O(1) `pop_front()`. The texture and atlas are preloaded so the spawn system doesn't re-load the tilesheet every frame.

### Building the schedule

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

Flattens all waves into events, sorts by time. For two overlapping waves:

```
Wave A: 0.0 soldier, 1.0 soldier, 2.0 soldier
Wave B: 1.5 runner, 2.5 runner
→ after sort: 0.0 soldier, 1.0 soldier, 1.5 runner, 2.0 soldier, 2.5 runner
```

### The spawn system

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

        // ... compute position from waypoints ...

        commands.spawn((
            Sprite::from_atlas_image(
                schedule.texture.clone(),
                TextureAtlas { layout: schedule.atlas.clone(), index: event.sprite },
            ),
            Transform::from_xyz(x, y, 1.0),
            Enemy,
            PathFollower { path_id: event.path, waypoint_index: 1, target },
            MoveSpeed(event.speed),
            Health(event.health),
        ));
    }
}
```

`elapsed` is driven by `Time::delta_secs()` — independent of the game's fixed timestep. It runs in `FixedUpdate`, so frame drops won't cause spawn bursts.

---

## Wiring (`src/main.rs`)

### Imports

Replace the old enemy imports with:

```rust
use enemy::{build_spawn_schedule, spawn_wave_enemies, move_enemies, cleanup_finished_enemies};
```

`Enemy`, `Health`, `PathFollower`, `MoveSpeed` are no longer used directly in `main.rs` — they're only consumed by `spawn_wave_enemies` internally.


### Startup chain

```rust
.add_systems(Startup, (
    load_level_data,
    setup_tower_atlas,
    setup_spawn_schedule,    // new — replaces spawn_test_enemy
    spawn_tilemap,
    spawn_placement_preview,
).chain())
```

`setup_spawn_schedule` calls `build_spawn_schedule` and inserts the result as a resource.

### FixedUpdate chain

```rust
.add_systems(FixedUpdate, (
    spawn_wave_enemies,      // new — must run before move_enemies
    move_enemies,
    attack_enemies,
    cleanup_finished_enemies,
).chain())
```

`spawn_wave_enemies` goes first so newly spawned enemies are picked up by `move_enemies` in the same tick. Without `.chain()`, a spawn could happen after movement, causing a one-frame delay.

### Removed

The `spawn_test_enemy` function is deleted. All its logic moved into `spawn_wave_enemies`.

---

## Running the project

```bash
cargo run
```

Expected behavior:

- **Wave 1 starts immediately** — 3 soldiers (slow, 100 HP) followed by 2 runners (fast, 60 HP). One spawns every 1.0 seconds.
- **At t=12.0, wave 2 starts** — 2 soldiers, 2 scouts (medium speed, 80 HP), then 1 heavy (very slow, 300 HP). Spawn interval 1.2 seconds.
- **Multiple enemies can be on screen simultaneously.** They move independently, towers can target any of them.
- The game runs indefinitely — waves complete, enemies die or reach the base, no "game over" trigger yet.

---

## Design decisions

| Decision | Rationale |
|---|---|
| **Flatten waves into a queue** | No per-wave state. Overlapping waves handled by sort. One system. |
| **`VecDeque` + `pop_front()`** | Simpler than `Vec` + `usize` index. No wasted space from shifted elements. |
| **Spawn texture preloaded in schedule** | Avoids `asset_server.load()` every frame per enemy. |
| **`spawn_wave_enemies` before `move_enemies`** | New enemies move on their first tick. Predictable. |
| **`elapsed` from `Time::delta_secs()`** | Frame-rate independent. Handles long frames gracefully. |
| **Enemy types in TOML** | Level designers can tune stats without recompiling. |
| **`#[serde(default)]` on new fields** | Backward-compatible. Missing fields default to empty. |

---

## Recap

In this part we:

1. Defined 4 **enemy types** (soldier, runner, heavy, scout) in `level_01.toml`.
2. Defined 2 **waves** with start times, spawn intervals, and enemy groups.
3. Built a **flattened spawn schedule** — all events expanded and sorted at load time into a `VecDeque`.
4. Replaced `spawn_test_enemy` with `spawn_wave_enemies` — a system that pops due events and spawns enemies.
5. Chained `spawn_wave_enemies` **before** `move_enemies` so fresh enemies move on their first tick.
6. Demonstrated **overlapping waves** via interleaved event times.

Part 11 can explore a game-over condition (lives system), wave completion feedback, or per-wave rewards — the spawn system now supports all of them.
