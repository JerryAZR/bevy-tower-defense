# Part 24: Input Abstraction — `GameAction` Events & Virtual Cursor

> **Time to read:** ~18 minutes
> **New concepts:** logical input events, virtual cursor resource, `CursorMoved` event, `MessageWriter` / `MessageReader` decoupling
> **Prerequisite:** Part 23 (level select buttons)

---

## Recap: What We Already Have

Seven systems spread across five files read raw device types directly — `ButtonInput<KeyCode>`, `MouseWheel`, `ButtonInput<MouseButton>`, and `Single<&Window>` for cursor position. Adding a gamepad (Part 25) would mean touching every one of these systems again. The input layer is welded to specific hardware.

---

## Goal: What We Will Build

Introduce a `GameAction` message for discrete actions (key presses, clicks, scroll) and a `VirtualCursorPos` resource for the shared cursor position (mouse, keyboard arrow, and eventually gamepad stick). Two new systems in `src/input.rs` form a **mouse compatibility layer**: `read_mouse_hover` updates the virtual cursor from `CursorMoved` events, and `read_mouse_click` emits `GameAction::Confirm` on left-click when the cursor is on a map tile.

By the end, no game-logic system touches a `KeyCode`, `MouseButton`, `MouseWheel`, or `Window` cursor query. Physical-input code lives in one file.

---

## New Bevy APIs & Concepts

### Logical input vs physical input

Physical input says *which device* and *which key*. Logical input says *what the player intended to do*. The translation happens once, in one place. Every system downstream only sees the logical layer.

If you're coming from Unity, this is the same idea as the **Input System** package (not the legacy Input Manager): define Action assets that map physical keys and gamepad buttons to named actions like "Jump" or "Fire", then subscribe to those actions in your MonoBehaviours. Our `GameAction` enum is Bevy's version of that Action map, and our reader systems are the binding layer.

### `CursorMoved` event

Bevy emits a `CursorMoved` event every frame the mouse moves (with `position: Vec2` and `delta: Option<Vec2>`). This is the clean way to detect mouse movement — no position-comparison hack needed. Because it's a message event, multiple readers can consume it without interfering with each other.

### Virtual cursor as a resource

`VirtualCursorPos(Option<[u32; 2]>)` is a plain `Resource` that holds the tile coordinate the player is currently pointing at — regardless of which device moved it. Mouse writes it via `CursorMoved`, keyboard writes it via arrow-key `GameAction` events. All cursor-reading systems need only `Res<VirtualCursorPos>`.

---

## Walkthrough

### Step 1: `VirtualCursorPos` — the shared cursor resource

We place it in `src/tower.rs` because it stores a tile coordinate — a
game-specific spatial concept — and `nudge_virtual_cursor`, which manipulates
it, lives in the same file.  `input.rs` only pushes raw mouse data into it,
the same way it writes `GameAction::Confirm` without owning `PlaceTower`.

```rust
/// Virtual cursor — the tile coordinate shared by mouse, keyboard, and
/// gamepad.  `read_mouse_hover` writes this every frame; gameplay systems
/// read it instead of querying the cursor directly.
#[derive(Resource, Default)]
pub struct VirtualCursorPos(pub Option<[u32; 2]>);
```

Seed it to the map centre when entering a level. In `spawn_placement_preview`, add a `map_layout` parameter and insert the resource:

```rust
commands.insert_resource(VirtualCursorPos(Some([
    map_layout.width / 2,
    map_layout.height / 2,
])));
```

This ensures the preview is visible immediately — no need to move the mouse first.

### Step 2: `read_mouse_hover` — the mouse compatibility layer

Add to `src/input.rs`. This system reads `CursorMoved` events (not raw cursor queries) and writes `VirtualCursorPos`:

```rust
fn read_mouse_hover(
    camera: Single<(&Camera, &GlobalTransform)>,
    map_layout: Res<MapLayout>,
    mut kb_pos: ResMut<VirtualCursorPos>,
    mut cursor_events: MessageReader<CursorMoved>,
) {
    // Only process the last cursor-move event of the frame.
    let Some(event) = cursor_events.read().last() else { return; };

    let (cam, cam_transform) = *camera;
    let Ok(world) = cam.viewport_to_world_2d(cam_transform, event.position) else { return; };
    if let Some(tile) = world_to_tile(world, map_layout.width, map_layout.height) {
        kb_pos.0 = Some(tile);
    }
}
```

Using `.last()` on the event iterator guarantees we react to the cursor's final position this frame. The system is gated by `.run_if(in_state(GameState::InGame))` in the plugin registration so it never tries to access `MapLayout` during the level select screen.

### Step 3: `read_mouse_click` → `GameAction::Confirm`

Also in `src/input.rs`. Left-click during `InGame` emits a `Confirm` action, but only when the cursor is on a map tile (so clicking empty space doesn't accidentally place a tower):

```rust
fn read_mouse_click(
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform)>,
    map_layout: Res<MapLayout>,
    mut actions: MessageWriter<GameAction>,
) {
    if !mouse.just_pressed(MouseButton::Left) { return; }

    let (cam, cam_transform) = *camera;
    let Some(cursor) = window.cursor_position() else { return; };
    let Ok(world) = cam.viewport_to_world_2d(cam_transform, cursor) else { return; };
    if world_to_tile(world, map_layout.width, map_layout.height).is_some() {
        actions.write(GameAction::Confirm);
    }
}
```

Both systems are added to `InputPlugin::build` with `in_state(GameState::InGame)` conditions:

```rust
.add_systems(Update, read_mouse_hover.run_if(in_state(GameState::InGame)))
.add_systems(Update, read_mouse_click.run_if(in_state(GameState::InGame)))
```

### Step 4: `GameAction` — the logical input vocabulary

Define the enum in `src/input.rs`. Variants capture intent, not hardware:

```rust
#[derive(Message)]
pub enum GameAction {
    Up, Down, Left, Right,
    SelectTower(usize), SelectLevel(usize),
    NextTower, PrevTower,
    Confirm, Cancel,
}
```

- `Up/Down/Left/Right` — arrow keys (keyboard), D-pad (gamepad later)
- `SelectTower(usize)` / `SelectLevel(usize)` — number-key shortcuts
- `NextTower` / `PrevTower` — scroll wheel
- `Confirm` — Enter, Space, mouse click, gamepad A
- `Cancel` — Escape, gamepad B

### Step 5: Keyboard and scroll → `GameAction`

`read_keyboard_for_actions` reads `ButtonInput<KeyCode>` and emits actions. Digit keys are context-dependent (select tower in `InGame`, select level in `LevelSelect`). `read_mouse_wheel` reads `MouseWheel` and emits `NextTower` / `PrevTower`.

The details are in `src/input.rs` — the key insight is that these two systems plus the two mouse systems form the **complete boundary** between hardware and game logic.

### Step 6: Refactor consumers

Each game-logic system that read raw input now reads `MessageReader<GameAction>` or `Res<VirtualCursorPos>` instead:

| System | Before | After |
|--------|--------|-------|
| `select_tower_by_key` | `ButtonInput<KeyCode>` | `MessageReader<GameAction>` (SelectTower, NextTower, PrevTower) |
| `navigate_level_select` | `ButtonInput<KeyCode>` | `MessageReader<GameAction>` (Up/Down/Left/Right, SelectLevel, Confirm) |
| `toggle_pause` | `ButtonInput<KeyCode>` | `MessageReader<GameAction>` (Cancel) |
| `handle_game_over_input` | `ButtonInput<KeyCode>` | `MessageReader<GameAction>` (Confirm, Cancel) |
| `update_placement_preview` | `Window` + `Camera` | `Res<VirtualCursorPos>` |
| `place_tower_on_click` | `MouseButton` + `Window` + `Camera` | `MessageReader<GameAction>` (Confirm) + `Res<VirtualCursorPos>` |
| `draw_tower_ranges` | `Window` + `Camera` | `Res<VirtualCursorPos>` |
| `cycle_tower_on_scroll` | `MouseWheel` | **deleted** — logic moved to `read_mouse_wheel` |
| `hovered_placeable_tile` | `Window` | **deleted** — replaced by `tile_is_placeable` helper |

### Step 7: `nudge_virtual_cursor` — keyboard cursor movement

A new system in `src/tower.rs` that reads arrow-key `GameAction` events and moves `VirtualCursorPos` one tile per press. Clamped to map bounds; tile validity is checked downstream by `update_placement_preview` and `place_tower_on_click` via `tile_is_placeable`.

```rust
pub fn nudge_virtual_cursor(
    mut actions: MessageReader<GameAction>,
    map_layout: Res<MapLayout>,
    mut cursor: ResMut<VirtualCursorPos>,
) {
    // ... match Up/Down/Left/Right, clamp to bounds ...
    cursor.0 = Some([nx, ny]);
}
```

This system belongs to the `GameplaySet::Interaction` set, so it only runs
when `game_is_running` — i.e. during `InGame` with `PauseState::Running`.

This system does **not** touch the preview sprites — `update_placement_preview` (which runs after) reads `VirtualCursorPos` and handles the visual follow-through.
### Step 8: `draw_cursor_highlight` — visual feedback

A new gizmo system that draws a translucent white rectangle on the tile under `VirtualCursorPos`. Separated from `draw_tower_ranges` because highlighting the cursor and drawing tower ranges are independent concerns.

> **Run the game now.** Arrow keys navigate the level grid. Arrow keys during a level move the preview — a white square highlights the cursor tile. The preview shows only on placeable tiles; hovering a path tile hides it. Mouse click or Enter places a tower at the cursor tile. Scroll cycles the tower dock. Escape pauses. Everything works — and no game-logic system touches raw input.

---

## Simplifications

- **Gamepad not yet wired.** `GameAction` has the right variants (D-pad → `Up/Down/Left/Right`, A → `Confirm`, B → `Cancel`), but the gamepad reader doesn't exist yet. That's Part 25.
- **`VirtualCursorPos` is tile-aligned.** Mouse positions are rounded to the nearest tile, so smooth sub-tile cursor movement is lost. Acceptable for a grid-based tower defense.

---

## Summary

- **`GameAction`** decouples intent from hardware — keyboard, mouse, and gamepad all emit the same variants.
- **`VirtualCursorPos`** is a shared resource for the cursor tile, written by mouse (`CursorMoved`) and keyboard (arrow-key actions).
- **`MessageWriter` / `MessageReader`** form the producer/consumer pipeline for actions.
- **Mouse compatibility layer** (`read_mouse_hover` + `read_mouse_click`) translates raw mouse input into the virtual cursor and confirm actions — gameplay systems never see a `MouseButton` or `Window`.
- **`nudge_virtual_cursor`** moves the shared cursor one tile per arrow press; `update_placement_preview` follows visually.
