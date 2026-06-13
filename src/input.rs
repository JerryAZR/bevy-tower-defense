use bevy::prelude::*;
use bevy::ecs::message::MessageWriter;
use bevy::input::mouse::MouseWheel;
use bevy::window::CursorMoved;
use crate::state::GameState;
use crate::map::MapLayout;
use crate::tower::{VirtualCursorPos, world_to_tile};
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
fn read_mouse_wheel(
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
// mouse cursor → VirtualCursorPos ("virtual cursor")
// ---------------------------------------------------------------------------

/// Updates `VirtualCursorPos` from the mouse cursor position every frame
/// during `InGame`.  Gameplay systems read this resource instead of
/// querying the cursor directly.
fn read_mouse_hover(
    camera: Single<(&Camera, &GlobalTransform)>,
    map_layout: Res<MapLayout>,
    mut kb_pos: ResMut<VirtualCursorPos>,
    mut cursor_events: MessageReader<CursorMoved>,
) {
    // Only process the last cursor-move event of the frame.
    let Some(event) = cursor_events.read().last() else { return; };

    let (cam, cam_transform) = *camera;
    let Ok(world) = cam.viewport_to_world_2d(cam_transform, event.position) else {
        return;
    };
    if let Some(tile) = world_to_tile(world, map_layout.width, map_layout.height) {
        kb_pos.0 = Some(tile);
    }
}

// ---------------------------------------------------------------------------
// mouse click → GameAction::Confirm
// ---------------------------------------------------------------------------

/// Left-click during `InGame` emits a `Confirm` action, but only when the
/// cursor is on a map tile.
fn read_mouse_click(
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform)>,
    map_layout: Res<MapLayout>,
    mut actions: MessageWriter<GameAction>,
) {
    if !mouse.just_pressed(MouseButton::Left) { return; }

    let (cam, cam_transform) = *camera;
    let Some(cursor) = window.cursor_position() else { return; };
    let Ok(world) = cam.viewport_to_world_2d(cam_transform, cursor) else {
        return;
    };
    if world_to_tile(world, map_layout.width, map_layout.height).is_some() {
        actions.write(GameAction::Confirm);
    }
}

// ---------------------------------------------------------------------------
// plugin

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_message::<GameAction>()
            .add_systems(Update, (read_keyboard_for_actions, read_mouse_wheel))
            .add_systems(Update, read_mouse_hover.run_if(in_state(GameState::InGame)))
            .add_systems(Update, read_mouse_click.run_if(in_state(GameState::InGame)));
    }
}
