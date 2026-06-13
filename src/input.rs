use bevy::prelude::*;
use bevy::ecs::message::MessageWriter;
use bevy::input::mouse::MouseWheel;
use bevy::ecs::message::MessageReader;
use crate::state::GameState;

// ---------------------------------------------------------------------------
// action event — logical input, decoupled from physical devices
// ---------------------------------------------------------------------------

/// A logical input action.  Keyboard, mouse, and (later) gamepad all emit
/// these same variants, so game-logic systems never touch raw device types.
#[derive(Message, Debug, Clone)]
pub enum GameAction {
    /// 4-directional navigation (level select grid, placement preview).
    Up,
    Down,
    Left,
    Right,
    /// Select a specific tower by index (number keys 1–5).
    SelectTower(usize),
    /// Select a specific level by index (number keys 1–9).
    SelectLevel(usize),
    /// Move tower dock selection to the next tower.
    NextTower,
    /// Move tower dock selection to the previous tower.
    PrevTower,
    /// Generic confirm (Enter, Space, gamepad A).
    Confirm,
    /// Generic cancel / back (Escape, gamepad B, gamepad Start).
    Cancel,
}

// ---------------------------------------------------------------------------
// digit key table — shared between the keyboard readers below
// ---------------------------------------------------------------------------

const DIGIT_KEYS: [KeyCode; 9] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];

// ---------------------------------------------------------------------------
// keyboard → GameAction
// ---------------------------------------------------------------------------

/// Reads keyboard input and emits `GameAction` events.
/// The mapping is context-dependent: digit keys mean different things
/// depending on which `GameState` is active.
fn read_keyboard_for_actions(
    keys: Res<ButtonInput<KeyCode>>,
    game_state: Res<State<GameState>>,
    mut actions: MessageWriter<GameAction>,
) {
    // 4-directional — works in any state that consumes them
    if keys.just_pressed(KeyCode::ArrowUp)    { actions.write(GameAction::Up); }
    if keys.just_pressed(KeyCode::ArrowDown)  { actions.write(GameAction::Down); }
    if keys.just_pressed(KeyCode::ArrowLeft)  { actions.write(GameAction::Left); }
    if keys.just_pressed(KeyCode::ArrowRight) { actions.write(GameAction::Right); }

    // Generic confirm / cancel
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
        actions.write(GameAction::Confirm);
    }
    if keys.just_pressed(KeyCode::Escape) {
        actions.write(GameAction::Cancel);
    }

    // Context-dependent digit keys
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

// ---------------------------------------------------------------------------
// mouse → GameAction
// ---------------------------------------------------------------------------

/// Reads mouse wheel and emits tower dock navigation actions.
/// Only the first scroll event each frame is processed — one step per tick.
fn read_mouse_for_actions(
    mut scroll: MessageReader<MouseWheel>,
    game_state: Res<State<GameState>>,
    mut actions: MessageWriter<GameAction>,
) {
    if *game_state.get() != GameState::InGame {
        return;
    }
    let Some(ev) = scroll.read().next() else {
        return;
    };
    if ev.y < 0.0 {
        actions.write(GameAction::NextTower);
    } else if ev.y > 0.0 {
        actions.write(GameAction::PrevTower);
    }
}

// ---------------------------------------------------------------------------
// plugin
// ---------------------------------------------------------------------------

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_message::<GameAction>()
            .add_systems(Update, (read_keyboard_for_actions, read_mouse_for_actions));
    }
}
