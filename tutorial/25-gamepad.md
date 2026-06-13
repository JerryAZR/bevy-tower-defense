# Part 25: Gamepad

> **Time to read:** ~12 minutes
> **New concepts:** `GilrsPlugin`, `Gamepad` query, analog stick pacing with `Timer`, dead zones
> **Prerequisite:** Part 24 (input abstraction)

---

## Recap: What We Already Have

`GameAction` events and `VirtualCursorPos` already abstract all game logic away from physical input. Keyboard, mouse, and mouse wheel write to these shared channels. Adding a new device means writing a small handful of reader systems — nothing else changes.

---

## Goal: What We Will Build

Two new systems in `src/input.rs`: `read_gamepad` for digital buttons (D-pad, face buttons, triggers) and `read_gamepad_stick` for analog left-stick movement with a timed repeat rate. Combined they cover the gamepad's full input surface without touching any game-logic file.

---

## New Bevy APIs & Concepts

### `bevy_gilrs` feature

Bevy's gamepad support comes from the [`gilrs`](https://docs.rs/gilrs/latest/gilrs/) crate, gated behind the `bevy_gilrs` Cargo feature. `DefaultPlugins` automatically registers the `GilrsPlugin` when the feature is enabled, which spawns a `Gamepad` component for each connected device. Add to `Cargo.toml`:

```toml
bevy = { version = "0.18.1", features = ["bevy_gilrs"] }
```

### `Gamepad` component

Each connected gamepad gets an entity with a `Gamepad` component. Query all of them:

```rust
fn read_gamepad(gamepads: Query<&Gamepad>) {
    for gamepad in gamepads.iter() { /* ... */ }
}
```

`Gamepad::digital()` returns the `ButtonInput<GamepadButton>` for that gamepad — same pattern as `ButtonInput<KeyCode>` for keyboard.

### `GamepadButton` variants

Bevy uses cardinal-position names rather than platform-specific labels: `South`, `East`, `North`, `West` replace "A/B/X/Y" or "Cross/Circle/Square/Triangle." `DPadUp`, `DPadDown`, `DPadLeft`, `DPadRight` for the D-pad.

### Analog stick vs digital buttons

`gamepad.digital()` only reports button presses (D-pad, face buttons, triggers-as-buttons). The analog left stick requires `gamepad.left_stick()`, which returns a `Vec2` in the range [-1, 1] for each axis. Because the stick reports a continuous value every frame, we can't use `just_pressed` — we need a **throttled timer** to pace movement.

Pacing analog input this way is a common pattern: a dead zone discards tiny values (stick wiggle), a timer limits the repeat rate, and the dominant axis is picked to avoid diagonal movement.

---

## Walkthrough

### Step 1: Add the `bevy_gilrs` feature

In `Cargo.toml`:

```toml
bevy = { version = "0.18.1", features = ["bevy_gilrs"] }
```

No plugin registration needed — `DefaultPlugins` already includes `GilrsPlugin`.

### Step 2: Add `read_gamepad` to `src/input.rs`

The system queries all `Gamepad` components and maps buttons to `GameAction` events:

```rust
fn read_gamepad(
    gamepads: Query<&Gamepad>,
    mut actions: MessageWriter<GameAction>,
) {
    for gamepad in gamepads.iter() {
        let btn = gamepad.digital();
        if btn.just_pressed(GamepadButton::DPadUp)    { actions.write(GameAction::Up); }
        if btn.just_pressed(GamepadButton::DPadDown)  { actions.write(GameAction::Down); }
        if btn.just_pressed(GamepadButton::DPadLeft)  { actions.write(GameAction::Left); }
        if btn.just_pressed(GamepadButton::DPadRight) { actions.write(GameAction::Right); }
        if btn.just_pressed(GamepadButton::South) {
            actions.write(GameAction::Confirm);
        }
        if btn.just_pressed(GamepadButton::East) {
            actions.write(GameAction::Cancel);
        }
        if btn.just_pressed(GamepadButton::Start) {
            actions.write(GameAction::Cancel);
        }
        if btn.just_pressed(GamepadButton::LeftTrigger) {
            actions.write(GameAction::PrevTower);
        }
        if btn.just_pressed(GamepadButton::RightTrigger) {
            actions.write(GameAction::NextTower);
        }
    }
}
```

Notice:

- **No context-dependent mapping** — D-pad is always navigation, `South` is always confirm. Unlike the keyboard reader (which needs `GameState` to decide what digits mean), the gamepad's limited buttons make mappings unambiguous.
- **Both `East` and `Start` emit `Cancel`** — matching Escape on keyboard, which both closes the game-over screen and toggles pause.
- **`LeftTrigger` / `RightTrigger` cycle the tower dock** — shoulder buttons (LB/RB) change the selected tower, same as mouse scroll.

### Step 3: Add `read_gamepad_stick` — analog stick movement

The stick reports continuous values every frame, so we pace movement with a
`Timer` instead of `just_pressed` checks. A repeating `Timer` fires every
150 ms; holding the stick in one direction emits a directional action each
time it expires.

```rust
fn read_gamepad_stick(
    gamepads: Query<&Gamepad>,
    mut actions: MessageWriter<GameAction>,
    mut timer: Local<Timer>,
    mut was_idle: Local<bool>,
    time: Res<Time>,
) {
    // Initialise the repeating timer once (150 ms per tile step).
    if timer.duration().is_zero() {
        *timer = Timer::from_seconds(0.15, TimerMode::Repeating);
        *was_idle = true;
    }

    for gamepad in gamepads.iter() {
        let stick = gamepad.left_stick();
        if stick.length() < 0.3 {
            // Stick released — next push will fire immediately.
            *was_idle = true;
            return;
        }

        timer.tick(time.delta());
        if !*was_idle && !timer.just_finished() {
            return;
        }

        *was_idle = false;
        timer.reset();

        let action = if stick.x.abs() > stick.y.abs() {
            if stick.x > 0.0 { GameAction::Right } else { GameAction::Left }
        } else {
            if stick.y > 0.0 { GameAction::Up } else { GameAction::Down }
        };
        actions.write(action);
    }
}
```

Notice:

- **Dead zone (0.3)** — stick values below this threshold set `was_idle`, so the first push after releasing the stick fires immediately.
- **`was_idle` flag** — tracks whether the stick was in the dead zone last frame. When transitioning from idle → active, the action fires on the first frame without waiting for the timer. `timer.reset()` afterwards ensures the next auto-fire is a full 150 ms away, not rushed by accumulated time.
- **Dominant axis only** — if the stick is pushed diagonally, only the stronger axis emits. This matches our 4-direction `GameAction` model.

### Step 4: Register both systems

```rust
impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_message::<GameAction>()
            .add_systems(Update, (read_keyboard_for_actions, read_mouse_wheel))
            .add_systems(Update, (read_mouse_hover, read_mouse_click)
                .run_if(in_state(GameState::InGame)))
            .add_systems(Update, (read_gamepad, read_gamepad_stick));
    }
}
```

Both systems run unconditionally — `GameAction` events are only consumed by systems that are active in the current state. D-pad and stick presses in `LevelSelect` work because `navigate_level_select` is running; in `InGame` they work because `nudge_virtual_cursor` is running.

> **Run the game now** with a gamepad plugged in. D-pad or left stick navigates the level select grid. Start a level: D-pad or left stick moves the placement preview. `LeftTrigger`/`RightTrigger` (LB/RB) cycle the tower dock. South (A) places a tower. East (B) pauses. After winning or losing, South or East returns to level select. Everything works — and we didn't touch a single game-logic system.

---

## Simplifications

- **All gamepads are treated identically.** No "player 1" assignment; pressing buttons on any connected controller triggers actions. Fine for a single-player game.

---

## Summary

- **Two reader systems, zero game-logic changes** — the payoff from Part 24's abstraction. `read_gamepad` for digital buttons, `read_gamepad_stick` for analog movement.
- **`bevy_gilrs` feature** enables `DefaultPlugins` to spawn `Gamepad` components.
- **`Query<&Gamepad>`** gives access to all connected gamepads; `gamepad.digital()` for buttons, `gamepad.left_stick()` for analog axes.
- **Analog stick movement** uses a `Timer` for repeat pacing, a dead zone for drift rejection, and dominant-axis picking for 4-direction movement.
- **`GamepadButton` uses cardinal names** (`South`, `East`) to avoid platform-specific labeling.
