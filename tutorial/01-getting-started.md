# Part 1: Getting Started — From `cargo init` to a Red Square

This guide walks you through creating a brand-new Bevy project from scratch and getting your first sprite on screen. We focus on *why* each step matters, not just *what* to type.

---

## Prerequisites

- **Rust** installed via [rustup](https://rustup.rs/). Bevy 0.18 requires a recent stable compiler (1.85+).
- A GPU with Vulkan (Linux/Windows) or Metal (macOS) support. Bevy can fall back to software rendering in some cases, but a real GPU makes development far smoother.

---

## Step 1: Create the Rust Project

```bash
cargo init --name bevy_tower_defense
```

`cargo init` creates a binary crate — a program with a `main()` function — in the current directory. We name it explicitly so the executable and the crate identity are clear. If you skip `--name`, Cargo uses the folder name, which is fine too.

After this, you have:

- `Cargo.toml` — the manifest that declares dependencies, features, and build settings.
- `src/main.rs` — the entry point where execution begins.
- A hidden `.git/` repository (unless you passed `--vcs none`).

---

## Step 2: Add Bevy

```bash
cargo add bevy
```

This edits `Cargo.toml` for you, pulling in the latest published version of Bevy from [crates.io](https://crates.io/crates/bevy). Bevy is a large, modular engine. By default, `cargo add bevy` enables a sensible set of features — rendering, audio, windowing, asset loading, and more. You could trim this later (e.g., disabling audio or 3D rendering if you only need 2D), but for learning, the defaults are perfect.

**What just happened under the hood?**
Bevy is split into many internal crates (`bevy_ecs`, `bevy_render`, `bevy_winit`, etc.). The top-level `bevy` crate re-exports them under a unified API and provides `DefaultPlugins`, which wires everything together.

---

## Step 3: The Anatomy of a Bevy App

Open `src/main.rs`. Replace the default `Hello, world!` with the smallest possible Bevy program:

```rust
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .run();
}
```

Let’s unpack this line by line.

### `use bevy::prelude::*`

Bevy’s prelude is a curated set of the most commonly used types. Importing it with `*` keeps beginner code uncluttered. In production code you may prefer explicit imports, but the prelude is idiomatic for Bevy projects.

### `App::new()`

`App` is the container that holds your entire game. It owns:

- The **ECS world** (all entities, components, and resources).
- The **schedule** (all systems and when they run).
- The **plugin registry**.

Nothing runs until you call `.run()`.

### `.add_plugins(DefaultPlugins)`

A *plugin* is a bundle of setup logic. `DefaultPlugins` is actually a plugin *group* that registers roughly two dozen plugins for you, including:

| Plugin (internal) | What it gives you |
|---|---|
| `WindowPlugin` | Opens an OS window and handles resize/close events. |
| `WinitPlugin` | Bridges Bevy to the OS event loop (via `winit`). |
| `RenderPlugin` | Sets up the GPU render graph. |
| `AssetPlugin` | Enables loading images, sounds, fonts, etc. |
| `InputPlugin` | Keyboard, mouse, and gamepad input. |
| `TransformPlugin` | Position, rotation, and scale for entities. |

Without `DefaultPlugins`, the program would exit immediately because there is no window and no event loop.

### `.run()`

This starts the engine loop. It never returns (unless all windows close or a panic occurs). The loop, simplified, looks like this:

1. Poll OS events (window messages, input).
2. Run scheduled systems (your game logic).
3. Submit draw commands to the GPU.
4. Present the frame to the screen.
5. Repeat.

---

## Step 4: Spawn a Camera

A window alone is just a black rectangle. To see anything, Bevy needs a **camera** — an entity that defines the viewpoint into the scene.

```rust
fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}
```

Register this system in `main()`:

```rust
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .run();
}
```

### Why `Commands`?

Bevy uses an **Entity-Component-System (ECS)** architecture. The game world is a database of entities, and each entity is a collection of components (plain data). Systems are functions that run every frame and operate on that data.

`Commands` is a special parameter that lets a system queue structural changes to the world — like spawning or despawning entities — in a way that is safe and efficient. We mark it `mut` because spawning changes internal state.

### Why `Startup`?

`add_systems(Startup, setup)` tells Bevy to run `setup` exactly once, after all plugins have finished initializing but before the main game loop begins. It is the idiomatic place to create initial entities like cameras, the player, the map, and UI root nodes.

### What is `Camera2d`?

`Camera2d` is a *bundle* — a collection of components pre-configured for a 2D orthographic camera. It includes:

- `Camera` — the core render target configuration.
- `Camera2d` marker — tells the render graph to use 2D pipelines.
- `Transform` — where the camera is in world space.
- `GlobalTransform` — the computed world matrix.
- `Visibility` — whether the camera is active.

By default, a `Camera2d` looks at the world origin `(0, 0, 0)` and uses a coordinate system where **one unit equals one pixel** at zoom level 1. Positive Y is up.

---

## Step 5: Spawn a Sprite

With a camera in place, we can add something visible:

```rust
fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    commands.spawn(Sprite::from_color(
        Color::srgb(1.0, 0.0, 0.0),
        Vec2::new(100.0, 100.0),
    ));
}
```

### `Sprite::from_color`

This is a convenience constructor that creates a sprite without needing an image asset. It builds a default-white texture in memory and tints it with the color you provide. The `Vec2` sets the width and height in world units (pixels, in our default setup).

`Color::srgb(1.0, 0.0, 0.0)` is pure red. Bevy uses physically based color APIs: `srgb` constructs a color in the standard RGB color space, which is what monitors expect.

### Where does it appear?

Sprites spawn at the world origin `(0, 0, 0)` by default. Because our `Camera2d` is also centered on the origin, the red square appears in the middle of the window.

---

## Step 6: Run It

```bash
cargo run
```

The first build takes a while (Bevy compiles many dependencies, including the Vulkan/metal GPU abstraction layer `wgpu`). Subsequent builds are much faster thanks to incremental compilation.

You should see:

- A window titled `bevy_tower_defense`.
- A black background.
- A red square in the center.

Close the window (or press `Alt+F4`) to exit.

---

## What If Something Goes Wrong?

| Symptom | Likely Cause |
|---|---|
| Window opens but stays black | You forgot to spawn a `Camera2d`. Without a camera, the renderer has no target to draw into. |
| `cargo run` panics with a render error | Your GPU/driver may not support the required backend. Updating GPU drivers usually fixes this. |
| Compile errors about `Camera2d` or `Sprite` | Check that `use bevy::prelude::*;` is present and that `cargo add bevy` succeeded (check `Cargo.toml`). |
| Extremely long first compile | Normal. Bevy is a large dependency. Use `cargo run` (debug) while developing; release builds are slower to compile but faster to run. |

---

## Recap

You now have a minimal Bevy app that:

1. Creates an `App` and registers `DefaultPlugins`.
2. Runs a `Startup` system to bootstrap the world.
3. Spawns a 2D camera so the scene is visible.
4. Spawns a colored sprite at the world origin.

In the next part, we’ll build on this foundation by drawing a grid-based map and introducing the core gameplay loop of a tower defense game.
