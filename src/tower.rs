use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use serde::Deserialize;

use crate::enemy::tile_to_world;
use crate::map::{MapLayout, TileType};
use crate::enemy::{Enemy, Health, PathFollower};
use crate::state::GameEntity;
use crate::economy::{Gold, Bounty, PlacementDenied, DENIED_FLASH_DURATION};

// ---------------------------------------------------------------------------
// tower registry — raw TOML representation
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct TowerRegistryRaw {
    #[serde(rename = "towers")]
    towers: HashMap<String, TowerDefinitionRaw>,
    #[serde(rename = "projectiles")]
    projectiles: HashMap<String, ProjectileDefinitionRaw>,
}

#[derive(Debug, Deserialize, Clone)]
struct TowerDefinitionRaw {
    name: String,
    description: String,
    base_sprite: usize,
    top_sprite: usize,
    preview_top_sprite: usize,
    cost: u32,
    attack_range: f32,
    attack_cooldown: f32,
    damage: Option<f32>,
    muzzle_flash_sprite: Option<usize>,
    projectile: Option<String>,
    ammo_slot_offsets: Option<Vec<[f32; 2]>>,
    ammo_refill_secs: Option<f32>,
}

#[derive(Debug, Deserialize, Clone)]
struct ProjectileDefinitionRaw {
    damage: f32,
    speed: f32,
    sprite: usize,
    explosion_sprite: Option<usize>,
    splash_radius: Option<f32>,
}

// ---------------------------------------------------------------------------
// tower registry — runtime representation (projectile resolved at load time)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Resource)]
pub struct TowerRegistry {
    pub towers: HashMap<String, TowerDefinition>,
}

#[derive(Debug, Clone)]
pub struct TowerDefinition {
    pub name: String,
    #[allow(dead_code)]
    pub description: String,
    pub base_sprite: usize,
    pub top_sprite: usize,
    pub preview_top_sprite: usize,
    pub cost: u32,
    pub attack_range: f32,
    pub attack_cooldown: f32,
    pub damage: Option<f32>,
    pub muzzle_flash_sprite: Option<usize>,
    pub projectile: Option<ProjectileDefinition>,
    pub ammo_slot_offsets: Option<Vec<[f32; 2]>>,
    pub ammo_refill_secs: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct ProjectileDefinition {
    pub damage: f32,
    pub speed: f32,
    pub sprite: usize,
    pub explosion_sprite: Option<usize>,
    pub splash_radius: Option<f32>,
}

// ---------------------------------------------------------------------------
// components
// ---------------------------------------------------------------------------

const MUZZLE_FLASH_DURATION: f32 = 0.15;

#[derive(Component)]
pub(crate) struct TowerPreview;

/// Shared combat state on every tower that can shoot (instant or rocket).
#[derive(Component)]
pub(crate) struct TowerAttacker {
    pub range: f32,
    pub timer: Timer,
}

/// Instant-tower-specific state. Also acts as the discriminator:
/// `With<InstantShooter>` finds instant towers.
#[derive(Component)]
pub(crate) struct InstantShooter {
    pub damage: f32,
    pub muzzle_flash_sprite: usize,
}

/// Rocket-tower-specific state. Also acts as the discriminator:
/// `With<AmmoState>` finds rocket launchers.
#[derive(Component)]
pub(crate) struct AmmoState {
    pub regen: Timer,
    pub slots: Vec<Option<Entity>>,
}

#[derive(Component)]
pub(crate) struct TowerTypeId(pub String);

#[derive(Component)]
pub(crate) struct DespawnTimer(pub Timer);

#[derive(Component)]
pub(crate) struct Projectile {
    pub target: Entity,
    pub target_position: Vec2,
    pub speed: f32,
    pub damage: f32,
    pub splash_radius: f32,
    pub explosion_sprite: usize,
}

#[derive(Component)]
pub(crate) struct Exploding;

#[derive(Component)]
pub(crate) struct MuzzleFlash;

#[derive(Resource)]
pub struct TowerAtlas {
    texture: Handle<Image>,
    layout: Handle<TextureAtlasLayout>,
}

#[derive(Resource, Default)]
pub struct PlacedTowers(pub HashSet<[u32; 2]>);

#[derive(Resource)]
pub struct SelectedTowerType(pub String);

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

pub fn load_tower_registry(mut commands: Commands) {
    let raw: TowerRegistryRaw = {
        let content = std::fs::read_to_string("assets/towers.toml")
            .unwrap_or_else(|e| panic!("Failed to read assets/towers.toml: {}", e));
        toml::from_str(&content)
            .unwrap_or_else(|e| panic!("Failed to parse assets/towers.toml: {}", e))
    };

    let mut towers = HashMap::new();
    for (id, def_raw) in raw.towers {
        let projectile = def_raw.projectile.as_ref().and_then(|p| {
            raw.projectiles.get(p).cloned().map(|pr| ProjectileDefinition {
                damage: pr.damage,
                speed: pr.speed,
                sprite: pr.sprite,
                explosion_sprite: pr.explosion_sprite,
                splash_radius: pr.splash_radius,
            })
        });

        towers.insert(id, TowerDefinition {
            name: def_raw.name,
            description: def_raw.description,
            base_sprite: def_raw.base_sprite,
            top_sprite: def_raw.top_sprite,
            preview_top_sprite: def_raw.preview_top_sprite,
            cost: def_raw.cost,
            attack_range: def_raw.attack_range,
            attack_cooldown: def_raw.attack_cooldown,
            damage: def_raw.damage,
            muzzle_flash_sprite: def_raw.muzzle_flash_sprite,
            projectile,
            ammo_slot_offsets: def_raw.ammo_slot_offsets,
            ammo_refill_secs: def_raw.ammo_refill_secs,
        });
    }

    commands.insert_resource(TowerRegistry { towers });
    commands.insert_resource(SelectedTowerType("rocket".to_string()));
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
    registry: Res<TowerRegistry>,
    selected: Res<SelectedTowerType>,
) {
    let tinted = |index: usize| Sprite {
        color: Color::srgba(1.0, 1.0, 1.0, 0.5),
        ..Sprite::from_atlas_image(
            atlas.texture.clone(),
            TextureAtlas { layout: atlas.layout.clone(), index },
        )
    };

    let def = registry.towers.get(&selected.0)
        .unwrap_or_else(|| panic!("Unknown tower type: {}", selected.0));

    commands.spawn((
        TowerPreview,
        GameEntity,
        tinted(def.base_sprite),
        Transform::from_xyz(0.0, 0.0, 2.0),
        Visibility::Hidden,
    ));
    commands.spawn((
        TowerPreview,
        GameEntity,
        tinted(def.preview_top_sprite),
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
    registry: Res<TowerRegistry>,
    selected: Res<SelectedTowerType>,
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

    let def = registry.towers.get(&selected.0)
        .expect("Selected tower type must exist in registry");
    let can_afford = gold.0 >= def.cost as f32;

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

/// Spawns an instant-damage tower (base + turret).
/// Reads all stats from the tower definition.
fn spawn_instant_tower(
    commands: &mut Commands,
    atlas: &TowerAtlas,
    def: &TowerDefinition,
    tower_key: &str,
    pos: Vec2,
) {
    commands.spawn((
        GameEntity,
        Sprite::from_atlas_image(
            atlas.texture.clone(),
            TextureAtlas { layout: atlas.layout.clone(), index: def.base_sprite },
        ),
        Transform::from_xyz(pos.x, pos.y, 2.0),
    ));

    commands.spawn((
        TowerTypeId(tower_key.to_string()),
        GameEntity,
        Sprite::from_atlas_image(
            atlas.texture.clone(),
            TextureAtlas { layout: atlas.layout.clone(), index: def.top_sprite },
        ),
        Transform::from_xyz(pos.x, pos.y, 2.1),
        TowerAttacker {
            range: def.attack_range,
            timer: Timer::from_seconds(def.attack_cooldown, TimerMode::Repeating),
        },
        InstantShooter {
            damage: def.damage.expect("instant tower must have damage"),
            muzzle_flash_sprite: def.muzzle_flash_sprite.expect("instant tower must have muzzle_flash_sprite"),
        },
    ));
}
/// Spawns a rocket launcher tower (base + barrel + ammo slots).
/// Reads all stats from the tower definition.
fn spawn_rocket_launcher(
    commands: &mut Commands,
    atlas: &TowerAtlas,
    def: &TowerDefinition,
    tower_key: &str,
    pos: Vec2,
) {
    commands.spawn((
        GameEntity,
        Sprite::from_atlas_image(
            atlas.texture.clone(),
            TextureAtlas { layout: atlas.layout.clone(), index: def.base_sprite },
        ),
        Transform::from_xyz(pos.x, pos.y, 2.0),
    ));

    let ammo_offsets = def.ammo_slot_offsets.clone()
        .expect("rocket tower must have ammo_slot_offsets");
    let ammo_refill = def.ammo_refill_secs
        .expect("rocket tower must have ammo_refill_secs");
    let ammo_sprite = def.projectile.as_ref()
        .expect("rocket tower must have a projectile definition")
        .sprite;

    let mut slot_entities = vec![None; ammo_offsets.len()];
    let turret_entity = commands.spawn((
        TowerTypeId(tower_key.to_string()),
        GameEntity,
        Sprite::from_atlas_image(
            atlas.texture.clone(),
            TextureAtlas { layout: atlas.layout.clone(), index: def.top_sprite },
        ),
        Transform::from_xyz(pos.x, pos.y, 2.1),
        TowerAttacker {
            range: def.attack_range,
            timer: Timer::from_seconds(def.attack_cooldown, TimerMode::Repeating),
        },
    )).id();

    commands.entity(turret_entity).with_children(|turret_children| {
        for (i, offset) in ammo_offsets.iter().enumerate() {
            let slot_entity = turret_children.spawn((
                Sprite::from_atlas_image(
                    atlas.texture.clone(),
                    TextureAtlas { layout: atlas.layout.clone(), index: ammo_sprite },
                ),
                Transform::from_xyz(offset[0], offset[1], 2.2),
            )).id();
            slot_entities[i] = Some(slot_entity);
        }
    });
    commands.entity(turret_entity).insert(AmmoState {
        regen: Timer::from_seconds(ammo_refill, TimerMode::Repeating),
        slots: slot_entities,
    });
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
    registry: Res<TowerRegistry>,
    selected: Res<SelectedTowerType>,
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

    let def = registry.towers.get(&selected.0)
        .expect("Selected tower type must exist in registry");

    // Check affordability — if the player can't pay, flash the preview red
    // instead of silently ignoring the click.
    if gold.0 < def.cost as f32 {
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
    gold.0 -= def.cost as f32;
    placed.0.insert(tile);
    let pos = tile_to_world(tile, map_layout.width as f32, map_layout.height as f32);

    if def.damage.is_some() {
        spawn_instant_tower(&mut commands, &atlas, def, &selected.0, pos);
    } else {
        spawn_rocket_launcher(&mut commands, &atlas, def, &selected.0, pos);
    }
}

pub fn attack_enemies(
    time: Res<Time>,
    atlas: Res<TowerAtlas>,
    mut gold: ResMut<Gold>,
    mut turrets: Query<(Entity, &mut Transform, &mut TowerAttacker, &InstantShooter), Without<Enemy>>,
    // Include Bounty so we can reward the player on kill.
    mut enemies: Query<(Entity, &Transform, &mut Health, &Bounty), (With<Enemy>, With<PathFollower>, Without<InstantShooter>)>,
    mut commands: Commands,
) {
    // Snapshot enemy positions, then release the query borrow.
    let enemy_positions: Vec<(Entity, Vec2)> = enemies
        .iter()
        .map(|(e, t, ..)| (e, t.translation.truncate()))
        .collect();

    for (turret_entity, mut turret_transform, mut attacker, instant) in turrets.iter_mut() {
        attacker.timer.tick(time.delta());
        let turret_pos = turret_transform.translation.truncate();

        // Find nearest enemy within range
        let mut nearest: Option<(Entity, f32)> = None;
        for &(entity, pos) in &enemy_positions {
            let dist = turret_pos.distance(pos);
            if dist <= attacker.range {
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

            if attacker.timer.just_finished() {
                // Deal damage and collect bounty if the enemy dies.
                if let Ok((entity, _, mut health, bounty)) = enemies.get_mut(target) {
                    if health.0 > 0.0 {
                        health.0 -= instant.damage;
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
                                TextureAtlas { layout: layout.clone(), index: instant.muzzle_flash_sprite },
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
pub fn refill_ammo(
    time: Res<Time>,
    mut turrets: Query<(Entity, &TowerTypeId, &mut AmmoState)>,
    mut commands: Commands,
    atlas: Res<TowerAtlas>,
    registry: Res<TowerRegistry>,
) {
    for (turret_entity, tower_id, mut ammo) in turrets.iter_mut() {
        ammo.regen.tick(time.delta());
        if ammo.regen.just_finished() {
            let empty_idx = ammo.slots.iter().position(|s| s.is_none());
            if let Some(idx) = empty_idx {
                let def = registry.towers.get(&tower_id.0)
                    .expect("Tower type must exist in registry");
                let ammo_offsets = def.ammo_slot_offsets.as_ref()
                    .expect("rocket tower must have ammo_slot_offsets");
                let ammo_sprite = def.projectile.as_ref()
                    .expect("rocket tower must have a projectile definition")
                    .sprite;
                let offset = ammo_offsets[idx];

                let mut new_entity = None;
                commands.entity(turret_entity).with_children(|turret_children| {
                    new_entity = Some(turret_children.spawn((
                        Sprite::from_atlas_image(
                            atlas.texture.clone(),
                            TextureAtlas { layout: atlas.layout.clone(), index: ammo_sprite },
                        ),
                        Transform::from_xyz(offset[0], offset[1], 2.2),
                    )).id());
                });
                if let Some(entity) = new_entity {
                    ammo.slots[idx] = Some(entity);
                }
            }
        }
    }
}

pub fn launch_rockets(
    time: Res<Time>,
    atlas: Res<TowerAtlas>,
    mut turrets: Query<(Entity, &mut Transform, &mut TowerAttacker, &mut AmmoState, &TowerTypeId), Without<Enemy>>,
    enemies: Query<(Entity, &Transform), (With<Enemy>, With<PathFollower>, Without<AmmoState>)>,
    slot_transforms: Query<&GlobalTransform>,
    mut commands: Commands,
    registry: Res<TowerRegistry>,
) {
    let enemy_positions: Vec<(Entity, Vec2)> = enemies
        .iter()
        .map(|(e, t)| (e, t.translation.truncate()))
        .collect();

    for (_turret_entity, mut turret_transform, mut attacker, mut ammo, tower_id) in turrets.iter_mut() {
        attacker.timer.tick(time.delta());
        let turret_pos = turret_transform.translation.truncate();

        let mut nearest: Option<(Entity, f32)> = None;
        for &(entity, pos) in &enemy_positions {
            let dist = turret_pos.distance(pos);
            if dist <= attacker.range {
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

            if attacker.timer.just_finished() {
                let slot_idx = ammo.slots.iter().position(|s| s.is_some());
                if let Some(idx) = slot_idx {
                    let ammo_entity = ammo.slots[idx].take().unwrap();
                    // Spawn the projectile at the slot's world position so it
                    // visually detaches from the launcher rather than teleporting.
                    let spawn_pos = slot_transforms.get(ammo_entity)
                        .map(|gt| gt.translation().truncate())
                        .unwrap_or(turret_pos);
                    commands.entity(ammo_entity).despawn();

                    let target_pos = enemy_positions.iter()
                        .find(|(e, _)| *e == target)
                        .map(|(_, p)| *p)
                        .unwrap_or(turret_pos + direction);

                    let def = registry.towers.get(&tower_id.0)
                        .expect("Tower type must exist in registry");
                    let proj_def = def.projectile.as_ref()
                        .expect("rocket tower must have a projectile definition");

                    commands.spawn((
                        Projectile {
                            target,
                            target_position: target_pos,
                            speed: proj_def.speed,
                            damage: proj_def.damage,
                            splash_radius: proj_def.splash_radius.unwrap_or(0.0),
                            explosion_sprite: proj_def.explosion_sprite.unwrap_or(0),
                        },
                        GameEntity,
                        Sprite::from_atlas_image(
                            atlas.texture.clone(),
                            TextureAtlas { layout: atlas.layout.clone(), index: proj_def.sprite },
                        ),
                        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 2.3),
                    ));
                }
            }
        }
    }
}

pub fn move_projectiles(
    time: Res<Time>,
    mut projectiles: Query<(Entity, &mut Transform, &mut Projectile), (Without<Exploding>, Without<Enemy>)>,
    enemies: Query<&Transform, (With<Enemy>, Without<Exploding>, Without<Projectile>)>,
    mut commands: Commands,
) {
    for (entity, mut transform, mut projectile) in projectiles.iter_mut() {
        // Homing: update target position if the enemy is still alive.
        if let Ok(target_transform) = enemies.get(projectile.target) {
            projectile.target_position = target_transform.translation.truncate();
        }

        let current_pos = transform.translation.truncate();
        let to_target = projectile.target_position - current_pos;
        let distance = to_target.length();
        let move_dist = projectile.speed * time.delta_secs();

        // Face the target — the rocket sprite points up by default.
        if distance > 0.0 {
            let direction = to_target.normalize();
            let angle = direction.y.atan2(direction.x) - std::f32::consts::FRAC_PI_2;
            transform.rotation = Quat::from_rotation_z(angle);
        }

        if distance <= move_dist {
            transform.translation.x = projectile.target_position.x;
            transform.translation.y = projectile.target_position.y;
            commands.entity(entity).insert(Exploding);
        } else {
            let direction = to_target.normalize();
            transform.translation += (direction * move_dist).extend(0.0);
        }
    }
}

pub fn explode_projectiles(
    mut commands: Commands,
    atlas: Res<TowerAtlas>,
    mut gold: ResMut<Gold>,
    projectiles: Query<(Entity, &Transform, &Projectile), (With<Exploding>, Without<Enemy>)>,
    mut enemies: Query<(Entity, &Transform, &mut Health, &Bounty), (With<Enemy>, Without<Exploding>)>,
) {
    for (proj_entity, proj_transform, projectile) in projectiles.iter() {
        let pos = proj_transform.translation.truncate();

        for (enemy_entity, enemy_transform, mut health, bounty) in enemies.iter_mut() {
            if pos.distance(enemy_transform.translation.truncate()) <= projectile.splash_radius {
                if health.0 > 0.0 {
                    health.0 -= projectile.damage;
                    if health.0 <= 0.0 {
                        gold.0 += bounty.0 as f32;
                        commands.entity(enemy_entity).despawn();
                    }
                }
            }
        }

        commands.spawn((
            GameEntity,
            DespawnTimer(Timer::from_seconds(0.15, TimerMode::Once)),
            Sprite::from_atlas_image(
                atlas.texture.clone(),
                TextureAtlas { layout: atlas.layout.clone(), index: projectile.explosion_sprite },
            ),
            Transform::from_xyz(pos.x, pos.y, 2.4),
            Visibility::default(),
        ));

        commands.entity(proj_entity).despawn();
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