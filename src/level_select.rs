use bevy::prelude::*;

use crate::{GameState, ScreenUi};

pub fn setup_level_select(mut commands: Commands) {
    commands
        .spawn((
            ScreenUi,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Press [1] — Level 1"),
                TextFont {
                    font_size: 40.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

pub fn handle_level_select_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if keys.just_pressed(KeyCode::Digit1) {
        next_state.set(GameState::InGame);
    }
}
