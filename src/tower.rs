use bevy::prelude::*;
use std::collections::HashSet;

use crate::enemy::tile_to_world;
use crate::map::{MapLayout, TileType};
use crate::enemy::{Enemy, Health, PathFollower};
use crate::state::GameEntity;
use crate::economy::{Gold, Bounty, PlacementDenied, TOWER_COST, DENIED_FLASH_DURATION};

const TOWER_BASE: usize = 180;
const TOWER_TOP: usize = 203;

const ATTACK_RANGE: f32 = 192.0;
const DAMAGE: f32 = 34.0;
const ATTACK_COOLDOWN: f32 = 0.5;
const FIRE_SPRITE: usize = 295;
const MUZZLE_FLASH_DURATION: f32 = 0.15;

#[derive(Component)]
pub struct Tower;

#[derive(Component)]
pub(crate) struct TowerTurret;

#[derive(Component)]
pub(crate) struct TowerPreview;

#[derive(Component)]
pub(crate) struct AttackRange(pub f32);

#[derive(Component)]
pub(crate) struct Damage(pub f32);

#[derive(Component)]
pub(crate) struct AttackTimer(pub Timer);

#[derive(Component)]
pub(crate) struct DespawnTimer(pub Timer);

#[derive(Component)]
pub(crate) struct MuzzleFlash;

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
        GameEntity,
        tinted(TOWER_BASE),
        Transform::from_xyz(0.0, 0.0, 2.0),
        Visibility::Hidden,
    ));
    commands.spawn((
        TowerPreview,
        GameEntity,
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
    gold: Res<Gold>,
    // We need &mut Sprite to tint the preview red on denied placement.
    // Option<&PlacementDenied> tells us whether the flash is active.
    mut preview_q: Query<(&mut Transform, &mut Visibility, &mut Sprite, Option<&PlacementDenied>), With<TowerPreview>>,
) {
    let (cam, cam_transform) = *camera;

    let Some(tile) = hovered_placeable_tile(
        &window, &cam, &cam_transform, &map_layout, &placed,
    ) else {
        for (_, mut vis, ..) in preview_q.iter_mut() {
            *vis = Visibility::Hidden;
        }
        return;
    };

    let pos = tile_to_world(tile, map_layout.width as f32, map_layout.height as f32);
    // Preview is green when affordable, white when neutral, red when denied.
    let can_afford = gold.0 >= TOWER_COST as f32;

    for (mut transform, mut vis, mut sprite, denied) in preview_q.iter_mut() {
        transform.translation.x = pos.x;
        transform.translation.y = pos.y;
        *vis = Visibility::Visible;

        if denied.is_some() {
            // Red flash — the player just tried to place a tower they can't afford.
            sprite.color = Color::srgba(1.0, 0.3, 0.3, 0.5);
        } else if can_afford {
            // Green tint to signal "you can place here."
            sprite.color = Color::srgba(0.3, 1.0, 0.3, 0.5);
        } else {
            // White/neutral — tile is valid but player lacks gold.
            sprite.color = Color::srgba(1.0, 1.0, 1.0, 0.5);
        }
    }
}

pub fn place_tower_on_click(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform)>,
    map_layout: Res<MapLayout>,
    mut placed: ResMut<PlacedTowers>,
    mut gold: ResMut<Gold>,
    atlas: Res<TowerAtlas>,
    // Query preview entities so we can attach the PlacementDenied flash.
    preview_q: Query<Entity, With<TowerPreview>>,
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

    // Check affordability — if the player can't pay, flash the preview red
    // instead of silently ignoring the click.
    if gold.0 < TOWER_COST as f32 {
        for preview_entity in preview_q.iter() {
            commands.entity(preview_entity).insert(PlacementDenied(
                Timer::from_seconds(DENIED_FLASH_DURATION, TimerMode::Once),
            ));
        }
        return;
    }

    // Deduct the cost *before* spawning the tower so the player can't
    // accidentally place two towers on one click (the second would fail
    // the affordability check).
    gold.0 -= TOWER_COST as f32;
    placed.0.insert(tile);
    let pos = tile_to_world(tile, map_layout.width as f32, map_layout.height as f32);

    // Base (static, z=2.0)
    commands.spawn((
        Tower,
        GameEntity,
        Sprite::from_atlas_image(
            atlas.texture.clone(),
            TextureAtlas { layout: atlas.layout.clone(), index: TOWER_BASE },
        ),
        Transform::from_xyz(pos.x, pos.y, 2.0),
    ));

    // Turret (rotates during targeting, z=2.1)
    commands.spawn((
        TowerTurret,
        GameEntity,
        Sprite::from_atlas_image(
            atlas.texture.clone(),
            TextureAtlas { layout: atlas.layout.clone(), index: TOWER_TOP },
        ),
        Transform::from_xyz(pos.x, pos.y, 2.1),
        AttackRange(ATTACK_RANGE),
        Damage(DAMAGE),
        AttackTimer(Timer::from_seconds(ATTACK_COOLDOWN, TimerMode::Repeating)),
    ));
}

pub fn attack_enemies(
    time: Res<Time>,
    atlas: Res<TowerAtlas>,
    mut gold: ResMut<Gold>,
    mut turrets: Query<(Entity, &mut Transform, &mut AttackTimer, &Damage, &AttackRange), (With<TowerTurret>, Without<Enemy>)>,
    // Include Bounty so we can reward the player on kill.
    mut enemies: Query<(Entity, &Transform, &mut Health, &Bounty), (With<Enemy>, With<PathFollower>, Without<TowerTurret>)>,
    mut commands: Commands,
) {
    // Snapshot enemy positions, then release the query borrow.
    let enemy_positions: Vec<(Entity, Vec2)> = enemies
        .iter()
        .map(|(e, t, ..)| (e, t.translation.truncate()))
        .collect();

    for (turret_entity, mut turret_transform, mut timer, damage, range) in turrets.iter_mut() {
        timer.0.tick(time.delta());
        let turret_pos = turret_transform.translation.truncate();

        // Find nearest enemy within range
        let mut nearest: Option<(Entity, f32)> = None;
        for &(entity, pos) in &enemy_positions {
            let dist = turret_pos.distance(pos);
            if dist <= range.0 {
                if nearest.map_or(true, |(_, best)| dist < best) {
                    nearest = Some((entity, dist));
                }
            }
        }

        if let Some((target, _)) = nearest {
            let direction = enemy_positions.iter()
                .find(|(e, _)| *e == target)
                .map(|(_, p)| *p - turret_pos)
                .unwrap_or(Vec2::X);
            let angle = direction.y.atan2(direction.x) - std::f32::consts::FRAC_PI_2;
            turret_transform.rotation = Quat::from_rotation_z(angle);

            if timer.0.just_finished() {
                // Deal damage and collect bounty if the enemy dies.
                if let Ok((entity, _, mut health, bounty)) = enemies.get_mut(target) {
                    if health.0 > 0.0 {
                        health.0 -= damage.0;
                        if health.0 <= 0.0 {
                            // Enemy killed — award its bounty to the player.
                            gold.0 += bounty.0 as f32;
                            commands.entity(entity).despawn();
                        }
                    }
                }

                // Muzzle flash: two fire sprites as children of the turret.
                // Offset (x_offset, 32.0) places them at the two barrel tips.
                let texture = atlas.texture.clone();
                let layout = atlas.layout.clone();
                let mut flash_id = None;
                commands.entity(turret_entity).with_children(|turret_children| {
                    flash_id = Some(turret_children.spawn((MuzzleFlash, GameEntity, DespawnTimer(Timer::from_seconds(MUZZLE_FLASH_DURATION, TimerMode::Once)), Transform::default(), Visibility::default())).id());
                });
                if let Some(flash_id) = flash_id {
                    commands.entity(flash_id).with_children(|flash_children| {
                        for i in 0..2 {
                            let x_offset = if i == 0 {-6.0} else {6.0};
                            flash_children.spawn((Sprite::from_atlas_image(
                                texture.clone(),
                                TextureAtlas { layout: layout.clone(), index: FIRE_SPRITE },
                            ),
                                Transform::from_xyz(x_offset, 32.0, 2.2),
                            ));
                        }
                    });
                }
            }
        }
    }
}

pub fn despawn_timed(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut DespawnTimer)>
 ) {
    for (entity, mut timer) in query.iter_mut() {
        timer.0.tick(time.delta());
        if timer.0.just_finished() {
            commands.entity(entity).despawn();
        }
    }
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
