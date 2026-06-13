# Part 22: Custom Run Conditions & System Sets

> **Time to read:** ~12 minutes
> **New concepts:** custom run conditions, `SystemSet` enum, `configure_sets`, `in_set`
> **Prerequisite:** Part 21 (Plugins)

---

## Recap: What We Already Have

Part 21 introduced the `Plugin` trait and an orthogonal `PauseState`. The pause condition is wired into three places in `main.rs` — every gameplay system chain repeats the same `.run_if(in_state(GameState::InGame).and(in_state(PauseState::Running)))`. It works, but the repetition is brittle: if we add a fourth gameplay chain, we must remember to copy the condition.

---

## Goal: What We Will Build

Two small changes that together eliminate the repetition:

1. **`game_is_running`** — a named run condition that captures "in a level and not paused" in one place.
2. **`GameplaySet`** — named system sets (`Simulation`, `Interaction`, `TowerDock`) that make the system architecture visible and let us apply the condition once per set instead of once per chain.

The game behaves identically. The difference is in `main.rs`: three identical run conditions collapse into one concept applied in one place.

---

## New Bevy APIs & Concepts

### Custom run conditions

A run condition is any function that takes system parameters and returns `bool`:

```rust
fn my_condition(query: Query<&Health>) -> bool { ... }
```

You pass it to `.run_if(my_condition)`. Bevy's type system recognizes the `fn(...) -> bool` signature and implements the `Condition` trait for it. This means you can write conditions as plain functions — no magic macro, no special return type.

### `SystemSet` enum

A `SystemSet` is an enum (or struct) that labels a group of systems:

```rust
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum MySet { A, B }
```

Sets give names to groups of systems, which serves two purposes:

- **Ordering:** you can declare that all systems in `SetA` run before all systems in `SetB`.
- **Conditions:** you can apply a run condition to an entire set via `configure_sets`, and every system in that set inherits it.

### `configure_sets` and `in_set`

- `.configure_sets(Schedule, MySet.run_if(my_condition))` attaches a condition to the set for a given schedule.
- `.in_set(MySet::A)` on a system or chain assigns it to the set, so it inherits whatever is configured on that set.
- **These two are independent.** `configure_sets` does not schedule systems — it only configures metadata. `in_set` does not apply conditions — it only assigns membership. If you `in_set` a system into a schedule without a matching `configure_sets`, the system runs unconditionally. Keep them in sync per schedule.

---

## Walkthrough

### Step 1: `game_is_running` — a custom run condition

Open `src/state.rs`. The three identical conditions in `main.rs` all check the same two things:

1. `GameState` is currently `InGame`
2. `PauseState` is currently `Running`

Extract that into a single function:

```rust
/// Returns `true` when the player is in a level and the game is not paused.
pub fn game_is_running(
    game_state: Res<State<GameState>>,
    pause_state: Res<State<PauseState>>,
) -> bool {
    *game_state.get() == GameState::InGame && *pause_state.get() == PauseState::Running
}
```

The function takes two `Res` parameters — the same system parameters you'd use in any system — and returns `bool`. That's all a custom run condition needs.

Notice the `*` dereference: `game_state.get()` returns `&GameState` (because `Res<State<T>>` yields a reference), so we compare with `==`.

### Step 2: `GameplaySet` — naming the system groups

Still in `src/state.rs`, add an enum above `game_is_running`:

```rust
/// Named system sets for the gameplay phase.
/// `configure_sets` applies the `game_is_running` condition once,
/// instead of repeating it on every `.add_systems` chain.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameplaySet {
    /// FixedUpdate: enemies, towers, economy.
    Simulation,
    /// Update: placement preview, tower placement, HUD, gizmos.
    Interaction,
    /// Update: scroll, number keys, click on the tower dock.
    TowerDock,
}
```

The `SystemSet` derive macro makes this enum usable in `.in_set()` and `.configure_sets()`. The variant names describe what each group does — this is documentation that lives in code, visible whenever someone reads `main.rs`.

Why three variants? Because our `main.rs` has three gameplay system chains: one in `FixedUpdate` (simulation), two in `Update` (interaction and tower dock). Each chain will become one set.

### Step 3: `configure_sets` — apply the condition once

Open `src/main.rs` and add your imports:

```rust
use state::{GameState, GameplaySet, game_is_running, AvailableLevels, spawn_camera, cleanup_level, cleanup_screen_ui};
```

`PauseState` is no longer imported — the condition function reads it internally.

Now add `configure_sets` calls anywhere before the systems that reference the sets. A natural place is right after `.add_message::<PlaceTower>()`:

```rust
.configure_sets(FixedUpdate, GameplaySet::Simulation.run_if(game_is_running))
.configure_sets(Update, (
    GameplaySet::Interaction,
    GameplaySet::TowerDock,
).run_if(game_is_running))
```

`.configure_sets` takes a schedule and a set config. The tuple `(Interaction, TowerDock)` shows that you can configure multiple sets at once — both get the same condition.

This is the key: the condition `game_is_running` is now declared in one place. Every system assigned to these sets will automatically inherit it.

### Step 4: `in_set` — assign systems to their sets

Replace each `.run_if(...)` with `.in_set(...)` on the three system chains:

| Before | After |
|--------|-------|
| `.chain().run_if(in_state(GameState::InGame).and(in_state(PauseState::Running)))` | `.chain().in_set(GameplaySet::Simulation)` |
| `.run_if(in_state(GameState::InGame).and(in_state(PauseState::Running)))` | `.in_set(GameplaySet::Interaction)` |
| `.run_if(in_state(GameState::InGame).and(in_state(PauseState::Running)))` | `.in_set(GameplaySet::TowerDock)` |

The `FixedUpdate` chain uses `.chain().in_set(...)` because it has ordering dependencies between systems. The `Update` chains use bare `.in_set(...)` because their systems run in parallel. Either way works.

> **Run the game now.** Start a level, press Escape. The game freezes. Press Escape again. Everything resumes. Same behavior, cleaner code.

### What changed in `main.rs`

Three identical `.run_if(...)` lines became three descriptive `.in_set(GameplaySet::...)` lines. The "am I paused?" logic moved to one function in `state.rs`. If we ever add a fourth gameplay system group, we just `.in_set(GameplaySet::Simulation)` and the condition is already taken care of — no copy-paste.

> **A note on scale.** In a project this small, the win might feel incremental — we replaced three lines with three slightly different lines. The real value surfaces in a game with twenty system groups, each guarded by a multi-condition check like `in_state(InGame).and(in_state(Running)).and(not(resource_exists::<CutscenePlaying>()))`. Centralizing that into one named condition and one set config means you change it in one place and never forget a chain.

---

## Simplifications

- **No intra-set ordering via `configure_sets`.** We rely on `.chain()` for ordering within a set and have no declared ordering *between* sets. In a larger project you might add `.configure_sets(Update, GameplaySet::Simulation.before(GameplaySet::Interaction))`, but our parallel groups have no ordering requirements.
- **`GameplaySet` covers only gameplay.** The `LevelSelect` and `GameOver` systems still use `in_state(GameState::LevelSelect)` and `in_state(GameState::GameOver)` directly — those are single-state checks that don't benefit from a named condition.

---

## Summary

- **Custom run conditions** are plain functions with system parameters that return `bool`. No special trait impl needed — the signature `fn(SomeParam) -> bool` is all Bevy requires.
- **`SystemSet` enums** give names to groups of systems. `#[derive(SystemSet)]` plus `Debug, Clone, PartialEq, Eq, Hash` is the pattern.
- **`configure_sets(Schedule, Set.run_if(condition))`** applies a run condition to every system in the set for that schedule.
- **`.in_set(MySet::Variant)`** assigns a system or chain to a set, replacing repeated `.run_if(...)` calls.
