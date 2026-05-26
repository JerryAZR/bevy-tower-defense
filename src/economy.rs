//! Gold economy — the resource that gates tower placement.
//!
//! # Design
//!
//! Gold is a single `f32` resource.  Costs and bounties are discrete integers
//! but we store gold as a float so that passive income (3 gold/sec) can
//! accumulate smoothly without an external accumulator.
//!
//! The HUD reads `gold.0 as u32` every rendered frame, which naturally rounds
//! the fractional portion **down**.
//!
//! ### Simplification
//!
//! - Tower cost is a hardcoded constant.  When we add multiple tower types
//!   later it will come from data (one field per tower definition in the TOML).
//! - No gold cap — the player can stockpile indefinitely.

use bevy::prelude::*;

use crate::state::GameEntity;

// ---------------------------------------------------------------------------
// constants
// ---------------------------------------------------------------------------

/// Gold the player starts each level with.
/// At 100 gold per tower this buys 3 towers.
pub const STARTING_GOLD: f32 = 300.0;

/// Gold earned per second, passively, as long as the level is running.
pub const PASSIVE_INCOME_RATE: f32 = 3.0;

/// Cost to place one tower.  Hardcoded — will become data-driven when we
/// add multiple tower types.
pub const TOWER_COST: u32 = 100;

/// How long the placement preview stays red after the player tries to place
/// a tower they cannot afford.
pub const DENIED_FLASH_DURATION: f32 = 0.3;

// ---------------------------------------------------------------------------
// resources
// ---------------------------------------------------------------------------

/// Current gold balance.
///
/// Stored as `f32` so passive income can add fractional amounts every
/// fixed timestep.  The HUD floors it (`as u32`) for display.
#[derive(Resource)]
pub struct Gold(pub f32);

// ---------------------------------------------------------------------------
// components
// ---------------------------------------------------------------------------

/// Marker on the "Gold: N" text entity in the in-game HUD.
#[derive(Component)]
pub struct GoldHud;

/// Bounty awarded when this enemy is killed.
///
/// Attached at spawn alongside [`Health`](crate::enemy::Health).  Read by
/// the tower attack system when the enemy's health drops to ≤ 0.
#[derive(Component)]
pub struct Bounty(pub u32);

/// Temporarily tints the placement preview red when the player cannot afford
/// a tower.  Removed automatically after the timer elapses.
#[derive(Component)]
pub struct PlacementDenied(pub Timer);

// ---------------------------------------------------------------------------
// systems
// ---------------------------------------------------------------------------

/// Spawns the "Gold: N" label in the top-left corner.
///
/// Runs on `OnEnter(GameState::InGame)` so it appears only during gameplay.
pub fn spawn_gold_hud(mut commands: Commands, gold: Res<Gold>) {
    commands.spawn((
        GoldHud,
        GameEntity,
        Text::new(format!("Gold: {}", gold.0 as u32)),
        TextFont {
            font_size: 32.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(16.0),
            left: Val::Px(16.0),
            ..default()
        },
    ));
}

/// Keeps the "Gold: N" text in sync with the `Gold` resource.
///
/// Runs every rendered frame so the display is always current, even between
/// fixed timesteps.
pub fn update_gold_hud(
    gold: Res<Gold>,
    mut query: Query<&mut Text, With<GoldHud>>,
) {
    // Floor the float for display — the player sees whole gold only.
    let display = gold.0 as u32;
    for mut text in query.iter_mut() {
        *text = Text::new(format!("Gold: {}", display));
    }
}

/// Adds passive income every fixed timestep.
///
/// Runs in `FixedUpdate` alongside other gameplay logic so that income
/// scales correctly regardless of render frame rate.
pub fn earn_passive_income(time: Res<Time>, mut gold: ResMut<Gold>) {
    gold.0 += PASSIVE_INCOME_RATE * time.delta_secs();
}

/// Counts down the placement-denied flash timer.
///
/// Runs every rendered frame for a snappy visual (FixedUpdate's default
/// 64 ms timestep would make a 0.3 s flash feel sluggish).
pub fn tick_placement_denied(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut PlacementDenied)>,
) {
    for (entity, mut denied) in query.iter_mut() {
        denied.0.tick(time.delta());
        if denied.0.just_finished() {
            commands.entity(entity).remove::<PlacementDenied>();
        }
    }
}
