# Part 20: Audio — Background Music and Sound Effects

> **Time to read:** ~12 minutes
> **New concepts:** `AudioPlayer`, `PlaybackSettings`, `Volume`, loading audio assets
> **Prerequisite:** Part 19 (gizmos)

---

## Recap: What We Already Have

The game is fully playable: towers attack, enemies die, gold changes, gizmos draw range rings. But it is completely silent. There is no feedback when a tower fires, when a rocket explodes, or when a level starts. Audio is the last major sensory channel we have not wired up.

---

## Goal: What We Will Build

Two kinds of audio:

1. **Background music** — a looping track that plays during `InGame` and stops when the level ends.
2. **Sound effects** — three one-shot sounds triggered by gameplay events:
   - **Laser fire** — when an instant shooter tower attacks.
   - **Rocket launch** — a looping thruster sound that follows the projectile while it flies.
   - **Rocket explosion** — when a projectile hits and deals splash damage.

These two patterns cover the two most common audio types in games: *persistent* (music) and *transient* (SFX).

---

## New Bevy APIs & Concepts

### `AudioPlayer`

In Bevy, audio is component-driven. To play a sound, you spawn an entity with two components: `AudioPlayer` (which holds a `Handle<AudioSource>`) and `PlaybackSettings` (which controls how it plays).

```rust
commands.spawn((
    AudioPlayer::new(handle),
    PlaybackSettings::DESPAWN,
));
```

Unlike many game engines where you call a `play_sound()` method on an audio manager, Bevy treats sounds as entities. This means audio naturally participates in the ECS: it can be queried, despawned, and parented like any other entity.

### `PlaybackSettings`

`PlaybackSettings` is a component that configures how `AudioPlayer` behaves. Bevy provides four common presets:

- `PlaybackSettings::ONCE` — play once, do nothing when finished.
- `PlaybackSettings::LOOP` — repeat forever.
- `PlaybackSettings::DESPAWN` — play once, then despawn the entity automatically.
- `PlaybackSettings::REMOVE` — play once, then remove the audio components.

For one-shot SFX we use `DESPAWN`, because the entity cleans itself up. For background music we use `LOOP`.

You can also chain modifiers like `.with_volume(Volume::Linear(0.3))` to attenuate a sound without editing the source file.

### Loading audio assets

Audio files are loaded the same way as images:

```rust
asset_server.load("audio/laser_fire.ogg")
```

Bevy supports Ogg Vorbis out of the box. The handle can be stored in a resource so multiple systems can reference it.

---

## Walkthrough

### The audio module

Create a new file `src/audio.rs`. This module owns every audio-related type and system.

#### 1. `AudioAssets`

We need a resource to hold the loaded handles. Every system that plays audio will read from it.

```rust
use bevy::prelude::*;
use bevy::audio::Volume;
use bevy::ecs::message::MessageReader;

#[derive(Resource)]
pub struct AudioAssets {
    pub laser_fire: Handle<AudioSource>,
    pub rocket_explosion: Handle<AudioSource>,
    pub rocket_launch: Handle<AudioSource>,
    pub background_music: Handle<AudioSource>,
}
```

#### 2. `BackgroundMusic` marker

The music entity lives for an entire level. We mark it so a dedicated system can find and stop it on level exit.

```rust
#[derive(Component)]
pub struct BackgroundMusic;
```

#### 3. `SoundType` and `PlaySound`

For one-shot SFX we use a message. The enum names the sound; the struct wraps it with a per-instance volume so the producer (gameplay) decides loudness and the consumer (audio) just plays it.

```rust
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
```

#### 4. The four systems

**Loading** — called during startup alongside sprite atlas loading:

```rust
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
```

**Start background music** — spawns a looping entity when the level begins:

```rust
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
```

**Stop background music** — finds the marked entity and despawns it:

```rust
pub fn stop_background_music(
    mut commands: Commands,
    music: Query<Entity, With<BackgroundMusic>>,
) {
    for entity in music.iter() {
        commands.entity(entity).despawn();
    }
}
```

**Play sound effects** — the consumer that reads `PlaySound` messages and spawns ephemeral audio entities:

```rust
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
```

Notice the pattern:
- The **music entity** persists across frames because it uses `LOOP`. We mark it with `BackgroundMusic` so `stop_background_music` can find it later. You could also use `GameEntity` here and let `cleanup_level` despawn it — the marker just makes the audio lifecycle explicit and easy to move if we add pause states later.
- The **SFX entities** are fire-and-forget. `PlaybackSettings::DESPAWN` means Bevy removes the entity after the clip finishes. We never have to clean them up manually.
- The **producer/consumer split** is the same pattern we used for tower placement in Part 18. Gameplay systems emit `PlaySound` messages; the audio system consumes them. The struct carries a per-instance `volume` so producers can attenuate individual sounds (the explosion is quieter than the laser) without the consumer needing to know why.

### Wiring the module into `main.rs`

Add `mod audio;` alongside the other modules, then import the four systems and the `PlaySound` message:

```rust
mod audio;

use audio::{load_audio_assets, start_background_music, stop_background_music, play_sound_effects, PlaySound, SoundType};
```

Register the message type:

```rust
// ... existing messages ...
.add_message::<PlaySound>()
```

Add `load_audio_assets` to the startup chain so handles are ready before the player reaches level select:

```rust
.add_systems(Startup, (load_tower_registry, load_audio_assets, spawn_camera, setup_tower_atlas).chain())
```

Start music when the level begins, and stop it when the level ends:

```rust
.add_systems(OnEnter(GameState::InGame), (
    // ... existing setup systems ...
    start_background_music,
).chain())

.add_systems(OnExit(GameState::InGame), (cleanup_level, stop_background_music))
```

Add the SFX consumer to `Update` so it runs every frame and catches any messages:

```rust
.add_systems(Update, (
    // ... existing systems ...
    play_sound_effects,
    // ...
).run_if(in_state(GameState::InGame)))
```

### Looping rocket thruster on the projectile

The rocket launch sound is different from the other two. It is **not** a one-shot — it should loop while the projectile is in flight and cut off the instant the rocket hits something.

Because Bevy audio is entity-based, the cleanest way to achieve this is to attach the audio components directly to the projectile entity. When `explode_projectiles` despawns the projectile, the sound stops automatically.

In `launch_rockets`, add the audio assets resource as a system parameter, then append the audio components to the projectile spawn bundle.

The system now queries:
- `audio_assets: Res<crate::audio::AudioAssets>` — to get the rocket launch handle.
- `Volume` is imported from `bevy::audio::Volume`.

In the `commands.spawn` call that creates the projectile, append:

```rust
AudioPlayer::new(audio_assets.rocket_launch.clone()),
PlaybackSettings::LOOP.with_volume(Volume::Linear(2.0)),
```

This is the key insight: **looping sounds can be components on gameplay entities**. We do not need a separate "stop sound" message or a lookup table mapping projectiles to audio sinks. The ECS handles both the visual and the auditory lifecycle of the rocket in one place.

### One-shot laser fire

In `attack_enemies`, add a `MessageWriter<PlaySound>` parameter. When `attacker.timer.just_finished()` fires, write the message before spawning the muzzle flash:

```rust
sounds.write(PlaySound { sound: SoundType::LaserFire, volume: 1.0 });
```

The `play_sound_effects` consumer will pick it up on the same frame and spawn a short-lived audio entity.

### One-shot rocket explosion

In `explode_projectiles`, add the same `MessageWriter<PlaySound>` parameter. At the top of the projectile loop, emit the explosion sound:

```rust
sounds.write(PlaySound { sound: SoundType::RocketExplosion, volume: 0.8 });
```
This runs before the damage and visual explosion spawn, so the sound and the sprite appear together.

> **Run the game now.** Select a level. You should hear background music start. Place a rapid tower — each shot should produce a short laser sound. Place a rocket launcher — the projectile should emit a continuous thruster noise that cuts off when it explodes. The explosion should play a crunch sound.

---

## Simplifications

- **No positional audio.** Our camera is fixed and the game world is small; left-right panning would not add meaningful information. A future part could add `SpatialListener` and pan sounds based on screen position.
- **No volume mixing groups.** Music and SFX share the same master volume. A real project would use `Volume::Decibel` and separate busses for music, SFX, and UI.
- **No pitch randomization.** Every laser sounds identical. Randomizing `PlaybackSettings::speed` slightly per shot would make rapid fire feel less mechanical.
- **Single music track.** We play one loop for all levels. A more advanced system might select a track per level or crossfade between ambient and combat music based on wave intensity.

---

## Summary

- Audio in Bevy is **entity-based**: `AudioPlayer` + `PlaybackSettings` components on an entity produce sound.
- **`PlaybackSettings::LOOP`** keeps a sound repeating until its entity is despawned.
- **`PlaybackSettings::DESPAWN`** plays a sound once and automatically cleans up the entity.
- We used **two different patterns**:
  - **Entity-coupled looping** for the rocket thruster (sound lives on the projectile entity).
  - **Decoupled messages** for one-shot SFX (gameplay emits `PlaySound` with a per-instance volume; a consumer spawns ephemeral audio entities).
- Background music is started in `OnEnter(InGame)` and stopped in `OnExit(InGame)` by despawning its marked entity.
- `Volume::Linear` attenuates sounds without editing source files.
