# Part 11: Win/Lose Conditions — Closing the Loop

Enemies walk the path and towers shoot them. Now we close the game loop: the base has hit points, reaching enemies cost lives, and when all lives are gone or all enemies are dead, the game ends.

---

## What we will build

- **`BaseLives(i32)`** — a resource tracking the base's remaining HP. Initialized to 20.
- **`process_base_reachers`** — replaces the old `cleanup_finished_enemies`. Same query, but decrements lives before despawning.
- **`check_game_state`** — runs at the end of the FixedUpdate chain. Logs "Victory!" when all events are consumed and no enemies remain. Logs "Game Over" when lives hit zero.

Everything is output-only for now — a `Local<bool>` gate prevents spamming every frame.

---

## Why merge cleanup and base damage?

The old `cleanup_finished_enemies` queried `(With<Enemy>, Without<PathFollower>)` — enemies that finished the path. It despawned them. The new system adds one line: decrement `BaseLives`.

Splitting into two systems would work — chained systems don't conflict. But two systems for one logical operation adds unnecessary indirection. One system, one query, one iteration — cleaner.

---

## New resource

```rust
#[derive(Resource)]
pub struct BaseLives(pub i32);
```

Initialized to 5 in `main.rs`:

```rust
.insert_resource(BaseLives(5))
```

`insert_resource` (not `init_resource`) because `i32`'s default is 0.

---

## `process_base_reachers`

```rust
pub fn process_base_reachers(
    mut commands: Commands,
    mut lives: ResMut<BaseLives>,
    query: Query<Entity, (With<Enemy>, Without<PathFollower>)>,
) {
    for entity in &query {
        lives.0 -= 1;
        commands.entity(entity).despawn();
    }
}
```

Identical query to the old `cleanup_finished_enemies`. The only addition is `lives.0 -= 1` — one life lost per enemy that reaches the base.

---

## `check_game_state`

```rust
pub fn check_game_state(
    lives: Res<BaseLives>,
    schedule: Res<SpawnSchedule>,
    alive: Query<(), With<Enemy>>,
    mut finished: Local<bool>,
) {
    if *finished {
        return;
    }
    if lives.0 <= 0 {
        info!("Game Over — the base has been destroyed!");
        *finished = true;
    } else if schedule.events.is_empty() && alive.iter().count() == 0 {
        info!("Victory — all enemies defeated!");
        *finished = true;
    }
}
```

Two conditions:

| Condition | Trigger |
|---|---|
| Lose | `BaseLives <= 0` |
| Win | No events left in schedule AND no alive enemies |

The `Local<bool>` gate ensures the message prints once. `Local<T>` persists across system invocations — without it, the message would log every frame after the condition is met.

### Why not use an `Event`?

Events fire once but require an event reader system. For a single log message, `Local<bool>` is simpler and self-contained. We can switch to events when we add a restart prompt or UI.

### Stopping spawn after game over

When the game ends, `check_game_state` also inserts a `GameOver` resource:

```rust
#[derive(Resource, Default)]
pub struct GameOver;
```

`spawn_wave_enemies` checks for this resource and returns early — no more enemies spawn after defeat or victory:

```rust
pub fn spawn_wave_enemies(
    game_over: Option<Res<GameOver>>,
    // ...
 ) {
    if game_over.is_some() {
        return;
    }
```

`Option<Res<T>>` is Bevy's way to express "this resource may not exist yet" — it returns `None` during gameplay and `Some` after `check_game_state` inserts `GameOver`.
---

## Schedule

```rust
.add_systems(FixedUpdate, (
    spawn_wave_enemies,
    move_enemies,
    attack_enemies,
    process_base_reachers,   // was cleanup_finished_enemies
    check_game_state,        // new — runs last
).chain())
```

`process_base_reachers` and `check_game_state` run after combat. The order ensures the check reflects the current state after all actions for the tick.

---

## Running the project

```bash
cargo run
```

Expected behavior:

- Let enemies reach the base (don't place towers). After 5 enemies leak, the console prints:
  ```
  Game Over — the base has been destroyed!
  ```
- Place towers near the path. Kill all enemies. After the last enemy dies and the spawn queue empties:
  ```
  Victory — all enemies defeated!
  ```
- The message prints **once** — no log spam. After the message, **no more enemies spawn**.

---

## Design decisions

| Decision | Rationale |
|---|---|
| **Merge cleanup + lives in one system** | Same query, same entities. Splitting would require duplicated queries or `Without<T>` gating. |
| **`Local<bool>` gate** | Prevents per-frame log spam. Self-contained — no global state needed. |
| **`alive.iter().count()` for win check** | Direct and obvious. For thousands of enemies we'd use a counter, but the current scale makes the query trivial. |
| **`insert_resource(BaseLives(5))`** | 5 lives is enough to threaten the player but not instant loss. `i32::default()` 0 would be instant death. |

---

## Recap

In this part we:

1. Added `BaseLives(5)` resource — the base starts with 5 lives.
2. Replaced `cleanup_finished_enemies` with `process_base_reachers` — decrements lives before despawning.
3. Built `check_game_state` — logs "Victory!" when all enemies are dead, "Game Over" when the base falls.
4. Gated the check with `Local<bool>` so the message fires only once.
5. Chained `process_base_reachers` and `check_game_state` at the end of `FixedUpdate` so they reflect the tick's final state.
6. Added `GameOver` resource — inserted by `check_game_state`, checked by `spawn_wave_enemies` to stop further spawns.

The game loop is fully closed. Part 12 will tackle the multi-level architecture: a state machine for main menu / level selection / gameplay, on-demand level loading and cleanup, and resource lifecycle management — the foundation every additional level needs.
