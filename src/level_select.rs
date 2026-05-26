use bevy::prelude::*;

use crate::state::{GameState, ScreenUi, AvailableLevels, SelectedLevel};

pub fn scan_available_levels(
    mut commands: Commands,
    mut levels: ResMut<AvailableLevels>,
) {
    levels.0.clear();
    for i in 1..=9 {
        let path = format!("assets/levels/level_{:02}.toml", i);
        if std::path::Path::new(&path).exists() {
            levels.0.push(path);
        }
    }
    // Default to first available level
    if let Some(first) = levels.0.first() {
        commands.insert_resource(SelectedLevel(first.clone()));
    }
}

pub fn setup_level_select(mut commands: Commands, levels: Res<AvailableLevels>) {
    let mut text = String::new();
    for (i, path) in levels.0.iter().enumerate() {
        if !text.is_empty() {
            text.push('\n');
        }
        let name = path
            .strip_prefix("assets/levels/")
            .and_then(|s| s.strip_suffix(".toml"))
            .unwrap_or(path);
        text.push_str(&format!("[{}] {}", i + 1, name));
    }

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
                Text::new(text),
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
    levels: Res<AvailableLevels>,
    mut selected: ResMut<SelectedLevel>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for i in 0..levels.0.len().min(9) {
        let key = match i {
            0 => KeyCode::Digit1,
            1 => KeyCode::Digit2,
            2 => KeyCode::Digit3,
            3 => KeyCode::Digit4,
            4 => KeyCode::Digit5,
            5 => KeyCode::Digit6,
            6 => KeyCode::Digit7,
            7 => KeyCode::Digit8,
            8 => KeyCode::Digit9,
            _ => continue,
        };
        if keys.just_pressed(key) {
            selected.0 = levels.0[i].clone();
            next_state.set(GameState::InGame);
            return;
        }
    }
}
