# Part 11: Win/Lose Conditions — Closing the Loop

> **Time to read:** ~15 minutes  
> **New concepts:** `Local<T>`, `Option<Res<T>>`, marker resources  
> **Prerequisite:** Part 10 (waves and enemy spawning)

---

## Recap: What We Already Have

Towers shoot enemies, waves spawn on a timed schedule, and different enemy types move at different speeds. But the game never ends — enemies that reach the base simply disappear, and the player cannot lose.

---

## Goal: What We Will Build

We will close the game loop by giving the base hit points and detecting when the game is won or lost:

1. **`BaseLives`** — a resource tracking how many enemies can reach the base before defeat.
2. **`process_base_reachers`** — replaces `cleanup_finished_enemies`; enemies that finish the path now cost one life each.
3. **`GameOver` marker resource** — inserted when the game ends to signal other systems to stop.
4. **`check_game_state`** — detects victory (all waves done, all enemies dead) or defeat (lives exhausted) and logs the result once.

This matters because a tower defense without stakes is just a screensaver. Win/lose conditions turn the simulation into a game.

---

## New Bevy APIs & Concepts

### `Local<T>`

`Local<T>` is system-local state that persists across invocations of the same system. Unlike components (per-entity) or resources (global), `Local` is private to one system function. Bevy passes it as a mutable parameter, initialized to the type's `Default` value on the first run.

We use `Local<bool>` as a one-shot gate: once `check_game_state` detects a win or loss, it sets the flag to `true` and returns early on every subsequent frame. Without it, the log message would print 60 times per second forever.

> **Pitfall:** `Local` is scoped to the *system function*, not the entity or the world. Two different systems each get their own independent `Local<bool>`. Do not use `Local` to share state between systems — use a resource for that.

### `Option<Res<T>>`

Normally, `Res<T>` panics at runtime if the resource does not exist. `Option<Res<T>>` allows a system to gracefully handle the absence of a resource: it yields `None` when the resource is missing and `Some(Res<T>)` when present.

This is perfect for optional game state. During gameplay, `GameOver` does not exist, so `Option<Res<GameOver>>` is `None`. After `check_game_state` inserts it, the option becomes `Some`. `spawn_wave_enemies` can check `game_over.is_some()` without crashing during the first frame.

> **Pitfall:** `Option<ResMut<T>>` also exists, but mutably borrowing a non-existent resource does not create it — it simply returns `None`. If you need to insert a resource, use `Commands::insert_resource` or `ResMut<T>` on a resource that was already added.

### Marker resources (zero-sized types)

A marker resource is a zero-sized type used solely as a flag. It carries no data — its existence in the world is the signal.

```rust
#[derive(Resource, Default)]
pub struct GameOver;
```

`GameOver` has no fields, yet `Option<Res<GameOver>>` still works because Bevy tracks which resources exist independently of their contents. This pattern is common for state flags: the presence or absence of the resource is the entire API. In a future part we might replace this with a proper `States` enum, but a marker resource is the simplest way to broadcast "the game is over" to any system that cares.

---

## Walkthrough

### Designing the feature

Before writing code, think about what the player should see and what data that requires.

**Player-visible behavior:**

1. Enemies that reach the base deal damage — the base has a limited number of lives.
2. When lives hit zero, the game ends in defeat.
3. When all waves are exhausted and every enemy is dead, the game ends in victory.
4. After the game ends, no more enemies spawn and the result is logged exactly once.

**ECS data needed:**

- `BaseLives(pub i32)` resource — initialized to 5. Why 5? Enough to feel threatening without being instant loss.
- `process_base_reachers` system — same query as the old `cleanup_finished_enemies`, but decrements `BaseLives` before despawning.
- `GameOver` marker resource — inserted by `check_game_state` when either condition is met.
- `Option<Res<GameOver>>` in `spawn_wave_enemies` — returns early when the marker exists.
- `Local<bool>` in `check_game_state` — prevents log spam after the result is known.

**Design decision: merge cleanup and lives into one system.** The old `cleanup_finished_enemies` already queried `(With<Enemy>, Without<PathFollower>)` — enemies that finished the path. Adding `lives.0 -= 1` before the despawn is one extra line in the same loop. Splitting into two systems would work (chained systems don't conflict), but two systems for one logical operation adds unnecessary indirection. One system, one query, one iteration — cleaner.

---

### Step 1: Add `BaseLives`

In `src/enemy.rs`, add the resource:

```rust
#[derive(Resource)]
pub struct BaseLives(pub i32);
```

`BaseLives` is a tuple struct wrapping `i32`. We use `i32` instead of `f32` because lives are discrete — an enemy either reaches the base or it doesn't; there is no partial damage yet.

In `main.rs`, initialize it with `insert_resource`:

```rust
.insert_resource(BaseLives(5))
```

> **Why `insert_resource` instead of `init_resource`?** `init_resource` calls `Default::default()`. We *could* implement `Default for BaseLives` to return 5, but `insert_resource(BaseLives(5))` is shorter and naturally allows different levels to specify different starting lives.

---

### Step 2: Replace `cleanup_finished_enemies`

Rename the old cleanup system to `process_base_reachers` and add the life decrement. What must it do?

1. Query all enemies that have lost their `PathFollower` (they finished the path).
2. For each, subtract one life.
3. Despawn the entity.

What does it query?
- `ResMut<BaseLives>` — to decrement remaining lives.
- `Query<Entity, (With<Enemy>, Without<PathFollower>)>` — enemies that reached the base.
- `Commands` — to despawn entities.

Add it to `src/enemy.rs`:

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

The query is identical to the old `cleanup_finished_enemies`. The only addition is `lives.0 -= 1` — one life lost per enemy that reaches the base. The despawn still happens so the enemy disappears from the map.

---

### Step 3: Add the `GameOver` marker

In `src/enemy.rs`, add a zero-sized marker resource:

```rust
#[derive(Resource, Default)]
pub struct GameOver;
```

`Default` is required because Bevy may need to construct the type automatically in some contexts. Since `GameOver` has no fields, the derived default is a no-op.

---

### Step 4: Add `check_game_state`

Now we write the system that decides whether the player has won or lost. What must it check?

1. **Defeat:** `BaseLives` has dropped to zero or below.
2. **Victory:** The spawn schedule is empty *and* no enemies are alive.

Both conditions are checked against the world state at the start of `check_game_state`'s execution. `process_base_reachers` deducts lives and queues despawns immediately, but queued despawns are not visible to the query until the end of the tick. In practice this means defeat and victory may be detected one frame later than the triggering event — acceptable for a console log.

What does it query?
- `Commands` — to insert the `GameOver` resource when the game ends.
- `Res<BaseLives>` — to check for defeat.
- `Res<SpawnSchedule>` — to check if any spawn events remain.
- `Query<(), With<Enemy>>` — to count living enemies (we only need existence, so `()` is the data type).
- `Local<bool>` — a private flag so the message prints once.

Add it to `src/enemy.rs`:

```rust
pub fn check_game_state(
    mut commands: Commands,
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
        commands.insert_resource(GameOver);
        *finished = true;
    } else if schedule.events.is_empty() && alive.iter().count() == 0 {
        info!("Victory — all enemies defeated!");
        commands.insert_resource(GameOver);
        *finished = true;
    }
}
```

The `finished` flag starts as `false` (the `Default` for `bool`). Once either condition triggers, it flips to `true` and the system returns immediately on every subsequent frame.

> If you're coming from C or C++, think of `Local<T>` as a function-scope `static` variable — it persists across calls but is invisible outside the function. The difference is that Bevy injects it as a parameter rather than declaring it inside the body, and its lifetime is tied to the app, not the process.

The `alive.iter().count() == 0` check needs both conditions because enemies might still be alive when the last spawn event is consumed. The schedule empties once all waves are processed, but the final group of enemies may still be walking the path. Only when *both* the queue is empty and the enemy count is zero can we declare victory.

> **Note on efficiency:** Bevy offers three ways to ask a query about its matches, with different trade-offs:
> - **`alive.is_empty()`** — preferred for existence checks. It is O(1): it checks whether any matched archetype contains at least one entity, without enumeration.
> - **`alive.count()`** — preferred for getting a total. It may skip per-entity iteration when the query is archetypal, doing work proportional to matched tables instead.
> - **`alive.iter().count()`** — explicit but always O(number of enemies). We use it here because it makes the iterator-based mental model obvious to readers new to Bevy. For a shipping game you'd use `is_empty()` for the victory check.

> **Why not use an `Event` for the game-over signal?** Events fire once but require a separate event-reader system. For a single log message, `Local<bool>` is simpler and self-contained. In Part 12 we will replace the marker resource with a proper `States` enum, which is the idiomatic Bevy way to handle game state.

---

### Step 5: Stop spawning after game over

When `check_game_state` inserts `GameOver`, `spawn_wave_enemies` must return early so no more enemies appear. How do we make a system conditional on a resource that may not exist?

Add `Option<Res<GameOver>>` as the first parameter of `spawn_wave_enemies`. During gameplay the resource is absent, so the option is `None`. After game over it is `Some`:

```rust
pub fn spawn_wave_enemies(
    game_over: Option<Res<GameOver>>,
    mut schedule: ResMut<SpawnSchedule>,
    mut commands: Commands,
    level: Res<LevelData>,
    time: Res<Time>,
) {
    if game_over.is_some() {
        return;
    }
    // ... rest of the function unchanged
}
```

`Option<Res<T>>` is Bevy's way to express "this resource may not exist yet." It returns `None` during gameplay and `Some` after `check_game_state` inserts `GameOver`. The parameter order matters: Bevy matches parameters by type, so `Option<Res<GameOver>>` must come before `ResMut<SpawnSchedule>` so the scheduler can distinguish the two resource borrows.

---

### Step 6: Wire up the schedule

In `main.rs`, replace `cleanup_finished_enemies` with `process_base_reachers` and add `check_game_state` at the end of the `FixedUpdate` chain:

```rust
.add_systems(FixedUpdate, (
    spawn_wave_enemies,
    move_enemies,
    attack_enemies,
    process_base_reachers,
    check_game_state,
).chain())
```

The order is deliberate:

1. `spawn_wave_enemies` — may create new enemies.
2. `move_enemies` — moves all enemies, possibly removing `PathFollower` from base-reachers.
3. `attack_enemies` — may kill enemies, removing them from the world.
4. `process_base_reachers` — despawns enemies that reached the base and decrements lives.
5. `check_game_state` — evaluates the final state after all actions for this tick.

Update the imports in `main.rs`:

```rust
use enemy::{
    build_spawn_schedule, spawn_wave_enemies,
    move_enemies, process_base_reachers, check_game_state,
    BaseLives,
};
```

> Note: `GameOver` is not imported in `main.rs` — it is only constructed and consumed inside `enemy.rs`.

---

### Step 7: Verify

```bash
cargo run
```

You should see:

- The same `15×10` map, click-to-place towers, and wave spawning from Part 10.
- **Let enemies reach the base** (don't place towers). After 5 enemies leak, the console prints:
  ```
  Game Over — the base has been destroyed!
  ```
  No further enemies spawn.
- **Place towers near the path.** Kill all enemies. After the last enemy dies and the spawn queue empties:
  ```
  Victory — all enemies defeated!
  ```
  No further enemies spawn.
- The message prints **once** — no log spam.

---

### Simplifications

| Simplification | Why it works | Future direction |
|---|---|---|
| **Output-only feedback (log, no UI)** | Console output is enough to prove the condition fires. | Part 12 adds a `GameState` enum and a proper game-over screen with restart. |
| **`alive.iter().count()` for win check** | Direct and obvious for our scale. | With thousands of enemies, a counter resource updated on spawn/despawn would be faster than querying every frame. |
| **5 lives, hardcoded** | Enough to threaten the player but not instant loss. | Difficulty could scale per level or be configurable in the TOML. |
| **Marker resource instead of States** | Simpler than Bevy's `States` for a single binary flag. | Part 12 replaces `GameOver` with a proper `States` enum for menu → gameplay → game-over transitions. |
| **No restart** | The game ends and stays ended. | A restart button would despawn all game entities, reset resources, and rebuild the schedule. |

---

## Summary

- We added `BaseLives(5)` — the base starts with 5 lives, initialized with `insert_resource` because `i32::default()` is `0`.
- We replaced `cleanup_finished_enemies` with `process_base_reachers` — same query, but now decrements lives before despawning base-reachers.
- We built `check_game_state` — checks for defeat (lives ≤ 0) and victory (empty schedule + no living enemies), then inserts a `GameOver` marker resource.
- We gated `spawn_wave_enemies` with `Option<Res<GameOver>>` so spawning stops once the game ends.
- We used `Local<bool>` to ensure the win/lose message logs exactly once, not every frame.
- We chained `process_base_reachers` before `check_game_state` so the state check reflects the tick's final state.

In **Part 12** we will refactor the app into a proper state machine using Bevy's `States`: a level select screen, on-demand level loading, and clean teardown between rounds — the foundation for multiple playable levels.
