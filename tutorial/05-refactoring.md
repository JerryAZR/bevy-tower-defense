# Part 5: Refactoring — Splitting a Monolith into Modules

> **Time to read:** ~15 minutes  
> **New concepts:** Rust modules, visibility (`pub`), crate structure  
> **Prerequisite:** Part 4 (auto-tiling with `MapLayout`, `TileRules`, and `TilemapBundle` in a single `main.rs`)

---

## Recap: What We Already Have

Our auto-tiling system works: a `15×10` grid with a 3-tile-high road, resolved at spawn time by `TileRules`. But all the code lives in `main.rs` — `TileType`, `MapLayout`, `NeighborPattern`, `TileRules`, `build_rules()`, and the tilemap spawn loop. That is fine for a demo, but as we add level loading, enemy spawning, and tower placement, the file will become unmanageable.

---

## Goal: What We Will Build

We will split `main.rs` into three focused modules:

1. **`map.rs`** — logical grid data: `TileType`, `MapLayout`, marker components.
2. **`tiling.rs`** — auto-tiling rules and resolution.
3. **`main.rs`** — thin wiring layer: plugins, asset loading, and the tilemap spawn loop.

The goal is **organizational clarity**, not premature abstraction. Each file has a single responsibility, and `main.rs` becomes a short script that connects the pieces.

---

## New Bevy APIs & Concepts

### Rust Modules

Rust organizes code with the `mod` keyword. When you write `mod map;` in `main.rs`, the compiler looks for `src/map.rs` and makes its contents available as the `map` module. Items inside a module are **private by default**; you must add `pub` to expose them to other modules.

```rust
// In main.rs
mod map;              // "src/map.rs exists; load it as the `map` module"

use map::TileType;    // OK — TileType is `pub` in map.rs
```

### Visibility

| Keyword | Meaning |
|---|---|
| `fn foo()` | Private to this module. |
| `pub fn foo()` | Visible everywhere in the crate. |
| `pub(crate) fn foo()` | Visible everywhere in the crate (same as `pub` for single-crate projects). |
| `pub(super) fn foo()` | Visible to the parent module only. |

We use `pub` for types and methods that cross module boundaries. We keep helpers private because they are implementation details.

### Dependency Direction

A well-organized crate has a clear dependency graph. In our refactor:

- `tiling.rs` depends on `map.rs` (it needs `TileType` and `MapLayout`).
- `main.rs` depends on both `map.rs` and `tiling.rs` (it imports from both).

There is no `use crate::tiling` inside `map.rs` — the arrow only points one way. This prevents circular imports and makes the architecture easier to reason about.

**Pitfall:** It is tempting to make everything `pub` to avoid compiler errors. That works, but it also removes the guardrails that visibility provides. Start with the minimum visibility and widen it only when another module legitimately needs the item.

---

## Walkthrough

### Designing the split

Before moving code, we decide what belongs where. The boundary is already visible in Part 4: map data (what tiles are) and tiling logic (what tiles look like) are separate concerns.

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

### Step 1: Create `map.rs`

Create `src/map.rs` and move every type related to the logical grid:

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

### Step 2: Create `tiling.rs`

Create `src/tiling.rs` and move the auto-tiling engine:

```rust
use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;
use std::collections::HashMap;

use crate::map::{MapLayout, TileType};

// NeighborMatch, NeighborPattern, TileRule, TileTypeRuleset, TileRules ...
```

All the types from Part 4 (`NeighborMatch`, `NeighborPattern`, `TileRule`, `TileTypeRuleset`, `TileRules`) and the helpers (`pat`, `rule`, `build_rules`) move here unchanged.

**Key decisions:**

- **`use crate::map::{MapLayout, TileType};`** — `tiling.rs` depends on `map.rs`, not the other way around. `main.rs` also imports directly from `map.rs`, so the full graph is `main` → `map` and `main` → `tiling` → `map`.
- **`pub fn build_rules()`** — the only public *free* function. The methods on `TileRules` and `TileTypeRuleset` are also `pub` because `setup()` in `main.rs` calls `TileRules::resolve()`. Helpers `pat()` and `rule()` stay private because `build_rules()` is their sole caller.

---

### Step 3: Shrink `main.rs`

Replace the body of `main.rs` with module declarations and imports:

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
    // ... spawn tilemap exactly as in Part 4 ...
}
```

`main.rs` no longer contains:

- `TileType` definition (moved to `map.rs`)
- `NeighborPattern`, `TileRules` (moved to `tiling.rs`)
- `build_rules()`, `pat()`, `rule()` (moved to `tiling.rs`)

The spawn loop inside `setup()` is unchanged from Part 4. It still reads `tile_type` from `MapLayout`, calls `rules.resolve()`, and spawns a `TileBundle`.

---

### What we deliberately avoided

To keep the tutorial focused, we skipped these advanced patterns:

- **Plugins.** Bevy's `Plugin` trait lets you bundle systems and resources. For a two-module refactor, `add_systems(Startup, setup)` in `main()` is clearer than a `MapPlugin` and `TilingPlugin`.
- **Resource wrappers.** We could have wrapped `MapLayout` and `TileRules` in a `Level` resource. We will do that in Part 6 when level data arrives.
- **Events.** Dynamic tile updates (digging, building) would use Bevy events. Our maps are static, so events are unnecessary.

---

### Step 4: Verify

Run the compiler:

```bash
cargo check
```

Should produce no errors or warnings. Then:

```bash
cargo run
```

The output should be visually identical to Part 4: a centered `15×10` grid with a 3-tile-high horizontal road and correctly resolved auto-tiling. If anything looks different, the refactor introduced a bug — likely a missing import or a visibility issue.

---

## What stayed the same

- `build_demo_map()` is unchanged — it still produces the same logical grid.
- The auto-tiling rules and resolution logic are moved verbatim; no behavior changed.
- The tilemap spawn loop in `setup()` is identical to Part 4.

---

## What we gained

| Aspect | Before (Part 4) | After (Part 5) |
|---|---|---|
| File count | 1 (`main.rs`) | 3 (`main.rs`, `map.rs`, `tiling.rs`) |
| `main.rs` size | ~300 lines | ~60 lines |
| Map data | Mixed with rendering | Isolated in `map.rs` |
| Auto-tiling | Mixed with rendering | Isolated in `tiling.rs` |
| Adding a new system | Scroll through `main.rs` | Import the relevant module |

---

## Summary

- We extracted **map data** into `map.rs`: `TileType`, `MapLayout`, marker components.
- We extracted **auto-tiling logic** into `tiling.rs`: rules, patterns, `build_rules()`.
- We left `main.rs` as a thin **wiring layer**: plugins, asset loading, tilemap spawning.
- We used `pub` for cross-module types and kept helpers private.
- We preserved the one-way dependency: `tiling` depends on `map`, and `main` depends on both.

In **Part 6** we add `level.rs` to load map definitions from external TOML files, completing the separation of level design from game code.
