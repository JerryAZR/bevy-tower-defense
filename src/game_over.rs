use bevy::prelude::*;

use crate::state::{GameState, GameResult, ScreenUi};
use crate::input::GameAction;
use bevy::ecs::message::MessageReader;

pub fn setup_game_over(
    mut commands: Commands,
    result: Res<GameResult>,
) {
    let message = match *result {
        GameResult::Defeat => "Game Over -- the base was destroyed!",
        GameResult::Victory => "Victory -- all enemies defeated!",
    };

    commands
        .spawn((
            ScreenUi,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(20.0),
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(message),
                TextFont {
                    font_size: 40.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            parent.spawn((
                Text::new("Press Space / Enter / Escape to continue"),
                TextFont {
                    font_size: 24.0,
                    ..default()
                },
                TextColor(Color::srgba(0.7, 0.7, 0.7, 1.0)),
            ));
        });
}

pub fn handle_game_over_input(
    mut actions: MessageReader<GameAction>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for action in actions.read() {
        match action {
            GameAction::Confirm | GameAction::Cancel => {
                next_state.set(GameState::LevelSelect);
                return;
            }
            _ => {}
        }
    }
}
