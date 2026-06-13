use crate::tower::PlacedTowers;

use bevy::prelude::*;
use bevy::ecs::system::entity_command::despawn;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States, Default)]
pub enum PauseState {
    #[default]
    Running,
    Paused,
}

/// Named system sets for the gameplay phase.
/// `configure_sets` applies the `game_is_running` condition once,
/// instead of repeating it on every `.add_systems` chain.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameplaySet {
    /// FixedUpdate: enemies, towers, economy.
    Simulation,
    /// Update: placement preview, tower placement, HUD, gizmos.
    Interaction,
    /// Update: scroll, number keys, click on the tower dock.
    TowerDock,
}

/// Returns `true` when the player is in a level and the game is not paused.
pub fn game_is_running(
    game_state: Res<State<GameState>>,
    pause_state: Res<State<PauseState>>,
) -> bool {
    *game_state.get() == GameState::InGame && *pause_state.get() == PauseState::Running
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
        // queue_silenced(despawn()) silently ignores double-despawn
        // errors — useful when child entities may have been auto-
        // despawned by a parent earlier in this same iteration.
        commands.entity(entity).queue_silenced(despawn());
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
