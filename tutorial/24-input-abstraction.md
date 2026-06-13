# Part 24: Input Abstraction — `GameAction` Events

> **Time to read:** ~14 minutes  
> **New concepts:** logical input events, `MessageWriter` / `MessageReader` decoupling, context-dependent key mapping  
> **Prerequisite:** Part 23 (level select buttons)

---

## Recap: What We Already Have

Five systems read raw device types directly — `Res<ButtonInput<KeyCode>>` in `navigate_level_select`, `select_tower_by_key`, `toggle_pause`, and `handle_game_over_input`, plus `MessageReader<MouseWheel>` in `cycle_tower_on_scroll`. Adding a gamepad (Part 25) would mean touching every one of these systems again. The input layer is coupled to specific hardware.

---

## Goal: What We Will Build

Introduce a `GameAction` message — a logical vocabulary for player intent. Keyboard and mouse become *producers* of `GameAction` events. All game-logic systems become *consumers* of `GameAction` events. No game-logic file ever sees a `KeyCode` or `MouseWheel` again.

The game behaves identically. Scroll still cycles the tower dock. Arrow keys still navigate the level grid. The difference is that all physical-input code lives in one file (`src/input.rs`), and the rest of the codebase only knows about logical actions.

---

## New Bevy APIs & Concepts

### Logical input vs physical input

Physical input says *which device* and *which key*:
> "The user pressed KeyCode::Digit3 while in GameState::LevelSelect."

Logical input says *what the player intended to do*:
> "The player wants to select level index 2."

The translation from physical to logical happens once, in one place. Every system downstream only sees the logical layer.

### `MessageWriter<M>` and `MessageReader<M>`

We've used these before (for `PlaceTower` and `PlaySound`), but now they become the backbone of the input system:

- `MessageWriter<GameAction>` — the producer's handle; call `.write(action)` to enqueue an action.
- `MessageReader<GameAction>` — the consumer's handle; call `.read()` to drain actions this frame.

Because events are frame-scoped, actions fire exactly once per press — no "held button" spam.

---

## Walkthrough

### Step 1: Define `GameAction` and `InputPlugin`

Create `src/input.rs`. The `GameAction` enum captures every thing a player can *intend*:

```rust
#[derive(Message, Debug, Clone)]
pub enum GameAction {
    Up, Down, Left, Right,          // 4-directional navigation
    SelectTower(usize),             // number keys 1–5
    SelectLevel(usize),             // number keys 1–9
    NextTower, PrevTower,           // scroll wheel
    Confirm,                        // Enter / Space
    Cancel,                         // Escape
}
```

Notice what's *not* here: no `KeyCode`, no `MouseWheel`, no `GamepadButton`. This enum is device-agnostic by design. When Part 25 adds gamepad, the gamepad reader emits these same variants — no new variants needed.

The `InputPlugin` bundles the message registration and the two reader systems:

```rust
pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_message::<GameAction>()
            .add_systems(Update, (read_keyboard_for_actions, read_mouse_for_actions));
    }
}
```

### Step 2: Keyboard → GameAction

`read_keyboard_for_actions` reads `ButtonInput<KeyCode>` and writes `GameAction` events. The mapping is context-dependent because digit keys mean different things in different screens:

```rust
fn read_keyboard_for_actions(
    keys: Res<ButtonInput<KeyCode>>,
    game_state: Res<State<GameState>>,
    mut actions: MessageWriter<GameAction>,
) {
    // 4-directional — works in any state
    if keys.just_pressed(KeyCode::ArrowUp)  { actions.write(GameAction::Up); }
    // ... Down, Left, Right ...

    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
        actions.write(GameAction::Confirm);
    }
    if keys.just_pressed(KeyCode::Escape) {
        actions.write(GameAction::Cancel);
    }

    // Digits: context-dependent
    match *game_state.get() {
        GameState::InGame => {
            for i in 0..5 {
                if keys.just_pressed(DIGIT_KEYS[i]) {
                    actions.write(GameAction::SelectTower(i));
                }
            }
        }
        GameState::LevelSelect => {
            for i in 0..9 {
                if keys.just_pressed(DIGIT_KEYS[i]) {
                    actions.write(GameAction::SelectLevel(i));
                }
            }
        }
        _ => {}
    }
}
```

A shared `DIGIT_KEYS` array avoids repeating the `KeyCode::Digit1`..`KeyCode::Digit9` boilerplate. The `match` on `GameState` ensures that pressing "3" in a level selects tower index 2, while pressing "3" on the level select screen selects level index 2.

### Step 3: Mouse → GameAction

`read_mouse_for_actions` replaces `cycle_tower_on_scroll`. It reads `MessageReader<MouseWheel>`, checks that we're in a level, and writes `NextTower` / `PrevTower`:

```rust
fn read_mouse_for_actions(
    mut scroll: MessageReader<MouseWheel>,
    game_state: Res<State<GameState>>,
    mut actions: MessageWriter<GameAction>,
) {
    if *game_state.get() != GameState::InGame { return; }
    let Some(ev) = scroll.read().next() else { return; };
    if ev.y < 0.0 { actions.write(GameAction::NextTower); }
    else if ev.y > 0.0 { actions.write(GameAction::PrevTower); }
}
```

The `cycle_tower_on_scroll` function is deleted — its logic moved here.

### Step 4: Refactor consumers

Each game-logic system replaces its device-specific parameter with `MessageReader<GameAction>`.

**`select_tower_by_key` (`src/tower.rs`)** — absorbs scroll. Handles `SelectTower`, `NextTower`, `PrevTower`:

```rust
pub fn select_tower_by_key(
    mut actions: MessageReader<GameAction>,
    registry: Res<TowerRegistry>,
    mut selected: ResMut<SelectedTowerType>,
) {
    let len = registry.towers.len();
    for action in actions.read() {
        match action {
            GameAction::SelectTower(i) if *i < len => selected.0 = *i,
            GameAction::NextTower if selected.0 + 1 < len => selected.0 += 1,
            GameAction::PrevTower if selected.0 > 0 => selected.0 -= 1,
            _ => {}
        }
    }
}
```

> `action` borrows the event data, so pattern-matched fields like `i` in `SelectTower(i)` are `&usize`. Use `*i` to compare and assign.

**`navigate_level_select` (`src/level_select.rs`)** — one action per frame to keep navigation math clean:

```rust
pub fn navigate_level_select(
    mut actions: MessageReader<GameAction>,
    // ...
) {
    let Some(action) = actions.read().next() else { return; };
    match action {
        GameAction::Up if row > 0 => focused.0 = focused.0.saturating_sub(COLS),
        GameAction::Down if row + 1 < total_rows => focused.0 = (focused.0 + COLS).min(len - 1),
        // ... Left, Right, SelectLevel, Confirm ...
    }
}
```

**`toggle_pause` (`src/pause.rs`)** — checks for any `Cancel` in the event stream:

```rust
fn toggle_pause(
    mut actions: MessageReader<GameAction>,
    // ...
) {
    if !actions.read().any(|a| matches!(a, GameAction::Cancel)) { return; }
    // ... guard, toggle ...
}
```

**`handle_game_over_input` (`src/game_over.rs`)** — `Confirm` or `Cancel` returns to level select. The prompt text updates from "Press Space to continue" to "Press Space / Enter / Escape to continue".

### Step 5: Wire in `main.rs`

Add `mod input;`, import `InputPlugin`, and register it alongside the other plugins. Remove `cycle_tower_on_scroll` from the tower import and from the `TowerDock` system set — it no longer exists.

> **Run the game now.** Arrow keys navigate the level grid. Scroll cycles the tower dock. Escape pauses. Space dismisses the game-over screen. Digit keys 1–5 select towers, 1–9 select levels. Everything works as before — but no game-logic system touches `ButtonInput` or `MouseWheel`.

---

## Simplifications

- **No gamepad yet.** `GameAction` has the right variants for gamepad (D-pad → `Up/Down/Left/Right`, A → `Confirm`, B → `Cancel`), but the gamepad reader system doesn't exist yet. That's Part 25.
- **Mouse placement isn't abstracted.** `place_tower_on_click` still reads `ButtonInput<MouseButton>` and `Window`/`Camera` for coordinate conversion. Those are spatial queries that don't fit a `GameAction` event neatly. Part 25 will add placement via gamepad (preview nudge + A to place), keeping both mouse and gamepad paths working side by side.
- **`read_keyboard_for_actions` checks `GameState` internally.** An alternative is to split it into three systems with `run_if(in_state(...))` conditions. The inline match is shorter for a tutorial but couples the reader to the state type.

---

## Summary

- **`GameAction` is a device-agnostic event enum** — keyboard, mouse, and gamepad all write the same variants.
- **`MessageWriter<GameAction>`** is the producer handle; **`MessageReader<GameAction>`** is the consumer handle.
- **Context-dependent mapping** (digits mean different things in different screens) is handled once in the reader, not scattered across consumers.
- **Adding a new input device** (Part 25) requires adding one reader system; no game-logic files change.
