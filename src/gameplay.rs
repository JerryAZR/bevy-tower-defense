use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;

use crate::state::GameEntity;
use crate::level::{LevelData, build_map_from_level, load_level};
use crate::map::{MapLayout, MapTile, PathTile, TileType};
use crate::tiling::{TileRules, build_rules};
use crate::enemy::build_spawn_schedule;
use crate::state::BaseLives;
use crate::tower::PlacedTowers;

pub fn load_level_data(mut commands: Commands) {
    let level = load_level("assets/levels/level_01.toml");
    let map = build_map_from_level(&level);
    let rules = build_rules();

    commands.insert_resource(map);
    commands.insert_resource(rules);
    commands.insert_resource(level);
    commands.insert_resource(PlacedTowers::default());
    commands.insert_resource(BaseLives(5));
}

pub fn spawn_tilemap(
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

pub fn setup_spawn_schedule(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    level: Res<LevelData>,
) {
    let schedule = build_spawn_schedule(&level, &asset_server, &mut texture_atlas_layouts);
    commands.insert_resource(schedule);
}
