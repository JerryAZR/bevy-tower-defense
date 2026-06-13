use bevy::prelude::*;

use crate::state::{GameState, ScreenUi, AvailableLevels, SelectedLevel};

// ---------------------------------------------------------------------------
// grid layout constants
// ---------------------------------------------------------------------------
const COLS: usize = 3;
const BUTTON_W: f32 = 200.0;
const BUTTON_H: f32 = 60.0;
const GAP: f32 = 20.0;

// ---------------------------------------------------------------------------
// button colors
// ---------------------------------------------------------------------------
const COLOR_DEFAULT: Color = Color::srgba(0.15, 0.15, 0.15, 1.0);
const COLOR_FOCUSED: Color = Color::srgba(0.1, 0.3, 0.5, 1.0);
const COLOR_HOVERED: Color = Color::srgba(0.2, 0.25, 0.35, 1.0);
const COLOR_PRESSED: Color = Color::srgba(0.05, 0.15, 0.3, 1.0);

// ---------------------------------------------------------------------------
// resources & components
// ---------------------------------------------------------------------------

/// Which level button is currently highlighted by keyboard navigation.
/// Index into `AvailableLevels`.
#[derive(Resource, Default)]
pub struct FocusedLevel(pub usize);

/// Marker component attached to each level button, carrying its index.
#[derive(Component)]
pub struct LevelButton(pub usize);

// ---------------------------------------------------------------------------
// systems
// ---------------------------------------------------------------------------

/// Scans the assets directory for level files and populates `AvailableLevels`.
/// Also resets `FocusedLevel` to 0 so each visit to the level select screen
/// starts on the first button.
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
    // Reset navigation focus to the top
    commands.insert_resource(FocusedLevel(0));
}

/// Builds the level select UI: a full-screen flex container holding a
/// grid of `Button` entities, one per available level.
///
/// The grid wraps at `COLS` columns using `FlexWrap::Wrap` with a calculated
/// fixed width so overflowing buttons flow into the next row.
pub fn setup_level_select(mut commands: Commands, levels: Res<AvailableLevels>) {
    let grid_w = COLS as f32 * BUTTON_W + (COLS - 1) as f32 * GAP;

    commands
        .spawn((
            ScreenUi,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::Flex,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|parent| {
            parent
                .spawn(Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    width: Val::Px(grid_w),
                    column_gap: Val::Px(GAP),
                    row_gap: Val::Px(GAP),
                    ..default()
                })
                .with_children(|grid| {
                    for (i, path) in levels.0.iter().enumerate() {
                        let name = path
                            .strip_prefix("assets/levels/")
                            .and_then(|s| s.strip_suffix(".toml"))
                            .unwrap_or(path);

                        grid.spawn((
                            Button,
                            LevelButton(i),
                            Node {
                                width: Val::Px(BUTTON_W),
                                height: Val::Px(BUTTON_H),
                                display: Display::Flex,
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(2.0)),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(COLOR_DEFAULT),
                            BorderColor::all(Color::srgba(0.4, 0.4, 0.4, 1.0)),
                        ))

                        .with_child((
                            Text::new(name.to_string()),
                            TextFont {
                                font_size: 22.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    }
                });
        });
}

/// Keyboard navigation of the level select grid.
///
/// Arrow keys move the focus highlight.  Enter / Space selects the focused
/// level.  Number keys 1–9 jump directly and confirm in one step.
pub fn navigate_level_select(
    keys: Res<ButtonInput<KeyCode>>,
    levels: Res<AvailableLevels>,
    mut focused: ResMut<FocusedLevel>,
    mut selected: ResMut<SelectedLevel>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let len = levels.0.len();
    if len == 0 {
        return;
    }

    let row = focused.0 / COLS;
    let col = focused.0 % COLS;
    let total_rows = (len + COLS - 1) / COLS;

    // --- arrow navigation ---
    if keys.just_pressed(KeyCode::ArrowUp) && row > 0 {
        focused.0 = focused.0.saturating_sub(COLS);
    }
    if keys.just_pressed(KeyCode::ArrowDown) && row + 1 < total_rows {
        focused.0 = (focused.0 + COLS).min(len - 1);
    }
    if keys.just_pressed(KeyCode::ArrowLeft) && col > 0 {
        focused.0 -= 1;
    }
    if keys.just_pressed(KeyCode::ArrowRight) && col + 1 < COLS && focused.0 + 1 < len {
        focused.0 += 1;
    }

    // --- confirm (Enter / Space) ---
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
        selected.0 = levels.0[focused.0].clone();
        next_state.set(GameState::InGame);
        return;
    }

    // --- number key shortcuts (1–9) ---
    for i in 0..len.min(9) {
        #[rustfmt::skip]
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
            focused.0 = i;
            selected.0 = levels.0[i].clone();
            next_state.set(GameState::InGame);
            return;
        }
    }
}

/// Applies visual feedback to level-select buttons.
///
/// Runs every frame while on the level select screen.  The focused button
/// gets a highlight colour; hovered / pressed buttons get lighter / darker
/// tints unless they are the focused one (focus takes priority).
pub fn update_level_select_visuals(
    focused: Res<FocusedLevel>,
    mut buttons: Query<(&LevelButton, &Interaction, &mut BackgroundColor)>,
) {
    for (btn, interaction, mut bg) in buttons.iter_mut() {
        *bg = if btn.0 == focused.0 {
            BackgroundColor(COLOR_FOCUSED)
        } else {
            BackgroundColor(match *interaction {
                Interaction::Hovered => COLOR_HOVERED,
                Interaction::Pressed => COLOR_PRESSED,
                Interaction::None => COLOR_DEFAULT,
            })
        };
    }
}

/// Handles mouse clicks on level-select buttons.
///
/// Moves focus to the clicked button and immediately selects it (confirm +
/// navigate in one step for mouse users).
pub fn handle_level_button_click(
    levels: Res<AvailableLevels>,
    mut focused: ResMut<FocusedLevel>,
    mut selected: ResMut<SelectedLevel>,
    mut next_state: ResMut<NextState<GameState>>,
    buttons: Query<(&LevelButton, &Interaction), Changed<Interaction>>,
) {
    for (btn, interaction) in buttons.iter() {
        if *interaction == Interaction::Pressed {
            focused.0 = btn.0;
            selected.0 = levels.0[btn.0].clone();
            next_state.set(GameState::InGame);
            return;
        }
    }
}
