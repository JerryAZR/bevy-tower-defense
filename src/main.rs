use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;

#[derive(Component)]
struct MapTile;

#[derive(Component)]
struct PathTile;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins(TilemapPlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2d);

    let texture_handle: Handle<Image> = asset_server.load("Tilesheet/towerDefense_tilesheet.png");

    let map_size = TilemapSize { x: 15, y: 10 };
    let tile_size = TilemapTileSize { x: 64.0, y: 64.0 };
    let grid_size = tile_size.into();
    let map_type = TilemapType::Square;

    let tilemap_entity = commands.spawn_empty().id();
    let mut tile_storage = TileStorage::empty(map_size);

    let path_mid = map_size.y / 2;

    for x in 0..map_size.x {
        for y in 0..map_size.y {
            let tile_pos = TilePos { x, y };

            let tile_index = match (y, x) {
                // Lower road edge (visually below the road body)
                (r, 0) if r == path_mid - 1 => 125,
                (r, c) if r == path_mid - 1 && c == map_size.x - 1 => 127,
                (r, _) if r == path_mid - 1 => 126,

                // Road body
                (r, 0) if r == path_mid => 102,
                (r, c) if r == path_mid && c == map_size.x - 1 => 104,
                (r, _) if r == path_mid => 103,

                // Upper road edge (visually above the road body)
                (r, 0) if r == path_mid + 1 => 79,
                (r, c) if r == path_mid + 1 && c == map_size.x - 1 => 81,
                (r, _) if r == path_mid + 1 => 80,

                // Everything else is grass
                _ => 129,
            };

            let tile_entity = commands
                .spawn((
                    TileBundle {
                        position: tile_pos,
                        tilemap_id: TilemapId(tilemap_entity),
                        texture_index: TileTextureIndex(tile_index),
                        ..Default::default()
                    },
                    MapTile,
                ))
                .id();

            if tile_index != 129 {
                commands.entity(tile_entity).insert(PathTile);
            }

            tile_storage.set(&tile_pos, tile_entity);
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
