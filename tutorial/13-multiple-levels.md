# Part 13: Multiple Levels — Auto-Discovery and File Refactor

> **Time to read:** ~25 minutes  
> **New concepts:** `init_resource`
> **Prerequisite:** Part 12 (multi-level architecture)

---

## Recap: What We Already Have

The game has a state machine with three screens — `LevelSelect`, `InGame`, and `GameOver` — and a single hardcoded level loaded from `level_01.toml`. All shared types (`GameState`, `GameEntity`, `GameResult`) and lifecycle systems (`cleanup_level`, `spawn_camera`) live in `main.rs`, which has grown into a catch-all file. Adding a second level would require hardcoding another path somewhere.

---

## Goal: What We Will Build

1. **Reorganize into focused modules** — extract shared lifecycle types into `state.rs` and level-loading glue into `gameplay.rs` so `main.rs` stays readable.
2. **Auto-discover levels** — scan `assets/levels/` at runtime so adding a new level is just dropping a `.toml` file.
3. **Dynamic level select** — the UI shows all discovered levels, and keys `1`–`9` map to them.
4. **Three playable levels** — each with a different path layout and wave composition.

This matters because a real game ships with multiple levels, and designers should not need to touch Rust code to add content. Clean module boundaries also prevent `main.rs` from becoming an unmaintainable blob as the feature set grows.

---

## New Bevy APIs & Concepts

### `init_resource::<T>()`

Registers a resource in the app using its `Default` implementation. Unlike `insert_resource`, which requires a value immediately, `init_resource` creates the resource lazily at app startup:

```rust
.init_resource::<AvailableLevels>()
```

`AvailableLevels` derives `Default`, so it starts as an empty `Vec<String>`. A system populates it later. This is useful when the initial value is trivial (like an empty collection) and the real data comes from file scanning or user input.

> **Pitfall:** Forgetting `#[derive(Resource, Default)]` on the type causes a compile error — `init_resource` requires both traits.


## Walkthrough

### Designing the feature

**Player-visible behavior:**

1. On launch, the title screen lists all available levels by name.
2. Pressing a number key (`1`–`9`) starts the corresponding level.
3. Each level has a visibly different path layout.
4. After win or lose, pressing `Space` returns to the title screen, which still shows all levels.
5. Creating a new `level_04.toml` file and restarting the game makes it appear automatically.

**ECS data needed:**

- `state.rs` — shared lifecycle types (`GameState`, `GameEntity`, `ScreenUi`, `GameResult`, `GameFinished`, `BaseLives`) and systems (`cleanup_level`, `cleanup_screen_ui`, `spawn_camera`).
- `gameplay.rs` — level loading and tilemap spawning systems.
- `AvailableLevels` resource — list of discovered level file paths.
- `SelectedLevel` resource — the level the player chose.
- `scan_available_levels` system — populates `AvailableLevels` and sets a default `SelectedLevel`.
- Dynamic `setup_level_select` and `handle_level_select_input` — show discovered levels and map keys to them.

**Design decision: why move `BaseLives` and `GameFinished` to `state.rs`?** These resources were defined in `enemy.rs` because they were introduced alongside enemy logic. But they represent app-level lifecycle state, not enemy-specific behavior. Moving them to `state.rs` makes the dependency graph clearer: `state.rs` holds shared lifecycle types, and domain modules import from it. This avoids circular dependencies — `state.rs` can import `SpawnSchedule` from `enemy.rs` without `enemy.rs` also having to define lifecycle resources.

---

### Step 1: Create `state.rs`

Create `src/state.rs` and move all shared lifecycle types and systems into it. What belongs here?
- State enum and marker components (`GameState`, `GameEntity`, `ScreenUi`)
- Lifecycle resources (`BaseLives`, `GameFinished`, `GameResult`)
- New discovery resources (`AvailableLevels`, `SelectedLevel`)
- Cleanup and camera systems (`cleanup_level`, `cleanup_screen_ui`, `spawn_camera`)

`state.rs` must import `PlacedTowers` from `tower.rs` and `SpawnSchedule` from `enemy.rs` because `cleanup_level` removes both resources. This creates a dependency from `state.rs` to the domain modules, which is acceptable because the domain modules import shared types from `state.rs` in the other direction.

```rust
// src/state.rs
use crate::tower::PlacedTowers;
use crate::map::MapLayout;
use crate::tiling::TileRules;
use crate::level::LevelData;
use crate::enemy::SpawnSchedule;

use bevy::prelude::*;

#[derive(Resource)]
pub struct BaseLives(pub i32);

#[derive(Resource)]
pub struct GameFinished;

#[derive(Resource, Default)]
pub struct AvailableLevels(pub Vec<String>);

#[derive(Resource)]
pub struct SelectedLevel(pub String);

#[derive(Component)]
pub struct GameEntity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States, Default)]
pub enum GameState {
    #[default]
    LevelSelect,
    InGame,
    GameOver,
}

#[derive(Resource)]
pub enum GameResult { Victory, Defeat }

#[derive(Component)]
pub struct ScreenUi;

pub fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

pub fn cleanup_level(
    mut commands: Commands,
    entities: Query<Entity, With<GameEntity>>,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<MapLayout>();
    commands.remove_resource::<TileRules>();
    commands.remove_resource::<LevelData>();
    commands.remove_resource::<SpawnSchedule>();
    commands.remove_resource::<PlacedTowers>();
    commands.remove_resource::<BaseLives>();
    commands.remove_resource::<GameFinished>();
}

pub fn cleanup_screen_ui(
    mut commands: Commands,
    query: Query<Entity, With<ScreenUi>>,
) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}
```

What changed from Part 12?
- `BaseLives` and `GameFinished` moved here from `enemy.rs`.
- `AvailableLevels` and `SelectedLevel` are new.
- All types that were in `main.rs` are now here.

---

### Step 2: Create `gameplay.rs`

Create `src/gameplay.rs` and move level loading and tilemap spawning here. This module bridges level data with rendering. It needs `GameEntity` from `state.rs` to tag spawned entities.

```rust
// src/gameplay.rs
use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;

use crate::state::GameEntity;
use crate::level::{LevelData, build_map_from_level, load_level};
use crate::map::{MapLayout, MapTile, PathTile, TileType};
use crate::tiling::{TileRules, build_rules};
use crate::enemy::build_spawn_schedule;
use crate::state::{BaseLives, SelectedLevel};
use crate::tower::PlacedTowers;

pub fn load_level_data(mut commands: Commands, selected: Res<SelectedLevel>) {
    let level = load_level(&selected.0);
    let map = build_map_from_level(&level);
    let rules = build_rules();

    commands.insert_resource(map);
    commands.insert_resource(rules);
    commands.insert_resource(level);
    commands.insert_resource(PlacedTowers::default());
    commands.insert_resource(BaseLives(5));
}
```

What does `load_level_data` query?
- `Commands` — to insert resources.
- `Res<SelectedLevel>` — the level file path chosen by the player.

`load_level_data` changed in one way from Part 12: it reads `Res<SelectedLevel>` instead of hardcoding `"assets/levels/level_01.toml"`. The `SelectedLevel` resource is inserted by `scan_available_levels` before the player sees the level select screen, so it is guaranteed to exist when `load_level_data` runs.

`spawn_tilemap` and `setup_spawn_schedule` move here unchanged from Part 12. See `src/gameplay.rs` for their full implementations.

---

### Step 3: Update imports in domain modules

With types moved to `state.rs`, the domain modules need updated imports.

**In `src/enemy.rs`**, remove the `BaseLives` and `GameFinished` definitions and update imports:

```rust
use crate::state::{GameEntity, GameState, GameResult, BaseLives, GameFinished};
```

**In `src/tower.rs`**, update the `GameEntity` import:

```rust
use crate::state::GameEntity;
```

**In `src/game_over.rs`**, update imports:

```rust
use crate::state::{GameState, GameResult, ScreenUi};
```

These are mechanical changes — no logic changes in any domain module.

---

### Step 4: Scan for available levels

The level select screen needs to know which levels exist. We add `scan_available_levels` to check for `level_01.toml` through `level_09.toml` in `assets/levels/`. It runs every time we enter `LevelSelect`, so adding a level file while the game is running (and then returning to the menu) would pick it up.

What does it query?
- `Commands` — to insert the default `SelectedLevel` resource.
- `ResMut<AvailableLevels>` — the list to populate.

In `src/level_select.rs`:

```rust
use crate::state::{GameState, ScreenUi, AvailableLevels, SelectedLevel};

pub fn scan_available_levels(
    mut commands: Commands,
    mut levels: ResMut<AvailableLevels>,
) {
    levels.0.clear();
    for i in 1..=9 {
        let path = format!("assets/levels/level_{:02}.toml", i);
        if std::path::Path::new(&path).exists() {
            levels.0.push(path);
        }
    }
    if let Some(first) = levels.0.first() {
        commands.insert_resource(SelectedLevel(first.clone()));
    }
}
```

Why a fixed 1–9 range? It is simple, deterministic, and enough for a tutorial. A production game might use `std::fs::read_dir` to discover all `.toml` files with arbitrary names.

> **Pitfall:** `std::path::Path::new(&path).exists()` checks the file system at runtime. This works for native builds but fails on WASM because browsers cannot access the local file system. For web builds, you would embed the level list in the binary or fetch it from a server.

---

### Step 5: Build the dynamic level select UI

`setup_level_select` now reads `AvailableLevels` and builds the display text dynamically. Instead of hardcoded "Press [1] — Level 1", it iterates discovered levels and formats each as `[N] level_name`.

What does it query?
- `Commands` — to spawn UI nodes.
- `Res<AvailableLevels>` — the list of levels to display.

In `src/level_select.rs`:

```rust
pub fn setup_level_select(mut commands: Commands, levels: Res<AvailableLevels>) {
    let mut text = String::new();
    for (i, path) in levels.0.iter().enumerate() {
        if !text.is_empty() {
            text.push('\n');
        }
        let name = path
            .strip_prefix("assets/levels/")
            .and_then(|s| s.strip_suffix(".toml"))
            .unwrap_or(path);
        text.push_str(&format!("[{}] {}", i + 1, name));
    }

    commands
        .spawn((
            ScreenUi,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(text),
                TextFont {
                    font_size: 40.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}
```

The name extraction strips the `assets/levels/` prefix and `.toml` suffix so the player sees `level_01` instead of a full path. `unwrap_or(path)` is a safety net: if the path does not match the expected pattern, the raw path is displayed instead of crashing.

---

### Step 6: Handle dynamic input

`handle_level_select_input` maps number keys to discovered levels dynamically. It iterates up to `levels.0.len().min(9)` so if only three levels exist, only keys `1`–`3` do anything.

What does it query?
- `Res<ButtonInput<KeyCode>>` — to detect key presses.
- `Res<AvailableLevels>` — to know which levels exist.
- `ResMut<SelectedLevel>` — to record the chosen level path.
- `ResMut<NextState<GameState>>` — to transition to `InGame`.

In `src/level_select.rs`:

```rust
pub fn handle_level_select_input(
    keys: Res<ButtonInput<KeyCode>>,
    levels: Res<AvailableLevels>,
    mut selected: ResMut<SelectedLevel>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for i in 0..levels.0.len().min(9) {
        let key = match i {
            0 => KeyCode::Digit1,
            1 => KeyCode::Digit2,
            2 => KeyCode::Digit3,
            3 => KeyCode::Digit4,
            4 => KeyCode::Digit5,
            5 => KeyCode::Digit6,
            6 => KeyCode::Digit7,
            7 => KeyCode::Digit8,
            8 => KeyCode::Digit9,
            _ => continue,
        };
        if keys.just_pressed(key) {
            selected.0 = levels.0[i].clone();
            next_state.set(GameState::InGame);
            return;
        }
    }
}
```

> This is the same resource-as-communication-channel pattern we used in Part 12 with `GameResult` and `GameFinished` — one system writes a resource, another reads it, and neither knows about the other. Here `handle_level_select_input` writes the path into `SelectedLevel`, and `load_level_data` (which runs later in `OnEnter(InGame)`) reads it back.

> The `match` on index is verbose but explicit. Each key maps directly to an array index, so pressing `2` always selects the second level in the list. An alternative is a helper array `[KeyCode::Digit1, KeyCode::Digit2, ...]` indexed by `i`, which is shorter but less obvious to a reader learning Rust.

---

### Step 7: Move `setup_tower_atlas` to `Startup`

In Part 12, `setup_tower_atlas` ran in `OnEnter(LevelSelect)`. This re-ran every time the player returned to the level select screen after a game. While `asset_server.load()` caches handles, re-creating the atlas layout is unnecessary work. Moving it to `Startup` ensures it runs exactly once when the app launches.

No code changes to `setup_tower_atlas` itself — only its schedule registration moves in `main.rs`.

---

### Step 8: Wire the app in `main.rs`

With all types and systems moved to modules, `main.rs` shrinks to a thin wiring layer: `mod` declarations and the `App` builder. Every import now pulls from a module instead of `main.rs` internals.

What does `main.rs` contain?
- `mod` declarations for all modules.
- Imports from `state`, `enemy`, `tower`, `gameplay`, `level_select`, and `game_over`.
- The `App` builder with `.init_resource::<AvailableLevels>()`, schedule wiring, and state gating.

```rust
// src/main.rs
mod level;
mod map;
mod tiling;
mod enemy;
mod tower;
mod state;
mod gameplay;
mod level_select;
mod game_over;

use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;

use state::{GameState, AvailableLevels, spawn_camera, cleanup_level, cleanup_screen_ui};
use enemy::{spawn_wave_enemies, move_enemies, process_base_reachers, check_game_state};
use tower::{setup_tower_atlas, spawn_placement_preview, update_placement_preview, place_tower_on_click, attack_enemies, despawn_timed};
use gameplay::{load_level_data, setup_spawn_schedule, spawn_tilemap};
use level_select::{scan_available_levels, setup_level_select, handle_level_select_input};
use game_over::{setup_game_over, handle_game_over_input};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .add_plugins(TilemapPlugin)
        .init_state::<GameState>()
        .init_resource::<AvailableLevels>()
        .add_systems(Startup, (spawn_camera, setup_tower_atlas).chain())
        // ---------- LevelSelect ----------
        .add_systems(OnEnter(GameState::LevelSelect), (
            scan_available_levels,
            setup_level_select,
        ).chain())
        .add_systems(OnExit(GameState::LevelSelect), cleanup_screen_ui)
        .add_systems(Update, handle_level_select_input
            .run_if(in_state(GameState::LevelSelect)))
        // ---------- InGame ----------
        .add_systems(OnEnter(GameState::InGame), (
            load_level_data,
            setup_spawn_schedule,
            spawn_tilemap,
            spawn_placement_preview,
        ).chain())
        .add_systems(OnExit(GameState::InGame), cleanup_level)
        .add_systems(FixedUpdate, (
            spawn_wave_enemies,
            move_enemies,
            attack_enemies,
            process_base_reachers,
            check_game_state,
        ).chain().run_if(in_state(GameState::InGame)))
        .add_systems(Update, (
            update_placement_preview,
            place_tower_on_click,
            despawn_timed,
        ).run_if(in_state(GameState::InGame)))
        // ---------- GameOver ----------
        .add_systems(OnEnter(GameState::GameOver), setup_game_over)
        .add_systems(OnExit(GameState::GameOver), cleanup_screen_ui)
        .add_systems(Update, handle_game_over_input
            .run_if(in_state(GameState::GameOver)))
        .run();
}
```

Key changes from Part 12:
- All imports now pull from modules (`state`, `gameplay`, `level_select`, etc.) instead of `main.rs` internals.
- `.init_resource::<AvailableLevels>()` registers the empty level list.
- `setup_tower_atlas` moved from `OnEnter(LevelSelect)` to `Startup`.
- `scan_available_levels` added to the `OnEnter(LevelSelect)` chain, before `setup_level_select`.

The `OnEnter(LevelSelect)` chain is ordered: `scan_available_levels` populates `AvailableLevels` and sets `SelectedLevel`, then `setup_level_select` reads `AvailableLevels` to build the UI. If the order were reversed, the UI would see an empty list.

---

### Step 9: Create the new level files

Create `assets/levels/level_02.toml` and `assets/levels/level_03.toml`. Both follow the same schema as `level_01.toml` but with different paths, enemy types, and waves.

**Level 2** mirrors the original L-shape: enemies enter from the right, travel left, then down.

```toml
[map]
width = 15
height = 10

[paths.main_road]
waypoints = [
    [13, 9],
    [13, 5],
    [2, 5],
    [2, 1],
]

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

[[waves]]
start_time = 0.0
path = "main_road"
spawn_interval = 0.8
enemies = [
    { type = "runner", count = 4 },
    { type = "scout", count = 3 },
]

[[waves]]
start_time = 10.0
path = "main_road"
spawn_interval = 1.0
enemies = [
    { type = "soldier", count = 3 },
    { type = "heavy", count = 2 },
]
```

**Level 3** uses a zigzag: enemies enter from the top center, weave down, left, down, right, and down again.

```toml
[map]
width = 15
height = 10

[paths.main_road]
waypoints = [
    [7, 9],
    [7, 7],
    [2, 7],
    [2, 3],
    [12, 3],
    [12, 1],
]

# enemy_types identical to level_02

[[waves]]
start_time = 0.0
path = "main_road"
spawn_interval = 0.6
enemies = [
    { type = "scout", count = 6 },
    { type = "soldier", count = 2 },
]

[[waves]]
start_time = 8.0
path = "main_road"
spawn_interval = 0.9
enemies = [
    { type = "runner", count = 4 },
    { type = "heavy", count = 3 },
]
```

Both levels reuse the same four enemy types but vary wave timing, spawn intervals, and enemy counts. The shorter spawn intervals in level 3 make it more challenging.

---

### Step 10: Verify

```bash
cargo run
```

You should see:

- **Title screen** listing three levels: `[1] level_01`, `[2] level_02`, `[3] level_03`.
- Pressing `1`, `2`, or `3` loads the corresponding level with a visibly different path.
- After win or lose, pressing `Space` returns to the title screen.
- Creating `assets/levels/level_04.toml` (with a valid schema) and restarting the game adds `[4] level_04` to the list automatically.

---

### Simplifications

| Simplification | Why it works | Future direction |
|---|---|---|
| **Fixed 1–9 scan range** | Nine levels is enough for a tutorial. | Use `std::fs::read_dir` to discover all `.toml` files with arbitrary names. |
| **Resets `SelectedLevel` on every menu visit** | `scan_available_levels` always picks the first level as default. | Remember the player's last choice with a persistent setting or save file. |
| **Number-key input only** | Fast to implement, no mouse handling needed. | Clickable buttons or a scrollable list for many levels. |
| **No level preview/thumbnail** | The text list is sufficient for debugging. | Render a minimap or screenshot for each level in the select screen. |
| **File system scan on native only** | `std::path::Path::exists()` works on desktop. | For WASM, embed level metadata in a registry or fetch from a server. |

---

## Summary

- We reorganized the codebase into focused modules: `state.rs` for shared lifecycle types, `gameplay.rs` for level loading and rendering, and a thin `main.rs` wiring layer.
- We moved `BaseLives` and `GameFinished` from `enemy.rs` to `state.rs` to clarify module responsibilities and avoid circular dependencies.
- We added `AvailableLevels` and `SelectedLevel` resources to support multiple levels, using the resource-as-communication-channel pattern.
- We built `scan_available_levels` to auto-discover `level_*.toml` files, making new levels zero-config.
- We made the level select UI and input dynamic, mapping keys `1`–`9` to discovered levels.
- We created three distinct levels with different path shapes and wave compositions.
- We moved `setup_tower_atlas` to `Startup` so it only runs once per app launch.

In **Part 14** we will add a **gold economy**: a `Gold` resource, passive income, kill bounties defined per enemy type in the TOML, tower placement costs, and an in-game HUD showing the current balance. This turns the sandbox into a strategy game where every tower placement is a real decision.
