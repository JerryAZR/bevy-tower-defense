use bevy::prelude::*;
use std::collections::HashSet;

use crate::enemy::tile_to_world;
use crate::map::{MapLayout, TileType};

const TOWER_BASE: usize = 180;
const TOWER_TOP: usize = 203;

#[derive(Component)]
pub struct Tower;

#[derive(Component)]
pub(crate) struct TowerTurret;

#[derive(Component)]
pub(crate) struct TowerPreview;

#[derive(Resource)]
pub struct TowerAtlas {
    texture: Handle<Image>,
    layout: Handle<TextureAtlasLayout>,
}

#[derive(Resource, Default)]
pub struct PlacedTowers(pub HashSet<[u32; 2]>);

pub fn setup_tower_atlas(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let texture = asset_server.load("Tilesheet/towerDefense_tilesheet.png");
    let layout = TextureAtlasLayout::from_grid(UVec2::splat(64), 23, 13, None, None);
    commands.insert_resource(TowerAtlas {
        texture,
        layout: texture_atlas_layouts.add(layout),
    });
}

/// Returns the tile under the cursor if it is an unoccupied grass tile.
fn hovered_placeable_tile(
    window: &Window,
    camera: &Camera,
    camera_transform: &GlobalTransform,
    map_layout: &MapLayout,
    placed: &PlacedTowers,
) -> Option<[u32; 2]> {
    let tile = window
        .cursor_position()
        .and_then(|cursor| camera.viewport_to_world_2d(camera_transform, cursor).ok())
        .and_then(|world| world_to_tile(world, map_layout.width, map_layout.height))?;

    let is_grass = map_layout.get(tile[0], tile[1]) == Some(TileType::Grass);
    if is_grass && !placed.0.contains(&tile) {
        Some(tile)
    } else {
        None
    }
}

pub fn spawn_placement_preview(
    mut commands: Commands,
    atlas: Res<TowerAtlas>,
) {
    let tinted = |index: usize| Sprite {
        color: Color::srgba(1.0, 1.0, 1.0, 0.5),
        ..Sprite::from_atlas_image(
            atlas.texture.clone(),
            TextureAtlas { layout: atlas.layout.clone(), index },
        )
    };

    commands.spawn((
        TowerPreview,
        tinted(TOWER_BASE),
        Transform::from_xyz(0.0, 0.0, 2.0),
        Visibility::Hidden,
    ));
    commands.spawn((
        TowerPreview,
        tinted(TOWER_TOP),
        Transform::from_xyz(0.0, 0.0, 2.1),
        Visibility::Hidden,
    ));
}

pub fn update_placement_preview(
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform)>,
    map_layout: Res<MapLayout>,
    placed: Res<PlacedTowers>,
    mut preview_q: Query<(&mut Transform, &mut Visibility), With<TowerPreview>>,
) {
    let (cam, cam_transform) = *camera;

    let Some(tile) = hovered_placeable_tile(
        &window, &cam, &cam_transform, &map_layout, &placed,
    ) else {
        for (_, mut vis) in preview_q.iter_mut() {
            *vis = Visibility::Hidden;
        }
        return;
    };

    let pos = tile_to_world(tile, map_layout.width as f32, map_layout.height as f32);

    for (mut transform, mut vis) in preview_q.iter_mut() {
        transform.translation.x = pos.x;
        transform.translation.y = pos.y;
        *vis = Visibility::Visible;
    }
}

pub fn place_tower_on_click(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform)>,
    map_layout: Res<MapLayout>,
    mut placed: ResMut<PlacedTowers>,
    atlas: Res<TowerAtlas>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    let (cam, cam_transform) = *camera;

    let Some(tile) = hovered_placeable_tile(
        &window, &cam, &cam_transform, &map_layout, &placed,
    ) else {
        return;
    };

    placed.0.insert(tile);
    let pos = tile_to_world(tile, map_layout.width as f32, map_layout.height as f32);

    // Base (static, z=2.0)
    commands.spawn((
        Tower,
        Sprite::from_atlas_image(
            atlas.texture.clone(),
            TextureAtlas { layout: atlas.layout.clone(), index: TOWER_BASE },
        ),
        Transform::from_xyz(pos.x, pos.y, 2.0),
    ));

    // Turret (rotates in Part 9, z=2.1)
    commands.spawn((
        TowerTurret,
        Sprite::from_atlas_image(
            atlas.texture.clone(),
            TextureAtlas { layout: atlas.layout.clone(), index: TOWER_TOP },
        ),
        Transform::from_xyz(pos.x, pos.y, 2.1),
    ));
}

fn world_to_tile(world: Vec2, map_width: u32, map_height: u32) -> Option<[u32; 2]> {
    let tile_size = 64.0;
    let origin_x = -(map_width as f32) * tile_size / 2.0 + tile_size / 2.0;
    let origin_y = -(map_height as f32) * tile_size / 2.0 + tile_size / 2.0;

    let tx = ((world.x - origin_x) / tile_size).round() as i32;
    let ty = ((world.y - origin_y) / tile_size).round() as i32;

    if tx >= 0 && tx < map_width as i32 && ty >= 0 && ty < map_height as i32 {
        Some([tx as u32, ty as u32])
    } else {
        None
    }
}
