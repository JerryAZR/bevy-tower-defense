mod level;
mod map;
mod tiling;
mod enemy;
mod tower;
mod level_select;
mod game_over;

use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;

use level::{LevelData, build_map_from_level, load_level};
use map::{MapLayout, MapTile, PathTile, TileType};
use tiling::{TileRules, build_rules};
use enemy::{BaseLives, GameFinished, SpawnSchedule, build_spawn_schedule, spawn_wave_enemies, move_enemies, process_base_reachers, check_game_state};
use tower::{PlacedTowers, setup_tower_atlas, spawn_placement_preview, update_placement_preview, place_tower_on_click, attack_enemies, despawn_timed};
use level_select::{setup_level_select, handle_level_select_input};
use game_over::{setup_game_over, handle_game_over_input};

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
struct ScreenUi;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .add_plugins(TilemapPlugin)
        .init_state::<GameState>()
        .add_systems(Startup, spawn_camera)
        // ---------- LevelSelect ----------
        .add_systems(OnEnter(GameState::LevelSelect), (
            setup_level_select,
            setup_tower_atlas,
        ).chain())
        .add_systems(OnExit(GameState::LevelSelect), cleanup_screen_ui)
        .add_systems(Update, handle_level_select_input
            .run_if(in_state(GameState::LevelSelect)))
        // ---------- InGame ----------
        .add_systems(OnEnter(GameState::InGame), (
            load_level_data,
            setup_spawn_schedule,
            spawn_tilemap,
            spawn_placement_preview,
        ).chain())
        .add_systems(OnExit(GameState::InGame), cleanup_level)
        .add_systems(FixedUpdate, (
            spawn_wave_enemies,
            move_enemies,
            attack_enemies,
            process_base_reachers,
            check_game_state,
        ).chain().run_if(in_state(GameState::InGame)))
        .add_systems(Update, (
            update_placement_preview,
            place_tower_on_click,
            despawn_timed,
        ).run_if(in_state(GameState::InGame)))
        // ---------- GameOver ----------
        .add_systems(OnEnter(GameState::GameOver), setup_game_over)
        .add_systems(OnExit(GameState::GameOver), cleanup_screen_ui)
        .add_systems(Update, handle_game_over_input
            .run_if(in_state(GameState::GameOver)))
        .run();
}

fn load_level_data(mut commands: Commands) {
    let level = load_level("assets/levels/level_01.toml");
    let map = build_map_from_level(&level);
    let rules = build_rules();

    commands.insert_resource(map);
    commands.insert_resource(rules);
    commands.insert_resource(level);
    commands.insert_resource(PlacedTowers::default());
    commands.insert_resource(BaseLives(5));
}

fn spawn_tilemap(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    map: Res<MapLayout>,
    rules: Res<TileRules>,
) {

    let texture_handle: Handle<Image> =
        asset_server.load("Tilesheet/towerDefense_tilesheet.png");

    let map_size = TilemapSize {
        x: map.width,
        y: map.height,
    };
    let tile_size = TilemapTileSize { x: 64.0, y: 64.0 };
    let grid_size = tile_size.into();
    let map_type = TilemapType::Square;

    let tilemap_entity = commands.spawn(GameEntity).id();
    let mut tile_storage = TileStorage::empty(map_size);

    for x in 0..map.width {
        for y in 0..map.height {
            let pos = TilePos { x, y };
            let tile_type = map.get(x, y).unwrap();
            let visual_index = rules.resolve(tile_type, pos, &map);

            let tile_entity = commands
                .spawn((
                    TileBundle {
                        position: pos,
                        tilemap_id: TilemapId(tilemap_entity),
                        texture_index: TileTextureIndex(visual_index),
                        ..Default::default()
                    },
                    tile_type,
                    MapTile,
                    GameEntity,
                ))
                .id();

            if tile_type == TileType::Path {
                commands.entity(tile_entity).insert(PathTile);
            }

            tile_storage.set(&pos, tile_entity);
        }
    }

    commands.entity(tilemap_entity).insert(TilemapBundle {
        grid_size,
        map_type,
        size: map_size,
        storage: tile_storage,
        texture: TilemapTexture::Single(texture_handle),
        tile_size,
        anchor: TilemapAnchor::Center,
        ..Default::default()
    });
}

fn setup_spawn_schedule(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    level: Res<LevelData>,
) {
    let schedule = build_spawn_schedule(&level, &asset_server, &mut texture_atlas_layouts);
    commands.insert_resource(schedule);
}

fn cleanup_level(
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
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn cleanup_screen_ui(mut commands: Commands, query: Query<Entity, With<ScreenUi>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}
