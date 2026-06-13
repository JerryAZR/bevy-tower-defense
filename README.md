# Bevy Tower Defense

A tower defense game built from scratch with [Bevy](https://bevyengine.org/) 0.18,
accompanied by a 26-part tutorial that teaches Bevy through building a real project.

## Running

```bash
cargo run --release
```

All assets live in `assets/` — Bevy finds them automatically when run from the
project root. No extra setup needed.

## What's inside

- 7 hand-crafted levels with path-following enemies
- Two tower types: basic attacker and rocket launcher (area damage)
- Tower data loaded from TOML files — add new towers without touching code
- Placement preview with range gizmos
- Gold economy with passive income
- Level select screen with keyboard/gamepad grid navigation
- Pause, win/lose states, game-over screen
- Background music and sound effects
- Input abstraction — keyboard, mouse, and gamepad all emit the same `GameAction` events

## Tutorial

The `tutorial/` directory contains a 26-part walkthrough covering:

- Bevy ECS basics, plugins, resources, components, systems
- Tilemaps, map data, enemy path-following
- States, run conditions, system sets
- UI, level select, tower dock
- Custom events, message passing
- Audio (background music + SFX)
- Input abstraction (`GameAction`, `VirtualCursorPos`)
- Gamepad support (digital buttons + analog stick)
- Data-driven design (TOML tower/level definitions)
- Gizmos, projectile motion, area-damage attacks

Start at `tutorial/01-setup.md` and follow through in order — each part builds
on the previous one.

## Tech

- [Bevy](https://bevyengine.org/) 0.18 — game engine
- [bevy_ecs_tilemap](https://github.com/StarArawn/bevy_ecs_tilemap) 0.18 — tilemap rendering
- TOML — tower and level data files

## License

MIT — build whatever you want with this.
