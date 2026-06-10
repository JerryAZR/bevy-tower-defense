# Part 18: Custom Events — Decoupling with Messages

> **Time to read:** ~15 minutes
> **New concepts:** `#[derive(Message)]`, `MessageWriter<T>`, `MessageReader<T>`, `.after()` system ordering
> **Prerequisite:** Part 17 (tower selection UI)

---

## Recap: What We Already Have

The player can select towers from a dock and click the map to place them. Right now `place_tower_on_click` is a "god function": it validates the tile, checks gold, deducts cost, updates the placed-tiles set, spawns the tower entity, and flashes the preview on denial. These are three separate concerns jammed into one system.

---

## Goal: What We Will Build

We split tower placement into **one producer + two consumers** linked by a custom message:

| Concern | System | Module |
|---|---|---|
| Input validation | `place_tower_on_click` | `tower.rs` |
| Entity spawning | `spawn_tower_from_event` | `tower.rs` |
| Economy / bookkeeping | `deduct_gold_on_placement` | `economy.rs` |

The producer emits a `PlaceTower` message. The two consumers each read it independently. Neither consumer knows the other exists.

This teaches **decoupled system design** — one of the most important patterns in non-trivial Bevy projects.

---

## New Bevy APIs & Concepts

### `#[derive(Message)]`

In Bevy 0.18, the event system was replaced by a message system. A *message* is a transient broadcast value: you write it in one system and read it in others. Unlike components (attached to entities) or resources (global singletons), messages are fire-and-forget — they live for two frames, then are automatically cleared.

```rust
#[derive(Message)]
pub struct PlaceTower {
    pub tile: [u32; 2],
    pub world_pos: Vec2,
    pub tower_id: usize,
    pub cost: u32,
}
```

Every message type must be registered with `.add_message::<PlaceTower>()` on the `App`. Without this, `MessageWriter` and `MessageReader` panic at runtime.

### `MessageWriter<T>` and `MessageReader<T>`

`MessageWriter<T>` is the sending side. You call `.write(message)` to broadcast a value:

```rust
mut place_events: MessageWriter<PlaceTower>,

place_events.write(PlaceTower { tile, world_pos, tower_id, cost });
```

`MessageReader<T>` is the receiving side. Each system gets its own reader with its own cursor. Multiple systems reading the same message type do not interfere with each other — every reader sees every message independently.

```rust
mut events: MessageReader<PlaceTower>,

let mut iter = events.read();
let Some(event) = iter.next() else { return; };
assert!(iter.next().is_none(), "expected at most one message per frame");
// process event
```

> **Pitfall:** `.read()` returns an iterator, but the reader also advances an internal cursor. Calling `.read()` twice in the same system on the same reader will yield nothing the second time — the messages have already been consumed by this system.

### System ordering with `.after()`

In the message architecture, the producer emits and the consumers react. That relationship only makes sense if the producer runs first. Bevy does not guarantee execution order within a schedule unless you specify it, so without `.after()` a consumer could theoretically run before the producer in a given frame.

The message would still be delivered — messages persist for two frames — but the consumer would react one frame late. The player would never notice a single frame of delay, but the architecture would be harder to reason about: you'd be relying on message persistence rather than immediate reaction. Explicit ordering keeps the mental model clean.

```rust
.add_systems(Update, (
    spawn_tower_from_event.after(place_tower_on_click),
    deduct_gold_on_placement.after(place_tower_on_click),
))
```

`.after(system_name)` tells Bevy's scheduler: "run this system no earlier than `system_name` in the same schedule." It does not guarantee immediate adjacency — other systems may run between them — but it does guarantee the producer has finished before the consumers start.

> **`.after()` vs. `.chain()`** — `.chain()` serializes every system in a tuple in strict order, which also achieves ordering but over-constrains unrelated systems. `.after()` lets you draw dependency edges between specific pairs without forcing everything else to wait. If you find dependency graphs hard to think about, `.chain()` is perfectly fine for a small project: strict ordering only becomes a performance concern once you have enough systems that Bevy's scheduler could usefully parallelize them. You can always switch to `.after()` later.

## Walkthrough

### Designing the refactor

Before writing code, think about what each concern needs:

**Input validation** needs: mouse state, camera, map layout, placed tiles, gold balance, tower registry, selected tower type. It does not need to know how towers are spawned or how gold is stored.

**Entity spawning** needs: the tower definition, the world position, and `Commands`. It does not need to know about the mouse, gold, or whether the tile was affordable.

**Economy bookkeeping** needs: the tower cost and the tile coordinates. It does not need to know about sprites, cameras, or input devices.

From this we derive `PlaceTower` — the minimal data both consumers need:

- `tile` — so bookkeeping can mark it occupied.
- `world_pos` — so spawning doesn't recompute it.
- `tower_id` — so spawning can look up the definition.
- `cost` — so bookkeeping doesn't need the registry.

> **Why include `cost` in the message?** The producer already looked up the tower definition to check affordability. Including `cost` avoids a second registry lookup in the economy consumer. In a larger project you might pass the entire `TowerDefinition` clone, but four fields are enough for our needs.

### The message type

In `src/tower.rs`, add `PlaceTower` after `SelectedTowerType`:

```rust
#[derive(Message)]
pub struct PlaceTower {
    pub tile: [u32; 2],
    pub world_pos: Vec2,
    pub tower_id: usize,
    pub cost: u32,
}
```

### Refactoring the producer

`place_tower_on_click` now has a single responsibility: validate input and emit a message on success. It no longer spawns entities, deducts gold, or updates `PlacedTowers`.

What does it query now?
- `Res<Gold>` — read-only affordability check.
- `Res<PlacedTowers>` — read-only, to validate the tile is unoccupied.
- `MessageWriter<PlaceTower>` — to broadcast the placement.
- `Res<TowerRegistry>` and `Res<SelectedTowerType>` — to look up cost.
- `Commands` — still needed for the denied-flash feedback.

```rust
pub fn place_tower_on_click(
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform)>,
    map_layout: Res<MapLayout>,
    placed: Res<PlacedTowers>,
    gold: Res<Gold>,
    preview_q: Query<Entity, With<TowerPreview>>,
    mut commands: Commands,
    mut place_events: MessageWriter<PlaceTower>,
    registry: Res<TowerRegistry>,
    selected: Res<SelectedTowerType>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    let (cam, cam_transform) = *camera;

    let Some(tile) = hovered_placeable_tile(
        &window, &cam, &cam_transform, &map_layout, &placed,
    ) else {
        return;
    };

    let def = registry.towers.get(selected.0)
        .expect("Selected tower index must be in registry");

    // Check affordability — if the player can't pay, flash the preview red.
    if gold.0 < def.cost as f32 {
        for preview_entity in preview_q.iter() {
            commands.entity(preview_entity).insert(PlacementDenied(
                Timer::from_seconds(DENIED_FLASH_DURATION, TimerMode::Once),
            ));
        }
        return;
    }

    let pos = tile_to_world(tile, map_layout.width as f32, map_layout.height as f32);

    // Emit the message — spawning and gold deduction are handled elsewhere.
    place_events.write(PlaceTower {
        tile,
        world_pos: pos,
        tower_id: selected.0,
        cost: def.cost,
    });
}
```

Notice what is gone: `mut gold: ResMut<Gold>`, `mut placed: ResMut<PlacedTowers>`, `atlas: Res<TowerAtlas>`, and the spawn logic. The function is now focused entirely on "should we place a tower?" and "tell the world we did."

Register `place_tower_on_click` in `Update` under `in_state(GameState::InGame)`.

> **Run the game now.** You can still click, but nothing spawns and gold doesn't change — the producer emits messages into the void because no consumer is registered yet.

### The spawn consumer

`spawn_tower_from_event` reads `MessageReader<PlaceTower>` and spawns the appropriate tower for each message. It lives in `src/tower.rs` because it owns tower spawning logic.

What does it query?
- `MessageReader<PlaceTower>` — the messages to react to.
- `Commands` — to spawn entities.
- `Res<TowerAtlas>` and `Res<TowerRegistry>` — to look up sprites and definitions.

```rust
pub fn spawn_tower_from_event(
    mut events: MessageReader<PlaceTower>,
    mut commands: Commands,
    atlas: Res<TowerAtlas>,
    registry: Res<TowerRegistry>,
) {
    let mut iter = events.read();
    let Some(event) = iter.next() else { return; };
    assert!(
        iter.next().is_none(),
        "only one tower can be placed per frame; the producer should emit at most one PlaceTower message",
    );

    let def = registry.towers.get(event.tower_id)
        .expect("Tower type must exist in registry");

    if def.damage.is_some() {
        spawn_instant_tower(&mut commands, &atlas, def, event.tower_id, event.world_pos);
    } else {
        spawn_rocket_launcher(&mut commands, &atlas, def, event.tower_id, event.world_pos);
    }
}
```

This system knows nothing about gold, input, or the mouse. It only knows: "when someone says place a tower, I spawn it."

Register `spawn_tower_from_event` in `Update` with `.after(place_tower_on_click)`.

> **Run the game now.** Clicking places towers again, but gold never deducts and you can place infinite towers on the same tile — the bookkeeping consumer isn't registered yet.

### The bookkeeping consumer

`deduct_gold_on_placement` lives in `src/economy.rs` because it handles resource changes. It reads the same `PlaceTower` messages and updates `Gold` and `PlacedTowers`.

What does it query?
- `MessageReader<PlaceTower>` — the messages to react to.
- `ResMut<Gold>` — to subtract cost.
- `ResMut<PlacedTowers>` — to mark the tile occupied.

```rust
use crate::tower::{PlaceTower, PlacedTowers};

/// Deducts gold and marks the tile as occupied when a tower placement message
/// is received.
pub fn deduct_gold_on_placement(
    mut events: MessageReader<PlaceTower>,
    mut gold: ResMut<Gold>,
    mut placed: ResMut<PlacedTowers>,
) {
    let mut iter = events.read();
    let Some(event) = iter.next() else { return; };
    assert!(
        iter.next().is_none(),
        "only one tower can be placed per frame; the producer should emit at most one PlaceTower message",
    );

    gold.0 -= event.cost as f32;
    placed.0.insert(event.tile);
}
```

Notice that this system does not need `TowerRegistry` — the cost is carried in the message. This is the decoupling payoff: the economy module never imports tower spawning logic.

Register `deduct_gold_on_placement` in `Update` with `.after(place_tower_on_click)`.

> **Run the game now.** Tower placement works exactly as before — click, spawn, gold deducts, tile is blocked — but the work is now distributed across three independent systems.

### Why this split is safe — and why the consumers assert it

In the old design, `place_tower_on_click` performed **check-and-modify** as an atomic operation: it validated the click, deducted gold, and spawned the tower all in one system. With the refactor, the check stays in the producer while the modifications (spawning and gold deduction) move to separate consumers. A reader might rightly ask: *if gold is not deducted immediately, could the player click twice in rapid succession, causing the producer's second affordability check to use stale gold data?*

In our game, **no**, for two reasons:

1. **The producer can only emit one message per frame.** `place_tower_on_click` evaluates `just_pressed(MouseButton::Left)` once per run, and if it passes, it emits exactly one `PlaceTower` message and returns. There is no code path that evaluates the input a second time or emits a second message. Even if the player somehow clicked a thousand times per second, each click is a separate press-release cycle, and `just_pressed` only fires on the first frame of each press.

2. **The producer and consumers run in the same `Update` schedule.** With `.after(place_tower_on_click)`, the consumers are guaranteed to run in the same frame as the producer. Gold is deducted before the next frame begins.

This means the producer's gold check and the consumer's gold deduction are effectively atomic *across* the frame boundary: by the time the next frame starts, both have completed.

Given that we know there is at most one message, why do the consumers assert it with `iter.next()` + `assert!` instead of a `for` loop? Two reasons:

1. **They document the architectural invariant.** The code explicitly states: "this consumer expects exactly one message per frame." That is clearer than a `for` loop, which silently accepts any number.
2. **They catch bugs if the assumption ever stops being true.** If a future feature — batch placement, a bot, or network multiplayer — allows multiple placement requests per frame, the assertion panics immediately with a clear message instead of silently spawning ten towers or deducting ten times the gold.

If the invariant ever changes, the assertion tells you exactly where the design needs to be rethought.

> **When this would not be safe:** In a multiplayer game, a bot, or any system that can trigger multiple placement requests per frame, the producer's gold check would use stale data for all but the first request. You would need either a single atomic system that validates and deducts together, or a message consumer that rejects requests it cannot afford (emitting a rejection message for feedback).

### Wiring it all together

In `src/main.rs`, three things change:

1. **Register the message type** so Bevy creates the internal message queue:

```rust
.add_message::<PlaceTower>()
```

2. **Import the new systems and the message type.**

3. **Add the consumers with explicit ordering.** Without `.after(place_tower_on_click)`, consumers could run before the producer in a given frame, forcing them to react on the next frame instead of immediately:

```rust
.add_systems(Update, (
    update_placement_preview,
    place_tower_on_click,
    spawn_tower_from_event.after(place_tower_on_click),
    deduct_gold_on_placement.after(place_tower_on_click),
    despawn_timed,
    update_gold_hud,
    tick_placement_denied,
).run_if(in_state(GameState::InGame)))
```

> **Run the game now.** Everything should behave identically to before the refactor: click to place, gold deducts, preview flashes red when unaffordable. The only difference is architectural — the code is now split into focused, independent systems.

---

## Simplifications

- **Affordability check stays in the producer.** A more aggressive refactor would emit a `TowerPlacementRequested` message and let the economy consumer reject it. That adds complexity (how does the player get feedback on rejection?) without teaching more about messages.
- **Single message type.** We could split `PlaceTower` into `PlaceTower` and `TowerPlaced` (the first requests, the second confirms), but that's overkill for a single-player game with no network latency.
- **No rejection message.** When the player cannot afford a tower, the producer handles the feedback directly (red flash) instead of emitting a `PlacementDenied` message for a separate UI system to handle.

---

## Summary

- We introduced **custom messages** with `#[derive(Message)]`, `MessageWriter<T>`, and `MessageReader<T>` — Bevy 0.18's replacement for the older event system.
- We split `place_tower_on_click` into a **producer** (validates input, emits `PlaceTower`) and two **consumers** (`spawn_tower_from_event` and `deduct_gold_on_placement`).
- Each consumer asserts **exactly one message per frame** instead of iterating — documenting the architectural invariant and catching bugs if a future feature ever breaks it.
- We used **`.after()`** to guarantee the producer runs before consumers in the same frame, making the architecture easier to reason about.
- We explained why the **gold-check / gold-deduction split is safe** in our single-player, human-input game, and when it would not be safe in multiplayer or automation contexts.
- Messages are registered with `.add_message::<T>()` and automatically cleared after two frames by `DefaultPlugins`.
