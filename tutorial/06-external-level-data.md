# Part 6: External Level Data — Loading Maps from TOML Files

> **Time to read:** ~25 minutes  
> **New concepts:** `serde` deserialization, TOML data files, data-driven design  
> **Prerequisite:** Part 5 (modular codebase with `map.rs`, `tiling.rs`, and `main.rs`)

---

## Recap: What We Already Have

Our code is organized into three modules: `map.rs` holds the logical grid, `tiling.rs` holds the auto-tiling engine, and `main.rs` wires everything together. The map itself is still built by `build_demo_map()` — a Rust function that hardcodes a 3-tile-high horizontal road. That works for one level, but every change requires a recompile.

---

## Goal: What We Will Build

We will move map definitions out of Rust and into external **TOML files**:

1. Add `serde` and `toml` dependencies for deserialization.
2. Design a TOML format that describes map dimensions, path waypoints, and (optionally) waves.
3. Create a `level.rs` module that loads TOML into `LevelData`, then builds a `MapLayout` from it.
4. Replace `build_demo_map()` with `load_level("assets/levels/level_01.toml")`.
5. Add inner-corner auto-tiling rules so turns look correct on the new path shape.

The result: designers can create and edit levels without touching Rust, and changes take effect immediately without recompiling.

---

## New Bevy APIs & Concepts

### `serde` and `Deserialize`

`serde` is Rust's standard serialization framework. The `Deserialize` derive macro generates code that populates a struct from an external format (JSON, TOML, YAML, etc.). In Bevy games, this is the standard way to load configuration, level data, and save files.

```rust
#[derive(Deserialize)]
struct Config {
    health: u32,
}
```

`toml::from_str::<Config>("health = 100")` produces `Config { health: 100 }`.

### Data-Driven Design

**Data-driven design** means keeping game logic in code and game content in data files. The code decides *how* things work; the data decides *what* things exist. Our auto-tiling rules look data-like, but they are still hardcoded Rust. We *could* externalize them into a rules file too, but since our project only has one tileset and one set of rules, that would add complexity without benefit. The map geometry, on the other hand, changes per level, so moving it into TOML is a clear win.

**Pitfall:** It is tempting to make the data format expressive (conditions, loops, expressions). Resist. A level file should describe *what* is in the level, not *how* to build it. Keep the build logic in Rust where it can be tested, debugged, and version-controlled with the compiler.

---

## Walkthrough

### Step 1: Add dependencies

Add two crates to `Cargo.toml`:

```toml
serde = { version = "1", features = ["derive"] }
toml = "0.8"
```

- **`serde`** provides the `Deserialize` derive macro.
- **`toml`** parses TOML text into Rust structs.

**Why TOML?** JSON is harder for humans to read and (in standard form) does not support comments. YAML supports comments but is notorious for "indentation hell" — a single stray space can change the meaning of an entire subtree. TOML is explicit, flat, and comment-friendly. Its weakness is serialization: there is no single canonical way to serialize a struct back to TOML. That does not matter here because our game only *consumes* level data; it never writes it.

---

### Step 2: Design the TOML format

Create `assets/levels/level_01.toml`:

```toml
[map]
width = 15
height = 10

[paths.main_road]
waypoints = [
    [2, 9],
    [2, 5],
    [12, 5],
    [12, 1],
]
```

**Design decisions:**

| Decision | Rationale |
|---|---|
| **No explicit spawn/base list** | Spawn points and bases are inferred from path endpoints. The first waypoint is where enemies enter; the last is what they defend. |
| **No terrain grid** | Everything defaults to grass. Path tiles are generated from waypoints. This keeps level files short and lets designers think in paths, not pixels. |
| **`[paths.id]` instead of `[[paths]]`** | Table keys enforce uniqueness. Two `[paths.main_road]` sections are a parse error. This is safer than an array where duplicate IDs are silently allowed. |
| **Waypoints are width-1 centerline** | The game expands each segment to width-3 at load time. Designers edit a clean polyline; the game handles the visual bulk. |

---

### Step 3: Create Rust types

Before writing code, consider what the runtime actually needs. The TOML file stores waypoints as a width-1 centerline — easy for a designer to edit, but our renderer needs the full width-3 grid of `Path` tiles. And in Part 7, enemies will need those same waypoints to follow the path. So we need *both*: the raw waypoints for gameplay logic, and the expanded grid for rendering.

This means our `LevelData` struct mirrors the TOML shape closely (so we can deserialize directly), while a separate `build_map_from_level` function derives `MapLayout` from it. We keep the original data around as an ECS resource so future systems do not have to reverse-engineer waypoints from the tile grid.

Create `src/level.rs`:

```rust
use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

use crate::map::{MapLayout, TileType};

#[derive(Debug, Deserialize, Resource)]
pub struct LevelData {
    pub map: MapData,
    pub paths: HashMap<String, PathData>,
    #[serde(default)]
    #[allow(dead_code)]
    pub waves: Vec<WaveData>,
}

#[derive(Debug, Deserialize)]
pub struct MapData {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PathData {
    pub waypoints: Vec<[u32; 2]>,
}

#[derive(Debug, Deserialize)]
pub struct WaveData {
    // Reserved for Part 7
}
```

**`#[derive(Resource)]`** lets us insert `LevelData` into Bevy's ECS as a resource. Gameplay systems in future parts will query it for spawn points, wave timings, and path waypoints.

**`#[serde(default)]`** on `waves` makes the field optional in TOML. If omitted, it defaults to an empty vector. This is useful while we are still designing the wave format.

**`#[allow(dead_code)]` on `waves`** suppresses a compiler warning. The field is defined now so the TOML format is complete, but nothing reads `waves` yet. Part 7 will implement wave spawning, at which point this attribute is removed. Using `#[allow(dead_code)]` as a temporary scaffold is acceptable in tutorial-style iterative development, but in production code you would either implement the consumer immediately or omit the field until it is needed.

---

### Step 4: Load and build the map

#### `load_level`

```rust
pub fn load_level(path: &str) -> LevelData {
    let content = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e));
    toml::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {}", path, e))
}
```

Simple and strict: if the file is missing or malformed, the game panics immediately. In a production game you might return a `Result` and show an error screen, but for a tutorial, fail-fast is clearer.

#### `build_map_from_level`

This is where the width-3 expansion happens:

```rust
pub fn build_map_from_level(level: &LevelData) -> MapLayout {
    let width = level.map.width;
    let height = level.map.height;
    let mut tiles = vec![TileType::Grass; (width * height) as usize];

    for (id, path) in &level.paths {
        if path.waypoints.len() < 2 {
            panic!("Path '{}' must have at least 2 waypoints", id);
        }
        for window in path.waypoints.windows(2) {
            let [x1, y1] = window[0];
            let [x2, y2] = window[1];

            if x1 == x2 {
                // Vertical segment: center column + one on each side.
                let y_start = y1.min(y2);
                let y_end = y1.max(y2);
                for y in y_start..=y_end {
                    for dx in -1i32..=1i32 {
                        let nx = x1 as i32 + dx;
                        if nx >= 0 && nx < width as i32 {
                            let idx = (y * width + nx as u32) as usize;
                            tiles[idx] = TileType::Path;
                        }
                    }
                }
            } else if y1 == y2 {
                // Horizontal segment: center row + one above and below.
                let x_start = x1.min(x2);
                let x_end = x1.max(x2);
                for x in x_start..=x_end {
                    for dy in -1i32..=1i32 {
                        let ny = y1 as i32 + dy;
                        if ny >= 0 && ny < height as i32 {
                            let idx = (ny as u32 * width + x) as usize;
                            tiles[idx] = TileType::Path;
                        }
                    }
                }
            } else {
                panic!(
                    "Path '{}' has a diagonal segment: ({},{}) -> ({},{}). \
                     Only axis-aligned segments are supported.",
                    id, x1, y1, x2, y2
                );
            }
        }
    }

    // Fill diagonal tiles at path corners so turns form complete 3×3 blocks.
    for (_id, path) in &level.paths {
        let wps = &path.waypoints;
        for i in 1..wps.len() - 1 {
            let [x1, y1] = wps[i - 1];
            let [x2, y2] = wps[i];
            let [x3, y3] = wps[i + 1];

            let dx1 = x2 as i32 - x1 as i32;
            let dy1 = y2 as i32 - y1 as i32;
            let dx2 = x3 as i32 - x2 as i32;
            let dy2 = y3 as i32 - y2 as i32;

            // The horizontal segment determines the missing x offset,
            // and the vertical segment determines the missing y offset.
            let mx = if dy1 == 0 { dx1.signum() } else { -dx2.signum() };
            let my = if dx1 == 0 { dy1.signum() } else { -dy2.signum() };

            let cx = x2 as i32 + mx;
            let cy = y2 as i32 + my;
            if cx >= 0 && cx < width as i32 && cy >= 0 && cy < height as i32 {
                let idx = (cy as u32 * width + cx as u32) as usize;
                tiles[idx] = TileType::Path;
            }
        }
    }

    MapLayout { width, height, tiles }
}
```

**Why the diagonal fill is needed:** When two perpendicular width-3 segments meet at a corner, they form an L-shape, not a complete 3×3 block. The diagonal tile on the "outside" of the turn is covered by neither segment. For example, a vertical segment at x=2 and a horizontal segment at y=5 both cover (2,5), but the tile at (1,4) — one step west and one step south of the corner — is left as grass. The second loop computes this missing tile for each interior waypoint and marks it as path.

**Key behaviors:**

- **Axis-aligned only.** Diagonal segments panic. This keeps the expansion logic simple and avoids ambiguous corner cases.
- **Bounds-checked.** Tiles outside the map are silently skipped. This lets waypoints sit near the edge without crashing.
- **Overlapping segments are fine.** If two path segments overlap, the tile is simply marked `Path` twice.

---

### Step 5: Add inner corner rules

With the diagonal fill in place, turns now produce complete 3×3 path blocks. This creates a new visual case: a tile surrounded by path on all four sides but with grass on one diagonal. These **inner corners** need their own sprites so the path does not look like a flat rectangle at every turn.

Add four new rules to `build_rules()` in `src/tiling.rs`, checked before the outer corner rules:

```rust
// Inner corners
rule(pat(same, same, same, same, same, same, diff, same), 82),   // upper-left
rule(pat(same, same, same, same, same, same, same, diff), 83),   // upper-right
rule(pat(same, same, same, same, diff, same, same, same), 105),  // bottom-left
rule(pat(same, same, same, same, same, diff, same, same), 106),  // bottom-right
```

Each pattern requires `Same` on all four cardinals and `Same` on three diagonals, with exactly one diagonal as `Different`. This specificity ensures inner corners match before the more general body tile (fallback 103).

---

### Step 6: Wire in `main.rs`

```rust
mod level;
mod map;
mod tiling;

use level::{build_map_from_level, load_level};

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let level = load_level("assets/levels/level_01.toml");
    let map = build_map_from_level(&level);
    let rules = build_rules();

    // ... spawn tilemap ...

    commands.insert_resource(map);
    commands.insert_resource(rules);
    commands.insert_resource(level);
}
```

`build_demo_map()` is gone. The map now comes entirely from the TOML file. We also insert `level` as a resource so future systems (enemy spawning, wave management) can access path waypoints and spawn points.

---

### Step 7: Verify

```bash
cargo run
```

You should see:

- A `15×10` grid.
- A step-shaped road starting at the top-left (x=2, y=9), going down to y=5, across to x=12, then down to y=1.
- Correct auto-tiling on all corners and edges, resolved from the rule book.
- The same visual quality as Part 4, but the geometry now comes from `assets/levels/level_01.toml`.

Try editing the waypoints in the TOML file and re-running. No recompile needed.

---

## Simplifications and future work

| Simplification | Future extension |
|---|---|
| **Panic on parse errors** | Return `Result` and show a user-friendly error dialog or log. |
| **Hardcoded file path** | Accept a command-line argument or menu selection to choose the level. |
| **No wave data** | `WaveData` will gain fields: spawn time, enemy type count, path reference. |
| **Single path per level** | Multiple paths are already supported by the `HashMap`; enemies will reference them by ID. |
| **Grass-only default terrain** | Add optional terrain layers (water, rocks) to the TOML and `TileType` enum. |

---

## Summary

- We added `serde` and `toml` dependencies for deserialization.
- We designed a **TOML level format** with `[map]`, `[paths.id]`, and optional `waves`.
- We created `src/level.rs` with `LevelData`, `load_level`, and `build_map_from_level`.
- We **replaced `build_demo_map()`** with file-driven level loading.
- We inserted `LevelData` as an ECS resource for future gameplay systems.
- We added **inner corner rules** so turns render correctly on the new path shape.
- We verified that editing the TOML file changes the map without recompiling.

In **Part 7** we will add enemies that follow the path waypoints from spawn to base.
