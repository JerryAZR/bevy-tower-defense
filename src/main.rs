use bevy::prelude::*;

#[derive(Component)]
struct MapTile;

#[derive(Component)]
struct PathTile;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    commands.spawn(Camera2d);

    let texture = asset_server.load("Tilesheet/towerDefense_tilesheet.png");
    let layout = TextureAtlasLayout::from_grid(UVec2::splat(64), 23, 13, None, None);
    let atlas_layout = texture_atlas_layouts.add(layout);

    let cols = 15;
    let rows = 10;
    let tile_size = 64.0;

    let offset_x = -(cols as f32 * tile_size) / 2.0 + tile_size / 2.0;
    let offset_y = -(rows as f32 * tile_size) / 2.0 + tile_size / 2.0;

    let path_mid = rows / 2;

    for row in 0..rows {
        for col in 0..cols {
            let x = offset_x + col as f32 * tile_size;
            let y = offset_y + row as f32 * tile_size;

            let tile_index = match (row, col) {
                // Lower road edge (visually below the road body)
                (r, 0) if r == path_mid - 1 => 125,   // bottom-left corner
                (r, c) if r == path_mid - 1 && c == cols - 1 => 127, // bottom-right corner
                (r, _) if r == path_mid - 1 => 126,    // bottom edge

                // Road body
                (r, 0) if r == path_mid => 102,       // left edge
                (r, c) if r == path_mid && c == cols - 1 => 104, // right edge
                (r, _) if r == path_mid => 103,       // road body

                // Upper road edge (visually above the road body)
                (r, 0) if r == path_mid + 1 => 79,    // upper-left corner
                (r, c) if r == path_mid + 1 && c == cols - 1 => 81, // upper-right corner
                (r, _) if r == path_mid + 1 => 80,    // top edge

                // Everything else is grass
                _ => 129,
            };

            let mut entity = commands.spawn((
                Sprite::from_atlas_image(
                    texture.clone(),
                    TextureAtlas {
                        layout: atlas_layout.clone(),
                        index: tile_index,
                    },
                ),
                Transform::from_xyz(x, y, 0.0),
                MapTile,
            ));

            if tile_index != 129 {
                entity.insert(PathTile);
            }
        }
    }
}
