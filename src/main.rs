mod level;
mod map;
mod tiling;
mod enemy;

use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;

use level::{LevelData, build_map_from_level, load_level};
use map::{MapLayout, MapTile, PathTile, TileType};
use tiling::{TileRules, build_rules};
use enemy::{Enemy, PathFollower, MoveSpeed, move_enemies, cleanup_finished_enemies};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .add_plugins(TilemapPlugin)
        .add_systems(Startup, (load_level_data, spawn_tilemap, spawn_test_enemy).chain())
        .add_systems(FixedUpdate, (move_enemies, cleanup_finished_enemies))
        .run();
}

fn load_level_data(mut commands: Commands) {
    let level = load_level("assets/levels/level_01.toml");
    let map = build_map_from_level(&level);
    let rules = build_rules();

    commands.insert_resource(map);
    commands.insert_resource(rules);
    commands.insert_resource(level);
}

fn spawn_tilemap(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    map: Res<MapLayout>,
    rules: Res<TileRules>,
) {
    commands.spawn(Camera2d);

    let texture_handle: Handle<Image> =
        asset_server.load("Tilesheet/towerDefense_tilesheet.png");

    let map_size = TilemapSize {
        x: map.width,
        y: map.height,
    };
    let tile_size = TilemapTileSize { x: 64.0, y: 64.0 };
    let grid_size = tile_size.into();
    let map_type = TilemapType::Square;

    let tilemap_entity = commands.spawn_empty().id();
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

fn spawn_test_enemy(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    level: Res<LevelData>,
) {
    let texture = asset_server.load("Tilesheet/towerDefense_tilesheet.png");
    let layout = TextureAtlasLayout::from_grid(UVec2::splat(64), 23, 13, None, None);
    let atlas_layout = texture_atlas_layouts.add(layout);

    let waypoints = &level.paths["main_road"].waypoints;
    let spawn_tile = waypoints[0];
    let target_tile = waypoints[1];

    let tile_size = 64.0;
    let map_width = level.map.width as f32;
    let map_height = level.map.height as f32;
    let origin_x = -map_width * tile_size / 2.0 + tile_size / 2.0;
    let origin_y = -map_height * tile_size / 2.0 + tile_size / 2.0;

    let x = origin_x + spawn_tile[0] as f32 * tile_size;
    let y = origin_y + spawn_tile[1] as f32 * tile_size;
    let target = Vec2::new(
        origin_x + target_tile[0] as f32 * tile_size,
        origin_y + target_tile[1] as f32 * tile_size,
    );

    commands.spawn((
        Sprite::from_atlas_image(
            texture,
            TextureAtlas {
                layout: atlas_layout,
                index: 245,
            },
        ),
        Transform::from_xyz(x, y, 1.0),
        Enemy,
        PathFollower {
            path_id: "main_road".to_string(),
            waypoint_index: 1,
            target,
        },
        MoveSpeed(192.0),
    ));
}
