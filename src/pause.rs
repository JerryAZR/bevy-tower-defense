use bevy::prelude::*;
use crate::state::{GameState, PauseState, GameFinished};

// ---------------------------------------------------------------------------
// marker component for the pause overlay entities
// ---------------------------------------------------------------------------

#[derive(Component)]
struct PauseOverlay;

// ---------------------------------------------------------------------------
// toggle — listen for Escape during InGame
// ---------------------------------------------------------------------------

fn toggle_pause(
    game_state: Res<State<GameState>>,
    pause_state: Res<State<PauseState>>,
    game_finished: Option<Res<GameFinished>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_pause: ResMut<NextState<PauseState>>,
) {
    if !keyboard.just_pressed(KeyCode::Escape) {
        return;
    }
    // Only toggle when the player is actually in a level
    if *game_state.get() != GameState::InGame {
        return;
    }
    // Don't pause if the game ended this frame — check_game_state may have
    // already inserted GameFinished, but the state transition to GameOver
    // won't apply until the end of the frame. Without this guard, the pause
    // overlay could spawn on top of the game-over screen.
    if game_finished.is_some() {
        return;
    }
    match pause_state.get() {
        PauseState::Running => next_pause.set(PauseState::Paused),
        PauseState::Paused => next_pause.set(PauseState::Running),
    }
}

// ---------------------------------------------------------------------------
// overlay appearance / disappearance
// ---------------------------------------------------------------------------

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
        TextFont {
            font_size: 64.0,
            ..default()
        },
        TextColor(Color::WHITE),
    ));
}

fn despawn_pause_overlay(mut commands: Commands, overlay: Query<Entity, With<PauseOverlay>>) {
    for entity in &overlay {
        commands.entity(entity).despawn();
    }
}

// ---------------------------------------------------------------------------
// plugin
// ---------------------------------------------------------------------------

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
