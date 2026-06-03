# Part 12: Multi-Level Architecture — State Machine and Cleanup

> **Time to read:** ~30 minutes  
> **New concepts:** `States`, `OnEnter` / `OnExit`, `.run_if(in_state(...))`, `NextState<T>`, marker components for bulk cleanup  
> **Prerequisite:** Part 11 (win/lose conditions)

---

## Recap: What We Already Have

The game has a complete loop: waves spawn, towers shoot, and win/lose conditions log the result. But everything is glued to `Startup` — the level loads once at app start, and the game cannot be replayed without restarting the executable. There is no menu, no game-over screen, and no way to clean up and start over.

---

## Goal: What We Will Build

We will refactor the app into a state machine so levels can load on demand, unload cleanly, and be replayed:

1. **Three states** — `LevelSelect`, `InGame`, `GameOver` — driven by Bevy's `States` system.
2. **On-demand loading** — entering `InGame` loads the level, builds the tilemap, and sets up the spawn schedule.
3. **Full cleanup** — exiting `InGame` despawns all game entities and removes level resources.
4. **`GameEntity` marker** — a single component tag on every spawned entity so cleanup is one query.
5. **Replayability** — after `GameOver`, press Space to return to `LevelSelect` and play again.

This matters because a real game needs menus, transitions, and the ability to restart. `Startup`-only initialization is fine for prototypes, but players expect to play multiple rounds without quitting.

---

## New Bevy APIs & Concepts

### `States`

Bevy's built-in state machine lets you define a set of mutually exclusive states for your app. Only one state is active at any time. Systems can react to state transitions (`OnEnter`, `OnExit`) or gate their execution with `.run_if(in_state(...))`.

A state enum requires four derives: `States`, `Default`, `Clone`, `Copy`, `PartialEq`, `Eq`, and `Hash`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States, Default)]
pub enum GameState {
    #[default]
    LevelSelect,
    InGame,
    GameOver,
}
```

`#[default]` marks which state the app starts in. `Hash` and `Eq` are required because Bevy uses the type as a lookup key internally. `Clone` and `Copy` are required because state values are passed around by value.

> **Pitfall:** Forgetting `Default` causes a compile error when calling `.init_state::<GameState>()`. Forgetting `Hash` or `Eq` produces an obscure trait-bound error deep in Bevy internals.

### `OnEnter` and `OnExit`

`OnEnter(State)` is a schedule that runs exactly once when the app enters a state. `OnExit(State)` runs exactly once when leaving it. They are the state-machine equivalent of `Startup` and teardown.

Use `OnEnter` for setup: load level data, spawn the tilemap, initialize resources. Use `OnExit` for teardown: despawn entities, remove resources, clear UI. This pairs naturally with replayability — each time you enter `InGame`, you get a fresh world state.

### `.run_if(in_state(...))`

System conditions gate whether a system runs at all. `.run_if(in_state(GameState::InGame))` means the system only executes while the app is in the `InGame` state. This replaces manual guards like `Option<Res<GameOver>>` or early-return checks.

```rust
.add_systems(Update, place_tower_on_click.run_if(in_state(GameState::InGame)))
```

Systems without a `run_if` condition run in every state. Be careful: a `FixedUpdate` system without a gate would continue running during `GameOver`, moving enemies and spawning waves while the player reads the result screen.

### `NextState<T>`

`ResMut<NextState<GameState>>` is how systems request a state transition. Writing to it does not change the state immediately — the transition happens after the current schedule completes:

```rust
mut next_state: ResMut<NextState<GameState>>,
// ...
next_state.set(GameState::GameOver);
```

This means `OnExit(InGame)` still runs after a system in `InGame` calls `next_state.set(GameOver)`. The sequence is: system runs → commands flush → state transitions → `OnExit(InGame)` runs → `OnEnter(GameOver)` runs. This ordering is critical: cleanup happens before the next state's setup.

### Marker components for bulk cleanup

A marker component is a zero-sized type attached to entities solely so they can be queried and acted on as a group. We use two:

- **`GameEntity`** — tagged on every entity spawned during gameplay (tiles, enemies, towers, previews, muzzle flashes). `cleanup_level` queries `With<GameEntity>` and despawns them all.
- **`ScreenUi`** — tagged on menu and game-over UI nodes. `cleanup_screen_ui` removes them when leaving a screen.

This is cleaner than enumerating every component type in cleanup queries. It also scales naturally: any new gameplay entity automatically gets cleaned up if you remember to add `GameEntity`.

> **Pitfall:** Forgetting to tag a spawned entity means it leaks across state transitions. A tower preview without `GameEntity` would persist into the next level, visible on the menu screen.

---

## Walkthrough

### Designing the feature

Before writing code, think about what the player should see and what data that requires.

**Player-visible behavior:**

1. A title screen appears on launch — "Press 1 to start Level 1."
2. Pressing `1` transitions to gameplay: the map loads, towers can be placed, waves spawn.
3. When the player wins or loses, a result screen appears with the outcome and "Press Space to continue."
4. Pressing `Space` returns to the title screen.
5. Starting a new level is a fresh game — no lingering enemies, towers, or resources from the previous round.

**ECS data needed:**

- `GameState` enum — `LevelSelect`, `InGame`, `GameOver`.
- `GameEntity` marker component — for bulk entity cleanup.
- `ScreenUi` marker component — for UI cleanup between screens.
- `GameResult` enum resource — `Victory` or `Defeat`, set by `check_game_state` and read by `setup_game_over`.
- `GameFinished` marker resource — prevents `check_game_state` from triggering multiple transitions.
- `cleanup_level` system — despawns all `GameEntity` entities and removes level resources.
- `cleanup_screen_ui` system — despawns all `ScreenUi` entities.
- `setup_level_select` and `setup_game_over` systems — UI for menu and result screens.
- Input handlers — `handle_level_select_input` (key `1`) and `handle_game_over_input` (Space).

**Design decision: resource-based gate instead of `Local<bool>`.** In Part 11, `check_game_state` used `Local<bool>` to fire once. That worked because the system ran every tick in a single monolithic app. With states, `check_game_state` might not run during `GameOver` — but if we ever transition back to `InGame`, the `Local` would still be `true` from the previous round, breaking the game. A `GameFinished` resource is better: it is inserted when the game ends and removed during `cleanup_level`, so each new round starts fresh.

---

### Step 1: Define the state enum

In `main.rs`, add the state enum. What must it have?

- `LevelSelect` — the starting screen.
- `InGame` — active gameplay.
- `GameOver` — result screen.

The derives are verbose but required by Bevy's state machinery:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States, Default)]
pub enum GameState {
    #[default]
    LevelSelect,
    InGame,
    GameOver,
}
```

Before any state-gated systems can run, the app must know about our `GameState` type. `.init_state::<GameState>()` registers the enum, sets the starting value to the `#[default]` variant (`LevelSelect`), and creates the `NextState` resource so systems can later request transitions. Add it to `main()`:

```rust
App::new()
    .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
    .add_plugins(TilemapPlugin)
    .init_state::<GameState>()
```

---

### Step 2: Add marker components

We need two marker tags so cleanup can target entities by group rather than by component type. `GameEntity` will be attached to every tile, enemy, tower, and effect so `cleanup_level` can despawn them all at once; it must be `pub` because `enemy.rs` and `tower.rs` will use it. `ScreenUi` tags menu and game-over nodes so they can be removed when leaving a screen, and it can stay private to `main.rs`. In `main.rs`, add both components:

```rust
#[derive(Component)]
pub struct GameEntity;

#[derive(Component)]
struct ScreenUi;
```

---

### Step 3: Tag all gameplay entities

Every entity spawned during gameplay must carry `GameEntity` so `cleanup_level` can find it. This is a mechanical change across three files.

**In `main.rs`, `spawn_tilemap`:** Add `GameEntity` to the tilemap entity and every tile entity:

```rust
let tilemap_entity = commands.spawn(GameEntity).id();
// ...
let tile_entity = commands
    .spawn((
        TileBundle { /* ... */ },
        tile_type,
        MapTile,
        GameEntity,  // new
    ))
    .id();
```

**In `src/enemy.rs`, `spawn_wave_enemies`:** Add `GameEntity` to the spawned enemy bundle:

```rust
commands.spawn((
    Sprite::from_atlas_image(/* ... */),
    Transform::from_xyz(x, y, 1.0),
    Enemy,
    PathFollower { /* ... */ },
    MoveSpeed(event.speed),
    Health(event.health),
    GameEntity,  // new
));
```

**In `src/tower.rs`:** Add `GameEntity` to:
- Both `TowerPreview` entities in `spawn_placement_preview`
- The `Tower` base in `place_tower_on_click`
- The `TowerTurret` in `place_tower_on_click`
- The `MuzzleFlash` child entity in `attack_enemies`

This is tedious but critical. One forgotten tag and that entity leaks across replays.

---

### Step 4: Replace `Local<bool>` with `GameFinished`

In Part 11, `check_game_state` used `Local<bool>` to fire once. That state would survive across replays because `Local` is scoped to the system function, not the game session. We replace it with a resource that is removed during cleanup.

In `src/enemy.rs`, add the marker resource:

```rust
#[derive(Resource)]
pub struct GameFinished;
```

Update `check_game_state` to use `Option<Res<GameFinished>>` instead of `Local<bool>`, and add `NextState` for transitions:

```rust
pub fn check_game_state(
    mut commands: Commands,
    finished: Option<Res<GameFinished>>,
    lives: Res<BaseLives>,
    schedule: Res<SpawnSchedule>,
    alive: Query<(), With<Enemy>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if finished.is_some() {
        return;
    }
    if lives.0 <= 0 {
        info!("Game Over — the base has been destroyed!");
        commands.insert_resource(GameResult::Defeat);
        commands.insert_resource(GameFinished);
        next_state.set(GameState::GameOver);
    } else if schedule.events.is_empty() && alive.iter().count() == 0 {
        info!("Victory — all enemies defeated!");
        commands.insert_resource(GameResult::Victory);
        commands.insert_resource(GameFinished);
        next_state.set(GameState::GameOver);
    }
}
```

What does it query?
- `Commands` — to insert `GameResult` and `GameFinished` resources.
- `Option<Res<GameFinished>>` — returns early if the game already ended (replaces Part 11's `Local<bool>`).
- `Res<BaseLives>` — to check for defeat.
- `Res<SpawnSchedule>` — to check if waves are exhausted.
- `Query<(), With<Enemy>>` — to count living enemies.
- `ResMut<NextState<GameState>>` — to request the state transition.

> **Why a resource instead of `Local`?** `Local<bool>` persists for the lifetime of the app. If the player returns to `LevelSelect` and starts a new game, the old `finished` flag would still be `true`, preventing win/lose detection in the new round. `GameFinished` is inserted on game over and removed during `cleanup_level`, so each round starts fresh.
>
> **Why guard at all?** State transitions happen in the `StateTransition` schedule, which runs once per frame after `PreUpdate` — not inside `FixedUpdate`. If the renderer is lagging, Bevy may run `FixedUpdate` multiple times per frame before the state actually changes. Without the `GameFinished` guard, `check_game_state` would fire on every catch-up tick, logging the message repeatedly and redundantly inserting resources.

---

### Step 5: Add `GameResult`

In `main.rs`, add an enum resource to pass the outcome from `check_game_state` to the `GameOver` screen:

```rust
#[derive(Resource)]
pub enum GameResult {
    Victory,
    Defeat,
}
```

`GameResult` is public because `game_over.rs` needs to read it. We do not derive `Default` because there is no meaningful default — the resource only exists after a game ends.

> **Why not read `BaseLives` directly in the GameOver screen?** We could, but that couples display logic to combat logic. If we later add a "time ran out" loss condition, the GameOver screen would need to inspect multiple resources. `GameResult` is deliberate decoupling: the gameplay system records *what happened*, and the UI reads *what to show*.

---

### Step 6: Create cleanup systems

When exiting `InGame`, we must remove every trace of the previous round. What must be cleaned up?

1. **All spawned entities** — tiles, enemies, towers, previews, muzzle flashes. Query by `GameEntity`.
2. **All level resources** — `MapLayout`, `TileRules`, `LevelData`, `SpawnSchedule`, `PlacedTowers`, `BaseLives`, `GameFinished`.

In `main.rs`, add `cleanup_level`:

```rust
use bevy::ecs::system::entity_command::despawn;

fn cleanup_level(
    mut commands: Commands,
    entities: Query<Entity, With<GameEntity>>,
) {
    for entity in &entities {
        // Silently ignore double-despawn errors — children may have been
        // auto-despawned by a parent earlier in this same iteration.
        commands.entity(entity).queue_silenced(despawn());
    }
    commands.remove_resource::<MapLayout>();
    commands.remove_resource::<TileRules>();
    commands.remove_resource::<LevelData>();
    commands.remove_resource::<SpawnSchedule>();
    commands.remove_resource::<PlacedTowers>();
    commands.remove_resource::<BaseLives>();
    commands.remove_resource::<GameFinished>();
}
```

#### Why `queue_silenced`?

Some game entities are **children** of other entities. In Part 9 we added muzzle flash effects as children of turret entities. Both the turret and its flash children carry `GameEntity`.

When `cleanup_level` iterates all `GameEntity` entities and despawns a turret (the parent), Bevy **automatically despawns its children**. But those children are still in the iteration — when the loop reaches them, their despawn command fails because the entity no longer exists. This produces a warning:

```
Entity despawned: The entity with ID 179v11 is invalid
```

`queue_silenced(despawn())` wraps the despawn command and silently drops errors. The entity is already gone, which is exactly the state we want.

> **Alternatives:** `queue_silenced` is not the only valid approach. You could also:
> - **Not tag child entities with `GameEntity`** — only tag parents, and let Bevy's recursive despawn handle the children.
> - **Filter the query with `Without<ChildOf>`** — only despawn root entities and let Bevy handle their children.
> - **Use `queue_handled`** — similar to `queue_silenced` but takes an error handler `fn(BevyError, ErrorContext)` when you want to inspect or log failures rather than silently drop them.
>
> All three are correct. We chose `queue_silenced` here primarily to demonstrate the API — every approach avoids leaking as long as you apply it consistently.

UI nodes for the title and game-over screens also need teardown when leaving those states, but they are not gameplay entities and should not be caught by `cleanup_level`. A separate `ScreenUi` marker and `cleanup_screen_ui` system handles them; this same system is reused for both `OnExit(LevelSelect)` and `OnExit(GameOver)`:

```rust
fn cleanup_screen_ui(
    mut commands: Commands,
    query: Query<Entity, With<ScreenUi>>,
) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}
```

---

### Step 7: Move initialization into `OnEnter(InGame)`

In Part 11, level loading happened in `Startup`. Now it must happen every time the player starts a level. We also need to re-initialize resources that were removed by `cleanup_level`.

Update `load_level_data` to insert resources that were previously added elsewhere:

```rust
fn load_level_data(mut commands: Commands) {
    let level = load_level("assets/levels/level_01.toml");
    let map = build_map_from_level(&level);
    let rules = build_rules();

    commands.insert_resource(map);
    commands.insert_resource(rules);
    commands.insert_resource(level);
    commands.insert_resource(PlacedTowers::default());  // was init_resource in main
    commands.insert_resource(BaseLives(5));              // was insert_resource in main
}
```

`PlacedTowers` and `BaseLives` moved from `main.rs` into `load_level_data` because they are removed during `cleanup_level` and must be recreated on each round.

The camera is spawned once in `Startup` (not state-gated) so it persists across all screens:

```rust
fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
```

`setup_tower_atlas` runs in `OnEnter(LevelSelect)` so the texture is loaded before gameplay begins, and persists across rounds since `TowerAtlas` is not cleaned up.

---

### Step 8: Create the LevelSelect screen

The title screen needs two things: a full-screen centered UI node telling the player how to start, and a system that watches for the `1` key and transitions to `InGame`. We keep the UI minimal so adding more levels later is easy. The root node carries `ScreenUi` so `cleanup_screen_ui` can remove it when we leave this state. Create `src/level_select.rs`:

```rust
use bevy::prelude::*;
use crate::{GameState, ScreenUi};

pub fn setup_level_select(mut commands: Commands) {
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
                Text::new("Press [1] — Level 1"),
                TextFont {
                    font_size: 40.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
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

What does `setup_level_select` do? It spawns a full-screen centered node with `ScreenUi` so `cleanup_screen_ui` can remove it later. The text is minimal — a debug draft that scales naturally when more levels are added.

What does `handle_level_select_input` query?
- `Res<ButtonInput<KeyCode>>` — to detect key presses.
- `ResMut<NextState<GameState>>` — to transition to `InGame`.

---

### Step 9: Create the GameOver screen

When the game ends, we need a screen that displays the outcome and waits for the player to press Space. It reads the `GameResult` resource set by `check_game_state` — note that `cleanup_level` intentionally does **not** remove `GameResult`, so the resource is still available during `OnEnter(GameOver)`. The root UI node carries `ScreenUi` for teardown. Create `src/game_over.rs`:

```rust
use bevy::prelude::*;
use crate::{GameState, GameResult, ScreenUi};

pub fn setup_game_over(
    mut commands: Commands,
    result: Res<GameResult>,
) {
    let message = match *result {
        GameResult::Defeat => "Game Over — the base was destroyed!",
        GameResult::Victory => "Victory — all enemies defeated!",
    };

    commands
        .spawn((
            ScreenUi,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(20.0),
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(message),
                TextFont {
                    font_size: 40.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            parent.spawn((
                Text::new("Press Space to continue"),
                TextFont {
                    font_size: 24.0,
                    ..default()
                },
                TextColor(Color::srgba(0.7, 0.7, 0.7, 1.0)),
            ));
        });
}

pub fn handle_game_over_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if keys.just_pressed(KeyCode::Space) {
        next_state.set(GameState::LevelSelect);
    }
}
```

What does `setup_game_over` query?
- `Commands` — to spawn UI nodes.
- `Res<GameResult>` — to determine which message to show.

> **Critical ordering detail:** `check_game_state` inserts `GameResult` via commands and sets `NextState` in the same system. Commands flush at the end of the `FixedUpdate` chain, so `GameResult` exists in the world before the state transition begins. Then `OnExit(InGame)` runs `cleanup_level`, which intentionally does **not** remove `GameResult`. Finally `OnEnter(GameOver)` runs `setup_game_over`, which reads the resource. If `cleanup_level` removed `GameResult`, the GameOver screen would panic trying to read a missing resource.

---

### Step 10: Wire the full schedule

In `main.rs`, replace the old flat system setup with state-driven schedules. Every gameplay system must be gated so it only runs during `InGame`:

```rust
fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .add_plugins(TilemapPlugin)
        .init_state::<GameState>()
        .add_systems(Startup, spawn_camera)
        // ---------- LevelSelect ----------
        .add_systems(OnEnter(GameState::LevelSelect), (
            setup_level_select,
            setup_tower_atlas,
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

Every gameplay system is gated with `.run_if(in_state(GameState::InGame))`. Without this gate, `FixedUpdate` systems would continue running during `GameOver`, spawning waves and moving enemies while the player reads the result.

The `OnEnter` chains ensure setup happens in order: level data loads before the tilemap spawns, and the tilemap exists before the placement preview appears.

With the new `level_select` and `game_over` modules, plus the shared `GameState` and `GameResult` types, `main.rs` needs an updated import list. Remove the old `Startup`-only setup symbols and bring in the state-screen systems and transition types:

```rust
use enemy::{
    BaseLives, GameFinished, SpawnSchedule,
    build_spawn_schedule, spawn_wave_enemies,
    move_enemies, process_base_reachers, check_game_state,
};
use tower::{
    PlacedTowers, setup_tower_atlas, spawn_placement_preview,
    update_placement_preview, place_tower_on_click,
    attack_enemies, despawn_timed,
};
use level_select::{setup_level_select, handle_level_select_input};
use game_over::{setup_game_over, handle_game_over_input};
```

Remove the old `Startup` chain that included `load_level_data` and `setup_spawn_schedule`. Those now live in `OnEnter(InGame)`.

---

### Step 11: Verify

```bash
cargo run
```

You should see:

- **Title screen** — "Press [1] — Level 1" centered on a dark background.
- Press `1` — the map loads, towers can be placed, waves spawn.
- **Let enemies reach the base** — after 5 leaks, the screen shows "Game Over — the base was destroyed!" and "Press Space to continue."
- **Place towers and win** — after all enemies die, the screen shows "Victory — all enemies defeated!"
- Press `Space` — returns to the title screen.
- Press `1` again — a fresh game starts. No lingering towers, enemies, or preview sprites from the previous round.

---

### Simplifications

| Simplification | Why it works | Future direction |
|---|---|---|
| **One hardcoded level** | `load_level_data` always loads `level_01.toml`. | Part 13 adds auto-discovery of `assets/levels/level_*.toml` files. |
| **Number-key level select** | Fast to build, no mouse handling needed. | Buttons, mouse selection, or a scrolling list for many levels. |
| **No save data** | High scores and progress are ephemeral. | A save file (JSON or TOML) could persist unlocked levels and best times. |
| **Camera spawned in Startup** | One camera for all screens avoids ordering issues. | Some games use different cameras per state (e.g., orthographic for gameplay, perspective for menus). |
| **`GameEntity` manual tagging** | Every spawn site must remember the tag. | A custom command or builder pattern could auto-attach `GameEntity` to all gameplay spawns. |
| **No animation transitions** | State changes are instant. | Fade-in/fade-out or slide transitions between screens. |

---

## Summary

- We defined a **three-state machine** (`LevelSelect`, `InGame`, `GameOver`) using Bevy's `States` derive and `.init_state()`.
- We added **`GameEntity`** and **`ScreenUi`** marker components to enable single-query cleanup of gameplay entities and UI nodes.
- We moved level loading from `Startup` into **`OnEnter(InGame)`** so each round starts fresh.
- We created **`cleanup_level`** (despawns `GameEntity` entities, removes resources) and **`cleanup_screen_ui`** (despawns `ScreenUi` nodes).
- We replaced `Local<bool>` with a **`GameFinished`** resource that is removed during cleanup, preventing stale state across replays.
- We added **`GameResult`** to decouple win/lose detection from the game-over display.
- We gated all gameplay systems with **`.run_if(in_state(GameState::InGame))`** so they only execute during active gameplay.
- We built minimal **LevelSelect** and **GameOver** screens with keyboard input and state transitions via `NextState`.

In **Part 13** we will add automatic level discovery (scanning `assets/levels/` for `.toml` files) and reorganize modules into focused files (`state.rs` for shared types, `gameplay.rs` for level loading) so `main.rs` stays readable as the app grows.
