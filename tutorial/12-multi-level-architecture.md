# Part 12: Multi-Level Architecture — State Machine and Cleanup

Our game has a complete loop — enemies spawn, towers shoot, win/lose conditions fire. But everything is glued to `Startup`. In this part we refactor the app into a state machine so levels can load on demand, unload cleanly, and be replayed.

---

## What we will build

- **Three screens** — `LevelSelect`, `InGame`, `GameOver` — driven by Bevy's `States`.
- **On-demand loading** — entering `InGame` loads the level, builds the tilemap, and sets up the spawn schedule.
- **Full cleanup** — exiting `InGame` despawns all game entities and removes level resources.
- **`GameEntity` marker** — a single component tag on every spawned entity so cleanup is one query.
- **Replayability** — after `GameOver`, press Space to return to `LevelSelect` and play again.

## Bevy concepts introduced

### `States`

Bevy's built-in state machine lets you define a set of exclusive states for your app. Systems can react to state changes (`OnEnter`, `OnExit`) or gate their execution with `.run_if(in_state(...))`. A `States` derive marks the enum:

```rust
#[derive(States, Default)]
enum GameState {
    #[default]
    LevelSelect,
    InGame,
    GameOver,
}
```

### `OnEnter` and `OnExit`

Schedules that run exactly once when entering or leaving a state. Used for setup (`OnEnter(InGame)` loads the level) and teardown (`OnExit(InGame)` despawns everything, `OnExit(LevelSelect)` clears the UI).

### `.run_if(in_state(...))`

System gating. Only runs the system when the app is in the specified state. Replaces manual `Option<Res<T>>` or early-return guards:

```rust
add_systems(Update, place_tower_on_click.run_if(in_state(GameState::InGame)))
```

### `NextState<T>`

A `ResMut<NextState<T>>` that systems write to in order to request a state transition. The actual transition happens after the current schedule completes:

```rust
mut next_state: ResMut<NextState<GameState>>,
// ...
next_state.set(GameState::GameOver);
```

## The state machine

```
┌─────────────┐    key 1     ┌──────────┐   win/lose   ┌──────────┐
│ LevelSelect │ ────────────→│  InGame  │ ────────────→│ GameOver │
└─────────────┘              └──────────┘              └──────────┘
       ↑                                                    │
       └────────────────── Space ───────────────────────────┘
```

### `main.rs` — State enum

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States, Default)]
pub enum GameState {
    #[default]
    LevelSelect,
    InGame,
    GameOver,
}
```

### Wiring

```rust
App::new()
    .init_state::<GameState>()

    // LevelSelect
    .add_systems(OnEnter(GameState::LevelSelect), (setup_level_select, setup_tower_atlas).chain())
    .add_systems(Update, handle_level_select_input.run_if(in_state(GameState::LevelSelect)))

    // InGame
    .add_systems(OnEnter(GameState::InGame), (load_level_data, setup_spawn_schedule, spawn_tilemap, spawn_placement_preview).chain())
    .add_systems(OnExit(GameState::InGame), cleanup_level)
    .add_systems(FixedUpdate, (spawn_wave_enemies, move_enemies, attack_enemies, process_base_reachers, check_game_state).chain().run_if(in_state(GameState::InGame)))
    .add_systems(Update, (update_placement_preview, place_tower_on_click, despawn_timed).run_if(in_state(GameState::InGame)))

    // GameOver
    .add_systems(OnEnter(GameState::GameOver), setup_game_over)
    .add_systems(Update, handle_game_over_input.run_if(in_state(GameState::GameOver)))
```

Every gameplay system is gated with `.run_if(in_state(InGame))` — they only execute during gameplay.

---

## `GameEntity` marker

```rust
#[derive(Component)]
pub struct GameEntity;
```

Tagged on every entity spawned during gameplay: tiles, enemies, towers, placement previews, muzzle flashes. Cleanup is one query:

```rust
fn cleanup_level(
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
}
```

Resources are removed separately — `OnEnter(InGame)` re-inserts fresh copies on replay.

### What persists

`TowerAtlas` (texture + atlas layout) lives across levels — loaded once in `OnEnter(LevelSelect)`. `Camera2d` is spawned once in `Startup`. `GameResult` survives `OnExit(InGame)` so `GameOver` can read it.

---

## LevelSelect screen

`src/level_select.rs` — minimal UI. Press key `1` to start the level:

```rust
pub fn setup_level_select(mut commands: Commands) {
    commands
        .spawn(Node { /* full-screen centered */ })
        .with_children(|parent| {
            parent.spawn((Text::new("Press [1] — Level 1"), TextFont { font_size: 40.0, ..default() }, TextColor(Color::WHITE)));
        });
}

pub fn handle_level_select_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if keys.just_pressed(KeyCode::Digit1) {
        next_state.set(GameState::InGame);
    }
}
```

This is a debug draft. Number keys work, no mouse. When we add more levels, the list grows naturally.

---

## GameOver screen

`src/game_over.rs` — shows result text and "Press Space to continue":

```rust
pub fn setup_game_over(
    mut commands: Commands,
    result: Res<GameResult>,
) {
    let message = match *result {
        GameResult::Defeat => "Game Over — the base was destroyed!",
        GameResult::Victory => "Victory — all enemies defeated!",
    };
    // ... UI node with message + "Press Space to continue" ...
}
```

`GameResult` is a resource set by `check_game_state` before the transition:

```rust
// In check_game_state:
if lives.0 <= 0 {
    commands.insert_resource(GameResult::Defeat);
    next_state.set(GameState::GameOver);
} else if all_enemies_dead {
    commands.insert_resource(GameResult::Victory);
    next_state.set(GameState::GameOver);
}
```

Critical detail: `commands.insert_resource(GameResult)` must happen in the same system as `next_state.set(GameOver)`. The command buffer flushes *before* the state transition, so `GameResult` exists when `OnEnter(GameOver)` runs — even though `OnExit(InGame)` runs in between.

### Why `GameResult` instead of reading `BaseLives`?

We could leave `BaseLives` out of `cleanup_level` and read it directly in `setup_game_over`. But that couples the display to the combat system — if we later add a "time ran out" loss condition, the GameOver screen would need to inspect a different resource. `GameResult` is a deliberate decoupling: the gameplay system records *what happened*, and the UI reads *what to show*. It's a design choice, not a constraint.

---

## Design decisions

| Decision | Rationale |
|---|---|
| **`GameEntity` marker** | Single tag, single query for cleanup. No need to enumerate component types. |
| **`run_if(in_state(InGame))`** | Systems don't run outside gameplay. No wasted queries, no `Option<T>` guards. |
| **`GameResult` resource** | Decouples win/lose detection from display. Survives `OnExit(InGame)` cleanup. |
| **`TowerAtlas` persists** | Tilesheet is shared across levels. Loading once avoids redundant asset work. |
| **One camera, spawned in `Startup`** | Avoids camera-ordering warnings during state transitions. |
| **`LevelSelect` uses number keys** | Debug draft. Fast to build, easy to extend with buttons or mouse later. |

---

## Recap

In this part we:

1. Introduced a **three-state machine** — `LevelSelect → InGame → GameOver → LevelSelect`.
2. Added **`GameEntity`** marker to every spawned entity so cleanup is a single query.
3. Moved level loading into **`OnEnter(InGame)`** — loads on demand, not at startup.
4. Added **`cleanup_level`** in `OnExit(InGame)` — despawns entities, removes level resources.
5. Created **`level_select.rs`** and **`game_over.rs`** — minimal UI screens with input handling.
6. Gated all gameplay systems with **`.run_if(in_state(InGame))`**.
7. Used **`GameResult`** resource to pass outcome from `check_game_state` to the `GameOver` screen.
8. Made the game **replayable** — Space returns to `LevelSelect`, 1 starts again.

Part 13 can add additional levels, a level selection screen that scales beyond one level, or persistent game data like high scores.
