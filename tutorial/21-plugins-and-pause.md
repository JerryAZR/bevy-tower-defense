# Part 21: Plugins — AudioPlugin and Pause Overlay

> **Time to read:** ~10 minutes
> **New concepts:** `Plugin` trait, sub-state pattern, `.and()` combinator
> **Prerequisite:** Part 20 (audio)

---

## Recap: What We Already Have

We added background music and sound effects in Part 20. The audio systems work, but every registration — loading, music start/stop, SFX consumer — lives directly in `main.rs`. The file is getting crowded, and there is no obvious boundary between domain groups.

---

## Goal: What We Will Build

Two things that together teach one concept:

1. **Extract `AudioPlugin`** — move all audio registrations out of `main.rs` and into `src/audio.rs`. The game behaves identically.
2. **Build `PausePlugin`** — press Escape to freeze the simulation and show a "PAUSED" overlay. A separate state machine (`PauseState`) controls whether gameplay systems run.

The common thread is the `Plugin` trait, Bevy's standard mechanism for organizing code into reusable, self-contained units.

---

## New Bevy APIs & Concepts

### `Plugin`

A `Plugin` is a struct that implements:

```rust
impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) { ... }
}
```

The `build` method receives `&mut App`, the same builder you use in `main()`. That means anything you can do in `main.rs` — register resources, add systems, add message types, configure state transitions — you can do inside a plugin.

Plugins exist because real projects outgrow a single `main()` function. Every Bevy application beyond a minimal example is organized into plugins: `DefaultPlugins`, `TilemapPlugin`, and now your own.

### Orthogonal state machines

Bevy supports multiple independent `States` types simultaneously. This is useful when two aspects of your app need separate lifecycles:

| State type | What it controls |
|-----------|-----------------|
| `GameState` | **What screen** the player sees (LevelSelect / InGame / GameOver) |
| `PauseState` | **Whether the simulation ticks** (Running / Paused) |

These are orthogonal — you can be in `InGame + Running` (normal play) or `InGame + Paused` (frozen). The transition between screens (`GameState`) is unaffected by the pause state.

### `.and()` combinator

`RunCondition` closures can be combined with `.and()` to require multiple conditions:

```rust
.run_if(in_state(GameState::InGame).and(in_state(PauseState::Running)))
```

Both conditions must be true for the system to run. Bevy also provides `.or()` for alternatives and `.not()` for negation.

---

## Walkthrough

### Before we start: what stays in `main.rs` versus what moves to plugins

The reader might wonder: *what is the right granularity for a plugin?*

There is no single answer. Some projects have one plugin per file. Others group related subsystems (economy, enemies, UI) into larger plugins and compose them in `main.rs`. The pattern we follow here is: **one domain, one plugin**. Audio is a self-contained domain. Pausing is another.

Code that **spans multiple domains** — like placing a tower, which involves the tower module, the economy module, and input — stays in `main.rs` where ordering constraints are visible in one place. Plugins are not a substitute for `main.rs`; they are a tool for moving self-contained groups out of it.

### Step 1: AudioPlugin — a pure refactor

Before writing code, look at what `main.rs` currently does with audio. It:

1. Imports four systems and a message type
2. Registers `PlaySound` as a message type
3. Adds `load_audio_assets` to the startup chain
4. Adds `start_background_music` to the `OnEnter(InGame)` chain
5. Removes `stop_background_music` on `OnExit(InGame)`
6. Adds `play_sound_effects` to the `Update` schedule

Six touch-points scattered across the file. All of them belong together.

**Create `AudioPlugin` in `src/audio.rs`.** The struct is minimal — it carries no data, just implements `Plugin`:

```rust
pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_message::<PlaySound>()
            .add_systems(Startup, load_audio_assets)
            .add_systems(OnEnter(GameState::InGame), start_background_music)
            .add_systems(OnExit(GameState::InGame), stop_background_music)
            .add_systems(Update, play_sound_effects
                .run_if(in_state(GameState::InGame)));
    }
}
```

Notice the pattern: the same `.add_systems()` and `.add_message()` calls you used in `main.rs`, now inside `build()`. The plugin also needs `use crate::state::GameState;` at the top of `src/audio.rs` to reference the state type.

**Replace the six touch-points in `main.rs`.** The import contracts from four system names and a type to a single plugin:

```rust
use audio::AudioPlugin;

.add_plugins(AudioPlugin)

Remove `.add_message::<PlaySound>()`, remove `load_audio_assets` from the startup tuple, remove `start_background_music` from the `OnEnter` tuple, remove `stop_background_music` from `OnExit`, and remove `play_sound_effects` from the `Update` tuple. They are all handled by the plugin now.

> **Run the game now.** The game plays identically — same music, same SFX. Nothing has changed from the player's perspective. You have just moved code, not changed behavior.

This is the signature of a clean refactor: zero observable difference, but a measurably simpler `main.rs`.

### Step 2: PauseState — a separate state machine

Adding pause as a new `GameState` variant (`LevelSelect / InGame / Paused / GameOver`) would cause a problem: `OnExit(GameState::InGame)` fires on any transition *out* of `InGame`, including a transition to `Paused`. That would trigger `cleanup_level` and destroy the level.

The solution is a second, orthogonal state:

```rust
// in src/state.rs
#[derive(States, Default, Clone, PartialEq, Eq, Hash)]
pub enum PauseState {
    #[default]
    Running,
    Paused,
}
```

`GameState` now handles only what screen the player sees. `PauseState` handles whether the game is ticking. They never interfere with each other.

### Step 3: PausePlugin — building a new feature as a plugin

Create `src/pause.rs`. This is a fresh module built as a plugin from the start, which demonstrates that the plugin pattern applies to new code, not just refactored code.

**Designing the pause feature.** Before writing any systems, think about what the player should observe:

1. **Press Escape to freeze.** The instant the player presses Escape, every enemy stops moving, every projectile freezes mid-flight, every tower stops attacking, and gold income halts.
2. **A "PAUSED" overlay appears** on top of the game, semi-transparent so the frozen battlefield is still visible behind it.
3. **Press Escape again to resume.** Everything picks up exactly where it left off.
4. **Only works during a level.** Pressing Escape from the level select screen or the game-over screen should do nothing.

From these rules we derive three systems:

- **`toggle_pause`** — reads keyboard input and flips the `PauseState` between `Running` and `Paused`. Guarded against toggling outside of `InGame`, and against a frame where the game has ended.
- **`spawn_pause_overlay`** — fires on `OnEnter(PauseState::Paused)`, creates a full-screen semi-transparent overlay with centered "PAUSED" text.
- **`despawn_pause_overlay`** — fires on `OnExit(PauseState::Paused)`, removes every entity marked with `PauseOverlay`.

We also need a private `PauseOverlay` marker component so the overlay entities can be found and cleaned up without exposing them to other modules.

**The toggle system** runs in `Update` and listens for Escape. What it queries:

| Parameter | Purpose |
|-----------|---------|
| `Res<State<GameState>>` | Which screen the player is on — only toggle while in a level |
| `Res<State<PauseState>>` | The current pause state, so we can flip it |
| `Option<Res<GameFinished>>` | Whether `check_game_state` has already determined the game is over this frame |
| `Res<ButtonInput<KeyCode>>` | The current frame's keyboard state |
| `ResMut<NextState<PauseState>>` | Where we queue the new pause state |

```rust
fn toggle_pause(
    game_state: Res<State<GameState>>,
    pause_state: Res<State<PauseState>>,
    game_finished: Option<Res<GameFinished>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_pause: ResMut<NextState<PauseState>>,
) {
    if !keyboard.just_pressed(KeyCode::Escape) { return; }
    if *game_state.get() != GameState::InGame { return; }

    // Guard: don't pause if the game ended this frame — the GameOver
    // transition hasn't applied yet, but GameFinished was already inserted.
    if game_finished.is_some() { return; }

    match pause_state.get() {
        PauseState::Running => next_pause.set(PauseState::Paused),
        PauseState::Paused => next_pause.set(PauseState::Running),
    }
}
```

**The race condition guard** deserves extra attention. Consider this scenario:

1. `check_game_state` (in `FixedUpdate`) detects that the last enemy reached the base
2. It inserts a `GameFinished` resource and writes `NextState(GameOver)`
3. In the same frame, `toggle_pause` (in `Update`) reads `game_state.get()` — which is still `GameState::InGame`, because state transitions are deferred to the next frame
4. Without the guard, `toggle_pause` would allow the pause overlay to spawn

The resource, however, is available immediately because `Commands` (used to insert `GameFinished`) flush at every schedule boundary. `FixedUpdate` runs before `Update` in the same frame, so by the time `toggle_pause` executes, `GameFinished` already exists. The guard catches the gap between resource insertion and state transition.

**The overlay systems** spawn and despawn the visual feedback:

```rust
fn spawn_pause_overlay(mut commands: Commands) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            display: Display::Flex,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
        PauseOverlay,
    )).with_child((
        Text::new("PAUSED"),
        TextFont { font_size: 64.0, ..default() },
        TextColor(Color::WHITE),
    ));
}

fn despawn_pause_overlay(mut commands: Commands, overlay: Query<Entity, With<PauseOverlay>>) {
    for entity in &overlay { commands.entity(entity).despawn(); }
}
```

The `PauseOverlay` marker is private to this module. `spawn_pause_overlay` runs on `OnEnter(PauseState::Paused)` and `despawn_pause_overlay` on `OnExit(PauseState::Paused)` — a clean lifecycle: enter → spawn, exit → despawn. You could also spawn the overlay once and toggle its `Visibility`, but pause is infrequent enough that spawn/despawn is simpler: no entity handle to store, no extra teardown on level exit.

**The plugin** wires everything together:

```rust
pub struct PausePlugin;

impl Plugin for PausePlugin {
    fn build(&self, app: &mut App) {
        app
            .init_state::<PauseState>()
            .add_systems(Update, toggle_pause)
            .add_systems(OnEnter(PauseState::Paused), spawn_pause_overlay)
            .add_systems(OnExit(PauseState::Paused), despawn_pause_overlay);
    }
}
```

Notice two things. First, `PauseState` is initialized here (not in `main.rs`). Second, the overlay systems only react to `PauseState` — they don't reference `GameState` at all. The pause-only-during-levels constraint lives inside `toggle_pause` via the `game_state.get() != GameState::InGame` check, so the overlay systems stay simple. (You could also move that constraint out by adding `.run_if(in_state(GameState::InGame))` to the toggle system.)

### Step 4: Wiring the pause condition in `main.rs`

In `main.rs`, add `.and(in_state(PauseState::Running))` to every gameplay system group. Each of the three groups changes the same way:

```rust
// Before
).chain().run_if(in_state(GameState::InGame)))

// After
).chain().run_if(in_state(GameState::InGame).and(in_state(PauseState::Running))))
```

The queries this affects:

| Schedule | Systems | Effect when paused |
|----------|---------|-------------------|
| `FixedUpdate` | enemy spawning, movement, tower attacks, ammo, projectile physics, economy | No simulation ticks |
| `Update` (group 1) | placement preview, tower placement, HUD updates, timed despawning, range gizmos | No UI changes, no tower placement |
| `Update` (group 2) | scroll-to-cycle, number key selection, dock selection highlight, dock click | No dock interaction |

When `PauseState` transitions to `Paused`, all three groups stop — every enemy freezes mid-march, every projectile stops mid-flight, every tower stops mid-attack. The overlay appears. Nothing moves until the player presses Escape again.

> **Run the game now.** Start a level. Press Escape. The game freezes — enemies, projectiles, towers all stop. A semi-transparent black overlay with centered "PAUSED" text covers the screen. Press Escape again. Everything resumes.


---

## Simplifications

- **No death-while-paused guard.** If the game would end while the player is paused (e.g., a timer-based loss condition), the pause state would block the check. Our loss condition is enemy-driven, and enemies don't move while paused, so this is safe.
- **All InGame systems are paused.** Some games let the player scroll the tower dock or view the map while paused. For simplicity we freeze everything, but you could relax this by omitting `.and(in_state(PauseState::Running))` from specific system groups.
- **Audio does not pause.** Background music and looping projectile thrusters continue playing while the game is frozen. Bevy's audio system runs independently of ECS schedules — pausing it would require either swapping playback modes on the fly or reaching into `AudioSink` directly.

---

## Summary

- **`Plugin`** encapsulates systems, resources, and state into self-contained units. Anything you can do in `main.rs`, you can do inside `Plugin::build()`.
- **AudioPlugin** was a pure refactor — zero behavioral change, but it reduced six registration lines in `main.rs` to one `.add_plugins()` call.
- **PausePlugin** was built as a new feature inside a plugin, proving the pattern extends to fresh code.
- **Orthogonal state machines** let you separate "what screen" from "whether the simulation runs." `PauseState` is independent of `GameState`.
- **Race conditions** between parallel systems can be caught with resource-based guards — `GameFinished` confirms the game ended even before the state transition applies.
