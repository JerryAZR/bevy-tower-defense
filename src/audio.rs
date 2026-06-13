use bevy::prelude::*;
use bevy::audio::Volume;
use bevy::ecs::message::MessageReader;
use crate::state::GameState;
// ---------------------------------------------------------------------------
// audio assets — loaded once during startup
// ---------------------------------------------------------------------------

#[derive(Resource)]
pub struct AudioAssets {
    pub laser_fire: Handle<AudioSource>,
    pub rocket_explosion: Handle<AudioSource>,
    pub rocket_launch: Handle<AudioSource>,
    pub background_music: Handle<AudioSource>,
}

// ---------------------------------------------------------------------------
// marker component for background music entity (so we can stop it cleanly)
// ---------------------------------------------------------------------------

#[derive(Component)]
pub struct BackgroundMusic;

// ---------------------------------------------------------------------------
// one-shot sound effect messages — emitted by gameplay, consumed by audio
// ---------------------------------------------------------------------------

#[derive(bevy::ecs::message::Message, Clone)]
pub enum SoundType {
    LaserFire,
    RocketExplosion,
}

#[derive(bevy::ecs::message::Message, Clone)]
pub struct PlaySound {
    pub sound: SoundType,
    pub volume: f32,
}

// ---------------------------------------------------------------------------
// load — called during startup alongside sprite atlas loading
// ---------------------------------------------------------------------------

pub fn load_audio_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    commands.insert_resource(AudioAssets {
        laser_fire: asset_server.load("audio/laser_fire.ogg"),
        rocket_explosion: asset_server.load("audio/rocket_explosion.ogg"),
        rocket_launch: asset_server.load("audio/rocket_launch.ogg"),
        background_music: asset_server.load("audio/background_music.ogg"),
    });
}

// ---------------------------------------------------------------------------
// background music — starts on level entry, stops on level exit
// ---------------------------------------------------------------------------

pub fn start_background_music(
    mut commands: Commands,
    audio_assets: Res<AudioAssets>,
) {
    commands.spawn((
        AudioPlayer::new(audio_assets.background_music.clone()),
        PlaybackSettings::LOOP.with_volume(Volume::Linear(0.3)),
        BackgroundMusic,
    ));
}

pub fn stop_background_music(
    mut commands: Commands,
    music: Query<Entity, With<BackgroundMusic>>,
) {
    for entity in music.iter() {
        commands.entity(entity).despawn();
    }
}

// ---------------------------------------------------------------------------
// one-shot SFX consumer — spawns ephemeral audio entities that auto-despawn
// ---------------------------------------------------------------------------

pub fn play_sound_effects(
    mut messages: MessageReader<PlaySound>,
    mut commands: Commands,
    audio_assets: Res<AudioAssets>,
) {
    for event in messages.read() {
        let handle = match event.sound {
            SoundType::LaserFire => audio_assets.laser_fire.clone(),
            SoundType::RocketExplosion => audio_assets.rocket_explosion.clone(),
        };
        commands.spawn((
            AudioPlayer::new(handle),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(event.volume)),
        ));
    }
}

// ---------------------------------------------------------------------------
// AudioPlugin — bundles all audio systems into a self-contained unit
// ---------------------------------------------------------------------------

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_message::<PlaySound>()
            .add_systems(Startup, load_audio_assets)
            .add_systems(OnEnter(GameState::InGame), start_background_music)
            .add_systems(OnExit(GameState::InGame), stop_background_music)
            .add_systems(Update, play_sound_effects
                .run_if(in_state(GameState::InGame)));
    }
}
