# Part 1: Getting Started — From `cargo init` to a Red Square

> **Time to read:** ~15 minutes  
> **New concepts:** `App`, `Plugin`, `DefaultPlugins`, `Schedule` (`Startup`), `Commands`, `Camera2d`, `Sprite`, `Vec2`
> **Prerequisite:** A working Rust installation (1.85+)

---

## Goal: What We Will Build

In this part we will create a brand-new Bevy project from scratch and get our first sprite on screen — a red square in the center of a window. Along the way we will learn the anatomy of a Bevy app, how plugins boot the engine, and how to spawn entities using Bevy's ECS architecture.

This is the foundation everything else rests on. Every part that follows assumes you can open a window and draw something inside it.

---

## New Bevy APIs & Concepts

Before we write any code, here are the concepts you will encounter in this part.

### `App`

`App` is the container that holds your entire game. It owns the ECS world (all your game data), the schedule (when systems run), and the plugin registry. You create one with `App::new()`, configure it, and then call `.run()` to start the engine. Nothing happens until `.run()` is invoked.

### `Plugin`

A *plugin* is a bundle of setup logic. When you add a plugin to an `App`, it can register systems, load resources, or configure render pipelines. Plugins are Bevy's way of keeping code modular: the window system, the renderer, and your own game logic can each live in their own plugin.

### `DefaultPlugins`

`DefaultPlugins` is a plugin *group* — a collection of plugins that bootstraps the engine. It registers roughly two dozen internal plugins for you, including:

| Plugin (internal) | What it gives you |
|---|---|
| `WindowPlugin` | Opens an OS window and handles resize/close events. |
| `RenderPlugin` | Sets up the GPU render graph. |
| `AssetPlugin` | Enables loading images, sounds, fonts, etc. |
| `InputPlugin` | Keyboard, mouse, and gamepad input. |
| `TransformPlugin` | Position, rotation, and scale for entities. |

Without `DefaultPlugins`, the program would exit immediately because there is no window and no event loop.

### `Commands`

Bevy uses an **Entity-Component-System (ECS)** architecture. The game world is a database of entities, and each entity is a collection of *components* (plain data). *Systems* are functions that run every frame and operate on that data.

`Commands` is a special parameter that lets a system queue structural changes to the world — like spawning or despawning entities — in a way that is safe and efficient. We mark it `mut` because spawning changes internal state.

### `Schedule` and `Startup`

A *schedule* is a named collection of systems that run at a specific point in time. Bevy provides built-in schedules like `Startup` and `Update`. You attach systems to a schedule with `add_systems`.

`Startup` is the schedule that runs exactly once, after all plugins have finished initializing but before the main game loop begins. It is the idiomatic place to create initial entities like cameras, the player, the map, and UI root nodes.
`Update`, which we will meet in later parts, runs every frame.

### `Camera2d`

`Camera2d` is a *bundle* — a collection of components pre-configured for a 2D orthographic camera. It includes a `Camera` component, a `Transform`, and a few others. By default it looks at the world origin `(0, 0, 0)` and uses a coordinate system where **one unit equals one pixel** at zoom level 1. Positive Y is up.

**Pitfall:** Forgetting to spawn a camera leaves the window black, because the renderer has no target to draw into.

### `Sprite` and `Vec2`

A `Sprite` is a visible 2D object. `Sprite::from_color` is a convenience constructor that creates a sprite without needing an image asset — it builds a default-white texture in memory and tints it with the color you provide.

`Vec2` is a 2D vector type from Bevy's math library (`glam`). We use it to set the width and height of the sprite in world units. In our default setup, one world unit equals one pixel.

---

## Walkthrough

### Step 1: Create the Rust Project

First, create a new binary crate in the current directory. We name it explicitly so the executable and crate identity are clear, but the folder name works too if you skip `--name`.

```bash
cargo init --name bevy_tower_defense
```

This gives you three things:

- `Cargo.toml` — the manifest that declares dependencies, features, and build settings.
- `src/main.rs` — the entry point where execution begins.
- A hidden `.git/` repository (unless you passed `--vcs none`).

### Step 2: Add Bevy

Add Bevy as a dependency. This edits `Cargo.toml` for you, pulling in the latest published version from [crates.io](https://crates.io/crates/bevy).

```bash
cargo add bevy
```

Bevy is a large, modular engine. By default, `cargo add bevy` enables a sensible set of features — rendering, audio, windowing, asset loading, and more. You could trim this later, but for learning, the defaults are perfect.

Bevy is split into many internal crates (`bevy_ecs`, `bevy_render`, `bevy_winit`, etc.). The top-level `bevy` crate re-exports them under a unified API. The prelude — which we will import with `use bevy::prelude::*` — is a curated set of the most commonly used types. Using the prelude keeps beginner code uncluttered; explicit imports are fine too once you know what you need.

### Step 3: The Smallest Bevy App

Open `src/main.rs` and replace the default `Hello, world!` with the smallest possible Bevy program. We need an `App`, we need to register `DefaultPlugins` so the window and renderer exist, and we need to call `.run()` to start the loop.

```rust
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .run();
}
```

Calling `.run()` starts the engine loop. It never returns (unless all windows close or a panic occurs). The loop, simplified, looks like this:

1. Poll OS events (window messages, input).
2. Run scheduled systems (your game logic).
3. Submit draw commands to the GPU.
4. Present the frame to the screen.
5. Repeat.

At this point you can run `cargo run` and see an empty black window. It is not very exciting, but it proves the engine is alive.

### Step 4: Spawn a Camera

A window alone is just a black rectangle. To see anything, Bevy needs a **camera** — an entity that defines the viewpoint into the scene.

We will write a `setup` function and tell Bevy to run it once at `Startup`. Inside the function we use `Commands` to queue the creation of a `Camera2d` entity:

```rust
fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}
```

Now register this system in `main()`. The `add_systems` call attaches `setup` to the `Startup` schedule:

```rust
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .run();
}
```

If you run the app now, the window is still black — but this time the renderer has a camera. There is simply nothing in the scene to draw yet.

### Step 5: Spawn a Sprite

With a camera in place, we can add something visible. We will spawn a `Sprite` using `Sprite::from_color`, which needs two things: a `Color` and a size (`Vec2`).

`Color::srgb(1.0, 0.0, 0.0)` is pure red. Bevy uses physically based color APIs: `srgb` constructs a color in the standard RGB color space, which is what monitors expect. `Vec2::new(100.0, 100.0)` sets the width and height to 100 world units each — in our default setup, that means 100 pixels.

Add the sprite spawn inside the same `setup` function:

```rust
fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    commands.spawn(Sprite::from_color(
        Color::srgb(1.0, 0.0, 0.0),
        Vec2::new(100.0, 100.0),
    ));
}
```

Sprites spawn at the world origin `(0, 0, 0)` by default. Because our `Camera2d` is also centered on the origin, the red square appears in the middle of the window.

### Step 6: Run It

```bash
cargo run
```

The first build takes a while — Bevy compiles many dependencies, including the Vulkan/Metal GPU abstraction layer (`wgpu`). Subsequent builds are much faster thanks to incremental compilation.

You should see:

- A window titled `bevy_tower_defense`.
- A black background.
- A red square in the center.

Close the window (or press `Alt+F4`) to exit.

### What If Something Goes Wrong?

| Symptom | Likely Cause |
|---|---|
| Window opens but stays black | You forgot to spawn a `Camera2d`. Without a camera, the renderer has no target to draw into. |
| `cargo run` panics with a render error | Your GPU/driver may not support the required backend. Updating GPU drivers usually fixes this. |
| Compile errors about `Camera2d` or `Sprite` | Check that `use bevy::prelude::*;` is present and that `cargo add bevy` succeeded (check `Cargo.toml`). |
| Extremely long first compile | Normal. Bevy is a large dependency. Use `cargo run` (debug) while developing; release builds are slower to compile but faster to run. |

---

## Summary

You now have a minimal Bevy app that:

1. Creates an `App` and registers `DefaultPlugins`.
2. Runs a `Startup` system to bootstrap the world.
3. Uses `Commands` to spawn a 2D camera so the scene is visible.
4. Spawns a colored `Sprite` at the world origin using `Vec2` for size.

In the next part, we'll build on this foundation by drawing a grid-based map and introducing the core gameplay loop of a tower defense game.
