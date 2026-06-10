# Part 19: Gizmos — Tower Range Visualization

> **Time to read:** ~12 minutes
> **New concepts:** `Gizmos`, `circle_2d`, per-frame drawing
> **Prerequisite:** Part 18 (custom messages)

---

## Recap: What We Already Have

The player can select towers, place them on the map, and watch them attack enemies. Each tower has an `attack_range` in its definition, but the player has no visual indication of how far that range extends. A `rapid` tower reaches 192 pixels; a `big_rocket` reaches 256. Those numbers are invisible.

We also have a small problem in our data model. `PlacedTowers` is a `HashSet<[u32; 2]>` — it knows *which* tiles are occupied, but not *what* sits on them. That was fine when we only needed to answer "is this tile free?", but now we want to draw a range ring for the tower under the cursor. A `HashSet` forces us to iterate every tower and check distance, which is O(n) and scales poorly.

---

## Goal: What We Will Build

Two things:

1. **Refactor `PlacedTowers`** from `HashSet<[u32; 2]>` to `HashMap<[u32; 2], Entity>`. The spawn consumer records the tower entity it just created, and the gizmo system looks it up in O(1) via the tile coordinate.
2. **Draw attack-range circles** in two situations:
   - **Placement preview** — the ghost tower always shows a gold range ring so the player can evaluate coverage *before* spending gold.
   - **Hovered placed towers** — when the cursor is on a tile with a tower, a white range ring appears.

We do **not** draw every tower's range at once. With a dozen towers, overlapping circles would produce visual noise. Preview-always + hover-on-demand keeps the screen readable.

---

## New Bevy APIs & Concepts

### `Gizmos`

Bevy's `Gizmos` system param is a lightweight drawing API for debug and helper geometry. Unlike spawned sprite entities, gizmo shapes are **ephemeral**: you draw them once per frame, and they vanish on the next. If you want a circle to persist, your system must call `gizmos.circle_2d` every frame.

```rust
fn draw_helper(mut gizmos: Gizmos) {
    gizmos.circle_2d(Vec2::ZERO, 100.0, Color::srgba(1.0, 1.0, 1.0, 0.5));
}
```

Gizmos are rendered as wireframe lines by default, which is perfect for range indicators — they show boundaries without obscuring the sprites beneath.

> **Coming from Unity?** This is the direct equivalent of `OnDrawGizmos` in a `MonoBehaviour`: shapes drawn every frame for debugging and visualization. The concept is identical — call drawing methods inside a system, and the shapes appear for one frame only.

> **Pitfall:** Forgetting that gizmos are per-frame. If you draw a circle in `OnEnter` and never again, it flashes for one frame and disappears.

### `circle_2d`

The method signature is:

```rust
gizmos.circle_2d(position: Vec2, radius: f32, color: Color)
```

`position` is the world-space center. `radius` is in world units (pixels, in our 1:1 orthographic camera). `color` supports alpha, so we can make the rings semi-transparent.

The `Gizmos` API also provides `line_2d`, `rect_2d`, `arc_2d`, and 3D variants like `sphere` and `arrow` — all with the same ephemeral, per-frame semantics.

---

## Walkthrough

### Refactoring `PlacedTowers`

Our old `PlacedTowers` only answered one question: *is this tile occupied?*

```rust
pub struct PlacedTowers(pub HashSet<[u32; 2]>);
```

To find the tower under the cursor, we'd have to iterate every placed tower and check distance. With a `HashMap<tile, Entity>`, we convert the cursor to a tile coordinate and look up the entity directly — O(1) instead of O(n).

In `src/tower.rs`, change the definition:

```rust
pub struct PlacedTowers(pub HashMap<[u32; 2], Entity>);
```

Update the occupancy check in `hovered_placeable_tile`:

```rust
if is_grass && !placed.0.contains_key(&tile) {
    // ...
}
```

Now the spawn consumer must record the entity it creates. Both `spawn_instant_tower` and `spawn_rocket_launcher` need to return the turret `Entity`:

```rust
fn spawn_instant_tower(
    commands: &mut Commands,
    // ...
) -> Entity {
    commands.spawn(( /* base sprite */ ));
    commands.spawn(( /* turret */ )).id()  // return the turret entity
}

fn spawn_rocket_launcher(
    commands: &mut Commands,
    // ...
) -> Entity {
    commands.spawn(( /* base sprite */ ));
    let turret_entity = commands.spawn(( /* turret */ )).id();
    // ... ammo setup ...
    turret_entity  // return the turret entity
}
```

Then `spawn_tower_from_event` captures that entity and inserts it into the map:

```rust
pub fn spawn_tower_from_event(
    mut events: MessageReader<PlaceTower>,
    mut commands: Commands,
    atlas: Res<TowerAtlas>,
    registry: Res<TowerRegistry>,
    mut placed: ResMut<PlacedTowers>,  // NEW
) {
    // ... read event, assert single message ...

    let tower_entity = if def.damage.is_some() {
        spawn_instant_tower(&mut commands, &atlas, def, event.tower_id, event.world_pos)
    } else {
        spawn_rocket_launcher(&mut commands, &atlas, def, event.tower_id, event.world_pos)
    };
    placed.0.insert(event.tile, tower_entity);
}
```

The bookkeeping consumer in `src/economy.rs` no longer needs `PlacedTowers` — tile tracking moved to the spawn system where the entity is born:

```rust
use crate::tower::PlaceTower;  // PlacedTowers removed

/// Deducts gold when a tower placement message is received.
pub fn deduct_gold_on_placement(
    mut events: MessageReader<PlaceTower>,
    mut gold: ResMut<Gold>,
    // mut placed removed — spawn system now handles this
) {
    // ... assert single message ...
    gold.0 -= event.cost as f32;
    // placed.0.insert removed — handled by spawn_tower_from_event
}
```
### The gizmo system

`draw_tower_ranges` does two things each frame:

1. **Hover detection for placed towers.** Convert the cursor from screen space to world space, then to a tile coordinate via `world_to_tile`. Look up that tile in `PlacedTowers`. If there is a tower there, retrieve its `Transform` and `TowerAttacker` via the entity and draw a white range ring. This is O(1) — no iteration over all towers.

2. **Preview range.** Check whether the placement preview is visible (it is hidden when the cursor leaves the map). If visible, look up the selected tower's `attack_range` from the registry and draw a gold ring at the preview's position.

To implement this, the system needs:

- `Gizmos` — the drawing API to call `circle_2d`.
- `Window` and `Camera` — to convert the screen-space cursor position to world-space coordinates.
- `PlacedTowers` — the tile→entity map, so we can look up the tower on the hovered tile in O(1).
- `Query<(&Transform, &TowerAttacker)>` — to read position and range from the specific tower entity found in the map.
- `Query<(&Transform, &Visibility), With<TowerPreview>>` — to get the ghost tower's position, and to skip drawing when it is hidden.
- `SelectedTowerType` and `TowerRegistry` — to know which tower is selected and what its `attack_range` is.
- `MapLayout` — needed by `world_to_tile` to convert the cursor's world position to a tile coordinate.

In `src/tower.rs`:

```rust
/// Draws attack-range circles for the placement preview and for any placed
/// tower currently under the mouse cursor.
pub fn draw_tower_ranges(
    mut gizmos: Gizmos,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform)>,
    placed: Res<PlacedTowers>,
    towers: Query<(&Transform, &TowerAttacker)>,
    preview: Query<(&Transform, &Visibility), With<TowerPreview>>,
    selected: Res<SelectedTowerType>,
    registry: Res<TowerRegistry>,
    map_layout: Res<MapLayout>,
) {
    let (cam, cam_transform) = *camera;
    let cursor_world = window
        .cursor_position()
        .and_then(|cursor| cam.viewport_to_world_2d(cam_transform, cursor).ok());

    // Draw a range ring for the placed tower on the tile under the cursor.
    if let Some(cursor_pos) = cursor_world {
        if let Some(tile) = world_to_tile(cursor_pos, map_layout.width, map_layout.height) {
            if let Some(&entity) = placed.0.get(&tile) {
                if let Ok((transform, attacker)) = towers.get(entity) {
                    let tower_pos = transform.translation.truncate();
                    gizmos.circle_2d(
                        tower_pos,
                        attacker.range,
                        Color::srgba(1.0, 1.0, 1.0, 0.5),
                    );
                }
            }
        }
    }

    // Draw a range ring for the placement preview only when it is visible.
    for (preview_transform, visibility) in preview.iter() {
        if *visibility != Visibility::Visible {
            continue;
        }
        let def = registry.towers.get(selected.0)
            .expect("Selected tower index must be in registry");
        let preview_pos = preview_transform.translation.truncate();
        gizmos.circle_2d(
            preview_pos,
            def.attack_range,
            Color::srgba(1.0, 0.84, 0.0, 0.4),
        );
    }
}
```

Register `draw_tower_ranges` in `Update` under `in_state(GameState::InGame)`:

```rust
.add_systems(Update, (
    update_placement_preview,
    place_tower_on_click,
    spawn_tower_from_event.after(place_tower_on_click),
    deduct_gold_on_placement.after(place_tower_on_click),
    despawn_timed,
    update_gold_hud,
    tick_placement_denied,
    draw_tower_ranges,
).run_if(in_state(GameState::InGame)))
```

> **Run the game now.** Move the mouse over the map — the ghost tower should trail a faint gold circle. Place a tower, then hover over its tile — a white circle appears showing its exact reach. Move the cursor to an empty tile and the ring vanishes.

---

## Simplifications

- **Tile-based hover.** We trigger the range ring when the cursor is on the tower's tile, not by checking distance to the tower's center. Since our game is built on a grid and every tower occupies exactly one tile, this is both simpler and precise enough.
- **No range tinting by tower type.** All hovered towers show white. A future part could color-code ranges (rapid = blue, rocket = red) to make tactical differences immediately obvious.

---

## Summary

- We refactored `PlacedTowers` from `HashSet<[u32; 2]>` to **`HashMap<[u32; 2], Entity>`** so the gizmo system can look up towers in O(1) via tile coordinate.
- The **spawn consumer** now records the entity it creates, because it is the system that owns the relationship between *tile* and *entity*.
- The **bookkeeping consumer** (`deduct_gold_on_placement`) shrank — it only handles gold now.
- We used Bevy's `Gizmos` system param to draw **per-frame wireframe circles** with `circle_2d`.
- **Preview range** is drawn always (gold, `alpha = 0.4`) when visible.
- **Placed tower ranges** are drawn on tile-hover only (white, `alpha = 0.5`).
- Gizmos are **fire-and-forget**: they must be redrawn every frame. This makes them ideal for transient visual feedback, but unsuitable for persistent geometry.
