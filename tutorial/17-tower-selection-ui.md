# Part 17: Tower Selection UI — Scroll, Keys, and a Dynamic Dock

> **Time to read:** ~10 minutes
> **New concepts:** `Node`, `ImageNode`, `Interaction`, `BorderColor`, `MessageReader<MouseWheel>`
> **Prerequisite:** Part 16 (data-driven towers)

---

## Recap: What We Already Have

All tower stats live in `assets/towers.toml`. The game loads them into a `TowerRegistry` and spawns towers based on a `SelectedTowerType` resource. Right now that resource is hardcoded to index `0`, so the player can only place the first tower in the registry.

---

## Goal: What We Will Build

A **tower selection dock** at the bottom of the screen that displays every tower defined in `towers.toml`. The player can switch towers three ways:

- **Scroll wheel** cycles forward/backward through the list.
- **Number keys** `1`–`5` jump directly to a tower.
- **Clicking a dock slot** selects that tower.

The dock shows each tower's preview sprite, name, cost, and key number. The selected slot gets a gold border. The placement preview on the map updates immediately when the selection changes.

At the end of this part we'll add a **third tower** purely by editing `assets/towers.toml` — no Rust code changes — to prove the data-driven registry actually works.
---

## New Bevy APIs & Concepts

### `Node` — layout-driven UI positioning

`Node` is a Bevy component that turns an entity into a UI element positioned by a flexbox layout engine rather than world coordinates. You specify `width`, `height`, `position_type`, `justify_content`, and `align_items` — the engine handles the rest. This is how we'll build the dock: a horizontal row of slots centered at the bottom of the screen.

**Pitfall:** UI entities without a `Node` component won't participate in layout and may render at unexpected positions or not at all.

### `ImageNode` — sprites inside UI

`ImageNode::from_atlas_image(...)` is the UI equivalent of `Sprite::from_atlas_image(...)`. It renders a texture atlas sprite inside a `Node` layout. We use it to show tower preview images inside each dock slot.

### `Interaction` — click detection on UI

Bevy attaches an `Interaction` component to UI entities with a `Node`. It cycles through `Interaction::None`, `Interaction::Hovered`, and `Interaction::Pressed`. We query for `Changed<Interaction>` to detect clicks without polling every frame.

### `BorderColor` — visual selection state

`BorderColor` tints the border of a `Node`. In Bevy 0.18 it has four fields (`top`, `right`, `bottom`, `left`). Setting all four to the same color highlights a slot. Changing the border color at runtime is how we show which tower is selected.

**Pitfall:** `BorderColor` has no effect unless the `Node` also sets a `border` width (e.g., `border: UiRect::all(Val::Px(2.0))`).

### `MessageReader<T>` — reading input events

Bevy 0.18 uses `MessageReader<T>` (not `EventReader<T>`) for pull-based input events. `MessageReader::read()` returns an iterator of events that occurred since the last read. For scroll input we only care about the first event each frame — one step per tick keeps the control deliberate.

---

## Walkthrough

### Designing the dock

Before writing code, think about what the player should see:

1. **A horizontal strip** at the bottom center of the screen.
2. **One slot per tower** — the number of slots matches `registry.towers.len()`.
3. **Each slot shows**: a small preview sprite, the tower name, its cost in green, and a key number badge.
4. **Gold border** around the currently selected slot; gray border on the rest.
5. **Scroll down** moves to the next tower; **scroll up** moves to the previous. Selection clamps at the ends — no wrap-around.
6. **Number keys** jump directly. Pressing `2` selects the second tower if it exists.
7. **Clicking a slot** also selects that tower.
8. **Placement preview updates** immediately when selection changes, even if the cursor is not over the map.

From this we derive two components:

- `TowerDock` — a tag on the root container so we can despawn the entire dock during cleanup.
- `TowerDockSlot(usize)` — stores the tower index this slot represents. The `usize` matches the index into `TowerRegistry.towers`.

`SelectedTowerType` already exists, but we'll change it from a placeholder to a real resource the player controls.

### The dock root and slots

In `src/tower.rs`, add the components and constants:

```rust
#[derive(Component)]
pub(crate) struct TowerDock;

#[derive(Component)]
pub(crate) struct TowerDockSlot(pub usize);

const DOCK_SLOT_SIZE: f32 = 80.0;
const DOCK_SLOT_GAP: f32 = 8.0;
const DOCK_BG: Color = Color::srgba(0.12, 0.12, 0.12, 0.9);
const DOCK_BORDER_DEFAULT: Color = Color::srgba(0.3, 0.3, 0.3, 1.0);
const DOCK_BORDER_SELECTED: Color = Color::srgba(1.0, 0.84, 0.0, 1.0);  // gold
```

`setup_tower_dock` spawns the root `Node` and iterates over `registry.towers` to create one slot per tower. Each slot is a `Node` with vertical flex layout (`flex_direction: FlexDirection::Column`) containing four children: a key-number badge, an `ImageNode` preview, a name `Text`, and a cost `Text`.

The slot itself carries `TowerDockSlot(i)` and `Interaction::None` so Bevy will track hover/press state. Every slot spawns with `BorderColor::all(DOCK_BORDER_DEFAULT)` — the actual highlight is handled by a separate system so `setup_tower_dock` doesn't need to know which tower is selected.

See `pub fn setup_tower_dock` in `src/tower.rs` for the full implementation.

Register `setup_tower_dock` in the `OnEnter(GameState::InGame)` chain in `src/main.rs`. Run the game: you should see a dark dock at the bottom with two slots. All borders are gray because the highlight system isn't registered yet.
### From string keys to integer indices

In Part 16, `TowerRegistry` stored towers in a `HashMap<String, TowerDefinition>` keyed by names like `"rocket"`. `SelectedTowerType` held a `String`, and `TowerTypeId` on each spawned tower also held a `String`. Every lookup required a HashMap search: `registry.towers.get("rocket")`.

That design made sense when there was only one tower type and no UI. Once we add selection cycling and number-key shortcuts, the string key becomes friction: scrolling means computing the "next" key in a sorted list, and number keys map naturally to positions, not names.

We refactored three things:

1. **`TowerRegistry` is now a `Vec<TowerDefinition>`** — no `HashMap`, no `sorted_keys` helper. The vector index *is* the canonical tower ID.
2. **`SelectedTowerType(pub usize)`** — the player's selection is just a number. Default is `0` (the first tower in the vector).
3. **`TowerTypeId(pub usize)`** — every spawned tower stores the index it was created from.

Lookups are now direct indexing: `registry.towers.get(selected.0)` instead of `registry.towers.get(&tower_id.0)`. No hashing, no string allocation, and scroll/key math becomes simple integer arithmetic.

The TOML file still uses string keys (`[towers.rocket]`) for human readability — `serde` reads them into a `HashMap` during deserialization, and `load_tower_registry` converts to a `Vec` at runtime via `raw.towers.into_values().map(...).collect()`. The order is arbitrary (driven by HashMap iteration), but once the vector exists the indices are stable for the rest of the session.

> **When to refactor vs. patch** — We chose to refactor here because the string-ID design was only one part old and had not spread everywhere yet. If the same string key had been baked into save files, networked replays, and external tooling, a refactor might cost more than it saves. Sometimes the right call is to wrap the old abstraction (`fn tower_key_to_index(&str) -> usize`) and move on. Whether to pay the refactor tax is a judgment call that depends on schedule, blast radius, and how long you expect the code to live.

> **Run the game now.** There is no new feature yet, but the refactor should not break anything. You should still see the two towers in the registry working exactly as before — only the internal lookup mechanism changed.

### Reacting to selection changes

`update_dock_selection` watches `SelectedTowerType` and updates every slot's border color. It queries:

```rust
selected: Res<SelectedTowerType>,
mut slots: Query<(&TowerDockSlot, &mut BorderColor)>,
```

The `if selected.is_changed()` guard ensures we only iterate slots when the player actually switched towers, not every frame. For each slot we compare `slot.0 == selected.0` and set all four `BorderColor` fields to either gold or gray.

> **Why `is_changed()`?** `Res<T>` triggers change detection when it is mutably accessed, even if the value is identical. `ResMut` writes happen in `cycle_tower_on_scroll` and the other input systems; `update_dock_selection` reads and reacts.

> **Pitfall:** `is_changed()` only fires when the resource is *mutated in the current frame*. If the player exits a level and re-enters, the dock entities are despawned and respawned, but `SelectedTowerType` still holds the old value. The highlight system sees no change and leaves all borders gray. The fix is to reset the selection in `setup_tower_dock`: `selected.0 = 0;`. This mutates the resource on level entry, guaranteeing `is_changed()` is true for the first frame of the new dock.

Register `update_dock_selection` in `Update` with the `in_state(GameState::InGame)` condition. Run the game: the first slot should have a gold border — `setup_tower_dock` resets `selected.0 = 0` on level entry, which triggers `is_changed()` and causes `update_dock_selection` to highlight it.

### Input: scroll wheel

`cycle_tower_on_scroll` reads scroll events via `MessageReader<MouseWheel>`. Bevy 0.18 delivers scroll events as messages; `scroll.read().next()` gives us the first event of the frame. We ignore any additional events — one step per tick keeps the control deliberate. If two events happen to arrive in the same frame (for example, because the frame rate dipped), we still only advance by one tower.

The system queries:

```rust
mut scroll: MessageReader<MouseWheel>,
registry: Res<TowerRegistry>,
mut selected: ResMut<SelectedTowerType>,
```

Scroll down (`ev.y < 0`) increments the index; scroll up (`ev.y > 0`) decrements. Both directions clamp at the boundaries — no wrap-around. This is a deliberate choice: with a small tower count, wrapping feels unnecessary, and the player can always use number keys to jump to the ends.

```rust
let len = registry.towers.len();
if len == 0 { return; }
let Some(ev) = scroll.read().next() else { return; };

if ev.y < 0.0 && selected.0 + 1 < len {
    selected.0 += 1;
} else if ev.y > 0.0 && selected.0 > 0 {
    selected.0 -= 1;
}
```

### Input: number keys

`select_tower_by_key` maps `KeyCode::Digit1` through `KeyCode::Digit5` to indices `0` through `4`. It bounds-checks against `registry.towers.len()` before assigning, so pressing `3` when only two towers exist does nothing.

```rust
for (i, keycode) in keycodes.iter().enumerate() {
    if i < registry.towers.len() && kb.just_pressed(*keycode) {
        selected.0 = i;
    }
}
```

### Input: clicking a dock slot

`handle_dock_slot_click` queries slots whose `Interaction` changed this frame:

```rust
slots: Query<(&TowerDockSlot, &Interaction), Changed<Interaction>>,
mut selected: ResMut<SelectedTowerType>,
```

When `Interaction::Pressed` is detected, it sets `selected.0 = slot.0`. Because `update_dock_selection` reacts to the resource change, the border highlight updates automatically on the same frame.
Register `cycle_tower_on_scroll`, `select_tower_by_key`, and `handle_dock_slot_click` in `Update` under the same `InGame` condition. Run the game: all three input methods should work. The gold border should follow your selection.

### Updating the placement preview on selection change

The placement preview system from Part 16 needs a small addition. Previously it only updated position and tint based on hover state. Now it must also swap the preview sprite when `SelectedTowerType` changes.

We spawn two preview entities at different z-positions — `z = 2.0` for the base sprite and `z = 2.1` for the top preview sprite. In `update_placement_preview` we collect them into a vector, assert there are exactly two, and sort by z to tell them apart:

```rust
let mut previews: Vec<_> = preview_q.iter_mut().collect();
assert_eq!(previews.len(), 2, "placement preview must have exactly 2 entities");
previews.sort_by(|a, b| a.0.translation.z.total_cmp(&b.0.translation.z));

// Lower z = base sprite, higher z = top preview sprite.
previews[0].2.texture_atlas.as_mut().unwrap().index = def.base_sprite;
previews[1].2.texture_atlas.as_mut().unwrap().index = def.preview_top_sprite;
```

The `assert_eq!` is a safety net: if someone adds a third preview entity later, the system panics immediately with a clear message rather than silently sorting the wrong entity to the front.

### Organizing the systems

Our `Update` block has grown. For readability, group related systems together — input systems near each other, visual updates near each other. If Bevy's tuple arity limit becomes an issue, split into two `add_systems(Update, ...)` calls with the same `.run_if(in_state(GameState::InGame))` condition:

```rust
.add_systems(Update, (
    cycle_tower_on_scroll,
    select_tower_by_key,
    handle_dock_slot_click,
).run_if(in_state(GameState::InGame)))
.add_systems(Update, (
    update_placement_preview,
    update_dock_selection,
    update_gold_hud,
    tick_placement_denied,
    place_tower_on_click,
    despawn_timed,
).run_if(in_state(GameState::InGame)))
```

This is purely organizational — no behavior changes.

Now that the dock works for the existing two towers, let's prove the data-driven design actually scales. Open `assets/towers.toml` and append a new tower and its projectile:

```toml
[towers.big_rocket]
name = "Big Rocket"
description = "Slow, devastating rocket with long reload."
base_sprite = 183
top_sprite = 229
preview_top_sprite = 206
cost = 200
attack_range = 256.0
attack_cooldown = 1.5
projectile = "big_rocket"
ammo_slot_offsets = [[0.0, 8.0]]
ammo_refill_secs = 4.0

[projectiles.big_rocket]
damage = 150.0
speed = 400.0
sprite = 252
explosion_sprite = 21
splash_radius = 100.0
```

Notice what is *not* here: no new Rust file, no new system, no new match arm. The tower uses the same `spawn_rocket_launcher` path as the regular rocket launcher because both lack a `damage` field and both reference a projectile. The only difference is the data: one slot instead of three, higher damage, slower refill, larger splash.

No rebuild is required — the TOML is read at startup. Restart the game and the dock automatically shows three slots.

> **Simplification:** We're reusing the rocket-launcher code path for every tower that has `projectile` but no `damage`. In a larger game you'd use an explicit `archetype` enum rather than inferring the behavior from field presence.
---

## Simplifications

- **No wrap-around** — selection clamps at the first and last tower. With a small tower count, wrapping feels unnecessary; number keys provide fast end-to-end navigation.
- **One scroll step per frame** — only the first `MouseWheel` event each frame is processed. A fast scroll still moves one tower at a time, keeping the control deliberate.
- **No tooltips** — the `description` field in `TowerDefinition` is still unused. A tooltip would require a hover-tracking system and a floating `Node` panel.
- **Fixed dock position** — the dock is always at the bottom center. A draggable or collapsible dock would need additional input handling and layout state.
- **Click only, no drag-and-drop** — the player clicks the map to place the selected tower, not the tower icon itself. Drag-and-drop would require tracking pointer offsets and validating drop targets.

---

## Summary

- We built a **dynamic tower dock** that reads `TowerRegistry` and creates one slot per tower automatically.
- `TowerDock` and `TowerDockSlot(usize)` are the minimal components needed for a flexbox-based UI dock.
- **Three input methods** — scroll wheel, number keys, and click — all mutate `SelectedTowerType`, and `update_dock_selection` reacts to highlight the correct slot.
- `MessageReader<MouseWheel>` reads scroll events in Bevy 0.18; we consume only the first event per frame for predictable stepping.
- `BorderColor` + `border` width on `Node` provides the selection highlight. `ImageNode` renders atlas sprites inside UI layouts.
- The placement preview updates its sprite indices by sorting the two preview entities by z-position — no extra tag components needed.

---

> **Next:** In **Part 18** we introduce **custom events** (`EventWriter<T>` / `EventReader<T>`) and refactor tower placement so that the click handler emits a `PlaceTower` event instead of spawning directly. Multiple independent systems then react to that event — one spawns the entity, another deducts gold — demonstrating how events decouple systems that should not know about each other.

