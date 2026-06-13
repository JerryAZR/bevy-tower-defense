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
mod audio;
mod pause;
mod input;
use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;

use state::{GameState, GameplaySet, game_is_running, AvailableLevels, spawn_camera, cleanup_level, cleanup_screen_ui};
use enemy::{spawn_wave_enemies, move_enemies, process_base_reachers, check_game_state};
use tower::{setup_tower_atlas, spawn_placement_preview, update_placement_preview, place_tower_on_click, spawn_tower_from_event, attack_enemies, despawn_timed, refill_ammo, launch_rockets, move_projectiles, explode_projectiles, load_tower_registry, setup_tower_dock, select_tower_by_key, update_dock_selection, handle_dock_slot_click, draw_tower_ranges, PlaceTower};
use gameplay::{load_level_data, setup_spawn_schedule, spawn_tilemap};
use level_select::{scan_available_levels, setup_level_select, navigate_level_select, update_level_select_visuals, handle_level_button_click};
use game_over::{setup_game_over, handle_game_over_input};
use economy::{spawn_gold_hud, update_gold_hud, earn_passive_income, tick_placement_denied, deduct_gold_on_placement};
use audio::AudioPlugin;
use pause::PausePlugin;
use input::InputPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .add_plugins(TilemapPlugin)
        .add_plugins(AudioPlugin)
        .add_plugins(PausePlugin)
        .add_plugins(InputPlugin)
        .init_state::<GameState>()
        .init_resource::<AvailableLevels>()
        .add_message::<PlaceTower>()
        .configure_sets(FixedUpdate, GameplaySet::Simulation.run_if(game_is_running))
        .configure_sets(Update, (
            GameplaySet::Interaction,
            GameplaySet::TowerDock,
        ).run_if(game_is_running))
        .add_systems(Startup, (load_tower_registry, spawn_camera, setup_tower_atlas).chain())
        // ---------- LevelSelect ----------
        .add_systems(OnEnter(GameState::LevelSelect), (
            scan_available_levels,
            setup_level_select,
        ).chain())
        .add_systems(OnExit(GameState::LevelSelect), cleanup_screen_ui)
        .add_systems(Update, (
            navigate_level_select,
            update_level_select_visuals,
            handle_level_button_click,
        ).run_if(in_state(GameState::LevelSelect)))
        // ---------- InGame ----------
        .add_systems(OnEnter(GameState::InGame), (
            load_level_data,
            setup_spawn_schedule,
            spawn_tilemap,
            spawn_placement_preview,
            spawn_gold_hud,
            setup_tower_dock,
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
        ).chain().in_set(GameplaySet::Simulation))
        .add_systems(Update, (
            update_placement_preview,
            place_tower_on_click,
            spawn_tower_from_event.after(place_tower_on_click),
            deduct_gold_on_placement.after(place_tower_on_click),
            despawn_timed,
            update_gold_hud,
            tick_placement_denied,
            draw_tower_ranges,
        ).in_set(GameplaySet::Interaction))
        .add_systems(Update, (
            select_tower_by_key,
            update_dock_selection,
            handle_dock_slot_click,
        ).in_set(GameplaySet::TowerDock))
        // ---------- GameOver ----------
        .add_systems(OnEnter(GameState::GameOver), setup_game_over)
        .add_systems(OnExit(GameState::GameOver), cleanup_screen_ui)
        .add_systems(Update, handle_game_over_input
            .run_if(in_state(GameState::GameOver)))
        .run();
}
