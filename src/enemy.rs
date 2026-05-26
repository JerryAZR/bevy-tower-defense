use bevy::prelude::*;

use std::collections::VecDeque;

use crate::level::LevelData;
use crate::state::{GameEntity, GameState, GameResult, BaseLives, GameFinished};

#[derive(Component)]
pub struct Enemy;

#[derive(Component)]
pub struct PathFollower {
    pub path_id: String,
    pub waypoint_index: usize,
    pub target: Vec2,
}

#[derive(Component)]
pub struct MoveSpeed(pub f32);

#[derive(Component)]
pub struct Health(pub f32);



pub struct SpawnEvent {
    time: f32,
    sprite: usize,
    speed: f32,
    health: f32,
    path: String,
}

#[derive(Resource)]
pub struct SpawnSchedule {
    events: VecDeque<SpawnEvent>,
    elapsed: f32,
    texture: Handle<Image>,
    atlas: Handle<TextureAtlasLayout>,
}

pub fn build_spawn_schedule(
    level: &LevelData,
    asset_server: &AssetServer,
    texture_atlas_layouts: &mut Assets<TextureAtlasLayout>,
) -> SpawnSchedule {
    let mut events: Vec<SpawnEvent> = Vec::new();

    for wave in &level.waves {
        let mut time = wave.start_time;
        for group in &wave.enemies {
            let def = &level.enemy_types[&group.enemy_type];
            for _ in 0..group.count {
                events.push(SpawnEvent {
                    time,
                    sprite: def.sprite,
                    speed: def.speed,
                    health: def.health,
                    path: wave.path.clone(),
                });
                time += wave.spawn_interval;
            }
        }
    }

    events.sort_by(|a, b| a.time.total_cmp(&b.time));

    SpawnSchedule {
        events: events.into(),
        elapsed: 0.0,
        texture: asset_server.load("Tilesheet/towerDefense_tilesheet.png"),
        atlas: texture_atlas_layouts.add(
            TextureAtlasLayout::from_grid(UVec2::splat(64), 23, 13, None, None)
        ),
    }
}

pub fn spawn_wave_enemies(
    mut schedule: ResMut<SpawnSchedule>,
    mut commands: Commands,
    level: Res<LevelData>,
    time: Res<Time>,
) {
    schedule.elapsed += time.delta_secs();

    while let Some(event) = schedule.events.front() {
        if event.time > schedule.elapsed {
            break;
        }
        let event = schedule.events.pop_front().unwrap();

        let waypoints = &level.paths[&event.path].waypoints;
        let spawn_tile = waypoints[0];
        let target_tile = waypoints[1];

        let map_width = level.map.width as f32;
        let map_height = level.map.height as f32;
        let tile_size = 64.0;
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
                schedule.texture.clone(),
                TextureAtlas {
                    layout: schedule.atlas.clone(),
                    index: event.sprite,
                },
            ),
            Transform::from_xyz(x, y, 1.0),
            Enemy,
            PathFollower {
                path_id: event.path,
                waypoint_index: 1,
                target,
            },
            MoveSpeed(event.speed),
            Health(event.health),
            GameEntity,
        ));
    }
}

pub fn move_enemies(
    mut commands: Commands,
    mut query: Query<(Entity, &mut PathFollower, &mut Transform, &MoveSpeed), With<Enemy>>,
    level: Res<LevelData>,
    time: Res<Time>,
) {
    let map_width = level.map.width as f32;
    let map_height = level.map.height as f32;

    for (entity, mut follower, mut transform, speed) in query.iter_mut() {
        let current = transform.translation.truncate();
        let to_target = follower.target - current;
        let distance = to_target.length();

        if distance <= 0.01 {
            // Snap distance: avoid division by zero / atan2 instability when
            // the enemy is already on (or extremely close to) the target tile.
            // Already at target -- advance immediately.
            advance_waypoint(&mut commands, entity, &mut follower, &level, map_width, map_height);
            continue;
        }

        let direction = to_target / distance;
        let step = speed.0 * time.delta_secs();
        let angle = direction.y.atan2(direction.x);

        if distance <= step {
            // Reach (or would overshoot) the waypoint this frame.
            transform.translation.x = follower.target.x;
            transform.translation.y = follower.target.y;
            transform.rotation = Quat::from_rotation_z(angle);
            advance_waypoint(&mut commands, entity, &mut follower, &level, map_width, map_height);
        } else {
            let new_pos = current + direction * step;
            transform.translation.x = new_pos.x;
            transform.translation.y = new_pos.y;
            transform.rotation = Quat::from_rotation_z(angle);
        }
    }
}

fn advance_waypoint(
    commands: &mut Commands,
    entity: Entity,
    follower: &mut PathFollower,
    level: &LevelData,
    map_width: f32,
    map_height: f32,
) {
    let waypoints = &level.paths[&follower.path_id].waypoints;
    follower.waypoint_index += 1;

    if follower.waypoint_index >= waypoints.len() {
        commands.entity(entity).remove::<PathFollower>();
    } else {
        follower.target = tile_to_world(waypoints[follower.waypoint_index], map_width, map_height);
    }
}

pub fn tile_to_world(tile: [u32; 2], map_width: f32, map_height: f32) -> Vec2 {
    let tile_size = 64.0;
    let origin_x = -map_width * tile_size / 2.0 + tile_size / 2.0;
    let origin_y = -map_height * tile_size / 2.0 + tile_size / 2.0;
    Vec2::new(
        origin_x + tile[0] as f32 * tile_size,
        origin_y + tile[1] as f32 * tile_size,
    )
}

pub fn process_base_reachers(
    mut commands: Commands,
    mut lives: ResMut<BaseLives>,
    query: Query<Entity, (With<Enemy>, Without<PathFollower>)>,
) {
    for entity in &query {
        lives.0 -= 1;
        commands.entity(entity).despawn();
    }
}

pub fn check_game_state(
    mut commands: Commands,
    finished: Option<Res<GameFinished>>,
    lives: Res<BaseLives>,
    schedule: Res<SpawnSchedule>,
    alive: Query<(), With<Enemy>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if finished.is_some() {
        return;
    }
    if lives.0 <= 0 {
        info!("Game Over -- the base has been destroyed!");
        commands.insert_resource(GameResult::Defeat);
        commands.insert_resource(GameFinished);
        next_state.set(GameState::GameOver);
    } else if schedule.events.is_empty() && alive.iter().count() == 0 {
        info!("Victory -- all enemies defeated!");
        commands.insert_resource(GameResult::Victory);
        commands.insert_resource(GameFinished);
        next_state.set(GameState::GameOver);
    }
}
