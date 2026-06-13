# Part 26: Epilogue — Uncovered Concepts & Packaging

> **Time to read:** ~10 minutes  
> **New concepts:** release builds, packaging, a tour of uncovered Bevy APIs  
> **Prerequisite:** Part 25 (gamepad)

---

## Recap: What We Have

A complete tower defense game with ECS architecture, tilemaps, multiple levels, a level select screen, tower placement with previews, range gizmos, gold economy, projectile attacks, rocket launchers, pause, audio, keyboard/mouse/gamepad input, and a custom run condition. Across twenty-five parts we introduced Bevy APIs and gamedev patterns as the project needed them.

---

## Goal: What We Will Do

First, a tour of Bevy concepts we skipped — what they are, why we didn't need them, and when to use them. Second, discuss packaging and build the game for release.

---

## Uncovered Bevy Concepts

### Sprite animation

Animating a sprite means swapping the texture atlas index over time — like flipping through a flipbook. A `Timer` tracks the frame duration, and each tick advances the index. Combine with a `TextureAtlas` and a `Sprite` component.

We didn't need this because all our sprites (towers, rockets, enemies) use single-frame textures — no frame sequences to animate. In a polished game, you'd use this for muzzle flashes, explosions, or animated enemies.

### Spatial audio

Bevy's `bevy_audio` supports `SpatialListener` and `SpatialAudioBundle` for 3D-positioned sound in a stereo mix. Sounds pan left/right based on the listener's position and fade with distance.

We played all sounds at full volume from the "center" because we have no player entity to serve as the listener. In a game with a scrolling camera or a moving player character, you'd attach a `SpatialListener` to the camera or player entity and spawn sounds with `SpatialAudioBundle`.

### Custom bundles

A bundle is a collection of components you spawn together — a template for an entity. We wrote inline tuple bundles like `(Sprite, Transform, Visibility)`. For reusable entity archetypes (e.g., every tower shares ~6 components), derive a `Bundle`:

```rust
#[derive(Bundle)]
struct TowerBundle {
    sprite: Sprite,
    transform: Transform,
    // ...
}
```

Keeps spawning code DRY when the same component combination appears in multiple places.

### Scene files

Bevy can serialize/deserialize entire entity hierarchies via `DynamicScene` and `.scn` files. Useful for saving and loading game state, or defining entity prefabs that can be spawned on demand — similar to Unity prefabs.

We loaded levels from TOML files instead — simpler for a grid-based game, and easier to hand-edit. Scenes become valuable when entities have complex nested hierarchies or when you want save-game functionality.

### Observers

Bevy 0.15+ introduced `Observer` — a reactive alternative to `MessageReader<M>`. Instead of iterating events each frame, you register a system that fires *when* a specific event occurs. Observers can be attached to entities or global.

We used `MessageReader` because that's the event API we introduced back in Part 18 and carried through the project. Both are valid; `Observer` would work equally well for our `GameAction` pipeline. The choice is a matter of style — pull (drain events each frame) vs push (react to each event as it arrives).

### Reflection

Bevy's reflection system (`Reflect`, `TypeRegistry`) enables runtime type inspection — serialization, cloning, property editing. Inspectors and editor tools build on it.

We used explicit Rust types and serde for TOML. Reflection matters for editor workflows; for a hand-coded tutorial game, explicit types are clearer.

---

## Packaging

### Window icon

Bevy 0.18 does not have a stable, cross-platform API for setting the taskbar icon. The `Window` component has no `icon` field, and the underlying `WinitWindows` is stored in a temporary `thread_local!` `RefCell` — not as an accessible `NonSend` resource.

Future Bevy releases aim to promote it to a proper resource. Until then, there is no standard, recommended way to set the icon at the Bevy application level. If you really need it today, search the Bevy Discord for community workarounds using `winit` directly — but be aware they rely on internal APIs that may break on upgrade.

### Executable / file icon

The icon you see in Explorer or Finder is a build-time concern, not a Bevy concern:

- **Windows** — embed an `.ico` via a `winres` build script before linking.
- **macOS** — bundle an `.icns` in the `.app/Contents/Resources/` and set it in `Info.plist`.
- **Linux** — provide a `.desktop` entry with an `Icon=` path pointing to a PNG.

These are all handled outside of Rust code. Tools like `cargo-bundle` and `cargo-packager` automate the per-platform steps.

### Release build

Building for distribution:

```bash
cargo build --release
```

The binary lands at `target/release/bevy_tower_defense(.exe)`. Run it from the project root (so Bevy finds `assets/` in the current directory), or copy the `assets/` folder next to the binary for distribution.

For a self-contained single file, the `bevy_embedded_assets` feature embeds assets into the binary via a build script, but that's more advanced.

On Windows, add `#![windows_subsystem = "windows"]` at the top of `main.rs` to hide the console window in release builds.

> **Run the release build.** Launch time is faster, frame rate is higher, and the binary is smaller than the debug build. Keep the `assets/` folder next to it.

---

## What We Built

Over 26 parts, we built a tower defense game from scratch in Bevy:

- A tile-based map with path-following enemies and a base to defend
- Tower placement with preview, range gizmos, and gold costs
- Two tower types: basic attacker and rocket launcher (area damage)
- Tower data loaded from TOML files — data-driven design
- Seven levels with increasing difficulty
- A level select screen with grid navigation
- Win/lose states, game-over screen, and pause
- Background music and sound effects
- Keyboard, mouse, and gamepad input — all abstracted behind `GameAction` events
- Every game-logic system reads from the same device-agnostic API

More importantly, we learned how Bevy structures a real project: plugins, states, run conditions, system sets, message-passing, resource management, and the separation of input from game logic.

---

## Summary

- **Uncovered concepts** — sprite animation, spatial audio, bundles, scenes, observers, reflection. Know they exist; reach for them when you need them.
- **Window icon** — Bevy 0.18 has no stable API; taskbar icons require community workarounds, file icons are a build-time concern outside Bevy's scope.
- **Release build** — `cargo build --release` produces an optimized binary; keep `assets/` next to it.
- **That's a wrap.** Twenty-six parts, one game, and a solid Bevy foundation. Dig deeper into what we covered — and what we didn't — by building something fun, weird, or ambitious of your own. Most importantly: have fun building games.
