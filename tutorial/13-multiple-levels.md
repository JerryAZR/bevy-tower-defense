# Part 13: Multiple Levels — Auto-Discovery and File Refactor

Part 12 gave us a state machine. But we only had one level, and module boundaries had blurred — `main.rs` held lifecycle systems, state types, and level-loading glue alongside the App builder. This part does two things: reorganizes into focused modules with clear responsibilities, then adds automatic level discovery so adding a new level is just dropping a `.toml` file.

---

## What we will build

- **File refactor** — two new modules: `state.rs` for shared types and lifecycle systems, `gameplay.rs` for level loading glue.
- **Level auto-discovery** — scanning `assets/levels/` for `level_*.toml` files at startup.
- **Dynamic level select** — the UI shows all discovered levels, numbered 1–9.
- **Three playable levels** — each with a different path layout and wave composition.

---

## File refactor

### Before

`main.rs` contained everything: state enum, marker components, lifecycle systems, level loading, tilemap spawning. This worked but was hard to extend.

### After

| Module | Responsibility |
|---|---|
| `state.rs` | `GameState`, `GameEntity`, `BaseLives`, `GameResult`, `GameFinished`, `ScreenUi`, `AvailableLevels`, `SelectedLevel`, lifecycle systems |
| `gameplay.rs` | `load_level_data`, `spawn_tilemap`, `setup_spawn_schedule` |
| `main.rs` | `mod` declarations + `App::new()...run()` |

`main.rs` shrank to a thin wiring layer. Each module has one clear purpose. The combat and data modules (`enemy.rs`, `tower.rs`, `level.rs`, `map.rs`, `tiling.rs`) had only import changes — no logic touched.

### Moving `BaseLives` and `GameFinished`

These were previously defined in `enemy.rs` and imported by `main.rs`. Now they live in `state.rs` — they're part of the app's lifecycle state, not enemy-specific logic. `enemy.rs` imports them from `state`:

```rust
use crate::state::{GameEntity, GameState, GameResult, BaseLives, GameFinished};
```

This avoids a circular dependency: `state.rs` still imports `SpawnSchedule` from `enemy.rs`, but the other direction is clean.

---

## Level auto-discovery

### New resources (`state.rs`)

```rust
#[derive(Resource, Default)]
pub struct AvailableLevels(pub Vec<String>);

#[derive(Resource)]
pub struct SelectedLevel(pub String);
```

`AvailableLevels` holds the list of discovered level paths. `SelectedLevel` tracks which level the player chose — set by the level select input system, read by `load_level_data`.

### Scanning levels

A new system in `level_select.rs`:

```rust
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

Scans for `level_01.toml` through `level_09.toml`. Pushes found paths into `AvailableLevels`. Sets `SelectedLevel` to the first available so entering `InGame` without pressing a key still works.

### Dynamic UI

`setup_level_select` now reads `AvailableLevels` and builds the text dynamically:

```rust
let mut text = String::new();
for (i, path) in levels.0.iter().enumerate() {
    let name = path
        .strip_prefix("assets/levels/")
        .and_then(|s| s.strip_suffix(".toml"))
        .unwrap_or(path);
    if !text.is_empty() {
        text.push('\n');
    }
    text.push_str(&format!("[{}] {}", i + 1, name));
}
```

A player with 3 levels sees:

```
[1] level_01
[2] level_02
[3] level_03
```

### Dynamic input

`handle_level_select_input` maps keys 1–9 to available level indices:

```rust
for i in 0..levels.0.len().min(9) {
    let key = match i {
        0 => KeyCode::Digit1,
        1 => KeyCode::Digit2,
        // ... up to Digit9
        _ => continue,
    };
    if keys.just_pressed(key) {
        selected.0 = levels.0[i].clone();
        next_state.set(GameState::InGame);
        return;
    }
}
```

### Using the selected level

`load_level_data` in `gameplay.rs` now reads `SelectedLevel` instead of hardcoding the path:

```rust
pub fn load_level_data(mut commands: Commands, selected: Res<SelectedLevel>) {
    let level = load_level(&selected.0);
    // ...
}
```

---

## Three levels

| Level | Path shape | Spawn | Theme |
|---|---|---|---|
| `level_01` | Left → down → right → down | Left side | Original L-shaped road |
| `level_02` | Right → down → left → down | Right side | Mirrored L-shaped road |
| `level_03` | Center → down → left → down → right → down | Top center | Zigzag road |

Each has different wave compositions and spawn intervals. Adding a fourth is just creating `level_04.toml` — the UI picks it up automatically.

---

## Wiring (`main.rs`)

```rust
App::new()
    // ...
    .init_resource::<AvailableLevels>()
    .add_systems(Startup, (spawn_camera, setup_tower_atlas).chain())
    .add_systems(OnEnter(GameState::LevelSelect), (
        scan_available_levels,
        setup_level_select,
    ).chain())
    // ...
```

`AvailableLevels` is initialized empty. `scan_available_levels` populates it and sets `SelectedLevel`. `setup_tower_atlas` moved to `Startup` — it runs once, not every time the player returns to level select.

---

## Running the project

```bash
cargo run
```

Expected behavior:

- Level select shows **3 levels**, labeled `level_01` through `level_03`.
- Press **1**, **2**, or **3** to start the corresponding level.
- Each level has a visibly different path layout.
- Press Space on the Game Over screen to return to level select.
- Create `assets/levels/level_04.toml` and restart — it appears as `[4]` with no code changes.

---

## Design decisions

| Decision | Rationale |
|---|---|
| **Separate `state.rs` and `gameplay.rs`** | `main.rs` becomes a thin wiring layer. Each module has a clear responsibility. |
| **`BaseLives`/`GameFinished` in `state.rs`** | They're app lifecycle state, not enemy logic. Avoids circular deps. |
| **File system scan for levels** | Zero-config. Drop a file, it shows up. No manual registration. |
| **`SelectedLevel` resource** | Decouples level select input from level loading. Simple `Res<T>` in both systems. |
| **`setup_tower_atlas` in `Startup`** | Runs once. `asset_server.load()` caches handles but the atlas layout shouldn't be re-registered. |

---

## Recap

In this part we:

1. **Refactored** by extracting `state.rs` and `gameplay.rs` — lifecycle types and systems moved out of `main.rs`, level-loading glue moved out of `main.rs`.
2. **Moved** `BaseLives` and `GameFinished` to `state.rs` to resolve import directions.
3. Added **`AvailableLevels`** and **`SelectedLevel`** resources for multi-level support.
4. Built **`scan_available_levels`** — auto-discovers `level_*.toml` files at startup.
5. Made the **level select UI dynamic** — shows discovered levels, not hardcoded text.
6. Made the **input system dynamic** — keys 1–9 map to discovered levels.
7. Created **three levels** with different path layouts to validate the system.
8. Moved **`setup_tower_atlas`** to `Startup` so it doesn't re-run on every level select visit.

The architecture is now solid — modular code, state-driven screens, auto-discovered levels. Part 14 can go anywhere: UI polish, new tower types, additional enemy mechanics, level editing tools, or whatever feels most impactful next.
