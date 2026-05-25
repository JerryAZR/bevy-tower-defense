# Part 5: Refactoring — Splitting a Monolith into Modules

Our `main.rs` has grown to over 300 lines. It contains map data, auto-tiling logic, and rendering setup all in one file. This works for a demo, but as we add level loading, enemy spawning, and tower placement, the file will become unmanageable.

In this part we refactor into three focused modules. The goal is not premature abstraction — it is **organizational clarity**. Each module has a single responsibility, and `main.rs` becomes a thin wiring layer.

---

## Why modularize now?

A common mistake is to modularize too early, creating tiny files with one function each. Another mistake is to modularize too late, when untangling dependencies becomes painful.

We modularize now because:

1. **Natural boundaries have emerged.** Map types, tiling rules, and rendering setup are already separate concerns in our heads. The code reflects that.
2. **Part 6 adds level loading.** A `level.rs` module will need to import `MapLayout` and `TileRules`. Having them in separate modules makes that import obvious.
3. **Tutorial clarity.** Readers can study auto-tiling without scrolling past rendering code.

---

## The new structure

```
src/
├── main.rs   // Application wiring: plugins, systems, demo map builder
├── map.rs    // Map data: TileType, MapLayout, marker components
└── tiling.rs // Auto-tiling: rules, patterns, resolution
```

| Module | Responsibility | What it exports |
|---|---|---|
| `map` | Logical grid and tile types | `TileType`, `MapLayout`, `MapTile`, `PathTile` |
| `tiling` | Visual rules and resolution | `TileRules`, `NeighborPattern`, `build_rules` |
| `main` | ECS setup and application entry | `main()`, `setup()`, `build_demo_map()` |

---

## Module 1: `map.rs`

Everything about the logical grid lives here.

```rust
use bevy::prelude::*;

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum TileType {
    Grass,
    Path,
}

#[derive(Component)]
pub struct MapTile;

#[derive(Component)]
pub struct PathTile;

#[derive(Resource)]
pub struct MapLayout {
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<TileType>,
}

impl MapLayout {
    pub fn get(&self, x: u32, y: u32) -> Option<TileType> {
        if x < self.width && y < self.height {
            Some(self.tiles[(y * self.width + x) as usize])
        } else {
            None
        }
    }
}
```

**Key decisions:**

- **`pub` on almost everything.** These types are the shared vocabulary of the project. Making them `pub` avoids visibility friction. In a larger codebase you might use `pub(crate)` to restrict cross-crate visibility, but for a single-crate game the difference is negligible.
- **No Bevy plugins or systems.** `map.rs` is plain data. It does not know about rendering, tiling, or level loading.

---

## Module 2: `tiling.rs`

Everything about auto-tiling lives here.

```rust
use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;
use std::collections::HashMap;

use crate::map::{MapLayout, TileType};

// NeighborMatch, NeighborPattern, TileRule, TileTypeRuleset, TileRules ...
```

**Key decisions:**

- **`use crate::map::{MapLayout, TileType};`** — `tiling.rs` depends on `map.rs`, not the other way around. The dependency graph is a tree: `main` → `tiling` → `map`.
- **`pub fn build_rules()`** — the only public function. Helpers `pat()` and `rule()` are private because `build_rules()` is the sole caller.
- **`TileRules` methods are `pub`.** `setup()` in `main.rs` calls `TileRules::resolve()`.

---

## Module 3: `main.rs`

`main.rs` shrinks dramatically. Its only jobs are plugin setup, asset loading, and spawning the tilemap.

```rust
mod map;
mod tiling;

use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;

use map::{MapLayout, MapTile, PathTile, TileType};
use tiling::build_rules;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .add_plugins(TilemapPlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let map = build_demo_map();
    let rules = build_rules();
    // ... spawn tilemap ...
}
```

**What `main.rs` no longer contains:**

- `TileType` definition (moved to `map.rs`)
- `NeighborPattern`, `TileRules` (moved to `tiling.rs`)
- `build_rules()`, `pat()`, `rule()` (moved to `tiling.rs`)

---

## Visibility in Rust: a quick primer

If you are new to Rust modules, here is what matters for game development:

| Keyword | Meaning |
|---|---|
| `fn foo()` | Private to this module. |
| `pub fn foo()` | Visible everywhere in the crate. |
| `pub(crate) fn foo()` | Visible everywhere in the crate (same as `pub` for single-crate projects). |
| `pub(super) fn foo()` | Visible to the parent module only. |

We use `pub` for all types and methods that cross module boundaries. We keep helpers like `pat()` and `rule()` private because they are implementation details of `build_rules()`.

---

## What we did not do

To keep the tutorial focused, we deliberately avoided these advanced patterns:

- **Plugins.** Bevy's `Plugin` trait lets you bundle systems and resources. For a two-module refactor, `add_systems(Startup, setup)` in `main()` is clearer than a `MapPlugin` and `TilingPlugin`.
- **Resource wrappers.** We could have wrapped `MapLayout` and `TileRules` in a `Level` resource. We will do that in Part 6 when level data arrives.
- **Events.** Dynamic tile updates (digging, building) would use Bevy events. Our maps are static, so events are unnecessary.

---

## Verifying the refactor

```bash
cargo check
```

Should produce no errors or warnings. Then:

```bash
cargo run
```

The output should be visually identical to Part 4: a centered `15×10` grid with a 3-tile-high horizontal road and correctly resolved auto-tiling. If anything looks different, the refactor introduced a bug — likely a missing import or a visibility issue.

---

## Recap

In this part we:

1. Extracted **map data** into `map.rs`: `TileType`, `MapLayout`, marker components.
2. Extracted **auto-tiling logic** into `tiling.rs`: rules, patterns, `build_rules()`.
3. Left `main.rs` as a thin **wiring layer**: plugins, asset loading, tilemap spawning.
4. Used `pub` for cross-module types and kept helpers private.
5. Verified that the visual output is unchanged.

In **Part 6** we add `level.rs` to load map definitions from external TOML files, completing the separation of level design from game code.
