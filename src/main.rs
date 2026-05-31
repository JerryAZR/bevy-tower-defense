mod level;
mod map;
mod tiling;
mod enemy;
mod tower;
mod state;
mod gameplay;
mod level_select;
mod game_over;
mod economy;
use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;

use state::{GameState, AvailableLevels, spawn_camera, cleanup_level, cleanup_screen_ui};
use enemy::{spawn_wave_enemies, move_enemies, process_base_reachers, check_game_state};
use tower::{setup_tower_atlas, spawn_placement_preview, update_placement_preview, place_tower_on_click, attack_enemies, despawn_timed, refill_ammo, launch_rockets, move_projectiles, explode_projectiles};
use gameplay::{load_level_data, setup_spawn_schedule, spawn_tilemap};
use level_select::{scan_available_levels, setup_level_select, handle_level_select_input};
use game_over::{setup_game_over, handle_game_over_input};
use economy::{spawn_gold_hud, update_gold_hud, earn_passive_income, tick_placement_denied};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .add_plugins(TilemapPlugin)
        .init_state::<GameState>()
        .init_resource::<AvailableLevels>()
        .add_systems(Startup, (spawn_camera, setup_tower_atlas).chain())
        // ---------- LevelSelect ----------
        .add_systems(OnEnter(GameState::LevelSelect), (
            scan_available_levels,
            setup_level_select,
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
            spawn_gold_hud,
        ).chain())
        .add_systems(OnExit(GameState::InGame), cleanup_level)
        .add_systems(FixedUpdate, (
            spawn_wave_enemies,
            move_enemies,
            attack_enemies,
            refill_ammo,
            launch_rockets,
            move_projectiles,
            explode_projectiles,
            process_base_reachers,
            check_game_state,
            earn_passive_income,
        ).chain().run_if(in_state(GameState::InGame)))
        .add_systems(Update, (
            update_placement_preview,
            place_tower_on_click,
            despawn_timed,
            update_gold_hud,
            tick_placement_denied,
        ).run_if(in_state(GameState::InGame)))
        // ---------- GameOver ----------
        .add_systems(OnEnter(GameState::GameOver), setup_game_over)
        .add_systems(OnExit(GameState::GameOver), cleanup_screen_ui)
        .add_systems(Update, handle_game_over_input
            .run_if(in_state(GameState::GameOver)))
        .run();
}
