use crate::tower::PlacedTowers;

use bevy::prelude::*;

use crate::map::MapLayout;
use crate::tiling::TileRules;
use crate::level::LevelData;
use crate::enemy::SpawnSchedule;
use crate::economy::Gold;

#[derive(Resource)]
pub struct BaseLives(pub i32);

#[derive(Resource)]
pub struct GameFinished;

#[derive(Resource, Default)]
pub struct AvailableLevels(pub Vec<String>);

#[derive(Resource)]
pub struct SelectedLevel(pub String);

#[derive(Component)]
pub struct GameEntity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States, Default)]
pub enum GameState {
    #[default]
    LevelSelect,
    InGame,
    GameOver,
}

#[derive(Resource)]
pub enum GameResult { Victory, Defeat }

#[derive(Component)]
pub struct ScreenUi;

pub fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

pub fn cleanup_level(
    mut commands: Commands,
    entities: Query<Entity, With<GameEntity>>,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<MapLayout>();
    commands.remove_resource::<TileRules>();
    commands.remove_resource::<LevelData>();
    commands.remove_resource::<SpawnSchedule>();
    commands.remove_resource::<PlacedTowers>();
    commands.remove_resource::<BaseLives>();
    commands.remove_resource::<GameFinished>();
    commands.remove_resource::<Gold>();
}

pub fn cleanup_screen_ui(mut commands: Commands, query: Query<Entity, With<ScreenUi>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}
