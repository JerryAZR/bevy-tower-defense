# Part 23: Level Select Buttons — Grid UI & Focus Navigation

> **Time to read:** ~15 minutes
> **New concepts:** `Button` entities, `Interaction` states, grid layout with `FlexWrap`, keyboard focus navigation, 2D-grid index math
> **Prerequisite:** Part 22 (system sets — for navigating the current codebase)

---

## Recap: What We Already Have

The level select screen is a single `Text` node. You pick a level by pressing a digit key 1–9. There is no visual feedback for which level is highlighted, no arrow-key navigation, and no mouse-click interaction. It works, but it's a dead-end for gamepad support.

---

## Goal: What We Will Build

Replace the monolithic `Text` node with a grid of individual `Button` entities — one per available level. Keyboard arrow keys move a focus highlight through the grid. Enter/Space selects the focused level. Mouse clicks also select. Digit keys remain as shortcuts. When Part 25 adds gamepad input, the same focus model will work with a D-pad.

---

## New Bevy APIs & Concepts

### `Button` and `Interaction`

`Button` is a built-in marker component that tells Bevy's UI system to track interaction state. When you spawn an entity with `Button`, Bevy automatically adds `Interaction` to it, updating the value every frame:

| `Interaction` value | Trigger |
|---------------------|---------|
| `Interaction::None` | Cursor is not over the button |
| `Interaction::Hovered` | Cursor entered the button's rect |
| `Interaction::Pressed` | Mouse button down while over the button |

You can query `Changed<Interaction>` to detect transitions (a button was *just* pressed, not *is* pressed).

### `FlexWrap`

`FlexWrap::Wrap` on a parent `Node` causes overflowing children to wrap onto the next row — the same behaviour as CSS `flex-wrap: wrap`. Combined with a calculated fixed `width`, this produces a grid from a flat list of children.

### Grid navigation math

When buttons are laid out in `COLS` columns, moving focus with arrow keys requires mapping a flat index to a row/column:

```
row = index / COLS      (integer division)
col = index % COLS
```

Up moves `index - COLS` (if row > 0). Down moves `index + COLS` (capped at the last button). Left/Right move ±1 with boundary checks on the current row.

---

## Walkthrough

### Step 1: Design the grid

Before writing code, decide the layout:

- **3 columns** — enough to fill the screen horizontally without crowding.
- **Button size:** 200×60 px — wide enough for "Level_01", tall enough to click comfortably.
- **20 px gap** between buttons in both directions.
- **Parent width:** `3 × 200 + 2 × 20 = 640 px` — exactly enough for one row of 3 buttons.
- **`FlexWrap::Wrap`** on a Row container makes the 4th button flow to the next row automatically.

With 9 levels this produces a 3×3 grid centred on screen.

We also need two pieces of state that the old `Text`-based screen didn't:

- **`FocusedLevel(usize)` resource** — which button row the highlight is on. Shared by the keyboard navigation system and the visual-update system.
- **`LevelButton(usize)` component** — attached to each button entity so systems can tell which level index a button represents.

### Step 2: `setup_level_select` — spawn the grid

Open `src/level_select.rs`. At the top add layout constants and colours:

```rust
const COLS: usize = 3;
const BUTTON_W: f32 = 200.0;
const BUTTON_H: f32 = 60.0;
const GAP: f32 = 20.0;

const COLOR_DEFAULT: Color = Color::srgba(0.15, 0.15, 0.15, 1.0);
const COLOR_FOCUSED: Color = Color::srgba(0.1, 0.3, 0.5, 1.0);
const COLOR_HOVERED: Color = Color::srgba(0.2, 0.25, 0.35, 1.0);
const COLOR_PRESSED: Color = Color::srgba(0.05, 0.15, 0.3, 1.0);
```

Then add the resource and component types below the imports:

```rust
#[derive(Resource, Default)]
pub struct FocusedLevel(pub usize);

#[derive(Component)]
pub struct LevelButton(pub usize);
```

Now replace `setup_level_select`. Instead of building one `Text` string, spawn a nested container hierarchy:

> **Full-screen container** (centers everything) → **Grid container** (wraps children into rows) → **Button entities** (one per level) → **Text children**

The grid container's `width` is calculated from the constants so that exactly `COLS` buttons fit per row. The hierarchy:

```rust
commands
    // full-screen flex container (centers everything)
    .spawn((ScreenUi, Node { ... }))
    .with_children(|parent| {
        parent
            // grid container — flex-wrap forces row overflow
            .spawn(Node {
                flex_wrap: FlexWrap::Wrap,
                width: Val::Px(COLS as f32 * BUTTON_W + (COLS - 1) as f32 * GAP),
                column_gap: Val::Px(GAP),
                row_gap: Val::Px(GAP),
                ..default()
            })
            .with_children(|grid| {
                for (i, path) in levels.0.iter().enumerate() {
                    grid.spawn((Button, LevelButton(i), Node { ... }, ...))
                        .with_child((Text::new(name), ...));
                }
            });
    });
```

The full code is in `src/level_select.rs` — the button `Node` includes border, border-radius, and flex centering; the constants live at the top of the file.

Each button entity bundles `Button`, `LevelButton(i)`, `Node`, `BackgroundColor`, and `BorderColor`. Bevy's UI system reads `Button` and automatically injects `Interaction` at runtime — you never spawn `Interaction` directly.

### Step 3: Reset focus on entry

`scan_available_levels` already runs in `OnEnter(LevelSelect)`. Add a line to reset the focus to the top-left button:

```rust
// Reset navigation focus to the top
commands.insert_resource(FocusedLevel(0));
```

This ensures each visit to the level select screen starts with the first button highlighted, regardless of where the player left it last time. (If you preferred the focus to persist across visits, you could instead call `.init_resource::<FocusedLevel>()` once at app startup — the resource would survive screen transitions on its own.)

### Step 4: `navigate_level_select` — keyboard-driven focus

This system replaces `handle_level_select_input`. It reads arrow keys, digit keys, Enter, and Space — everything the old system read plus the four arrow directions.

The core navigation logic:

```rust
let row = focused.0 / COLS;
let col = focused.0 % COLS;
let total_rows = (len + COLS - 1) / COLS;

if keys.just_pressed(KeyCode::ArrowUp) && row > 0 {
    focused.0 = focused.0.saturating_sub(COLS);
}
if keys.just_pressed(KeyCode::ArrowDown) && row + 1 < total_rows {
    focused.0 = (focused.0 + COLS).min(len - 1);
}
if keys.just_pressed(KeyCode::ArrowLeft) && col > 0 {
    focused.0 -= 1;
}
if keys.just_pressed(KeyCode::ArrowRight) && col + 1 < COLS && focused.0 + 1 < len {
    focused.0 += 1;
}
```

Down-arrow uses `.min(len - 1)` to clamp on the last row (which may be shorter than `COLS`). Right-arrow checks both the column boundary *and* the total button count for the same reason.

After the arrow keys, handle confirm and digit shortcuts exactly as before — they write to `SelectedLevel` and set `NextState(InGame)`.

### Step 5: `update_level_select_visuals` — colour feedback

This system runs every frame while on the level select screen. It iterates every button and sets its `BackgroundColor`:

- **Focused button** → `COLOR_FOCUSED` (blue tint)
- **Other buttons** → `COLOR_HOVERED` / `COLOR_PRESSED` / `COLOR_DEFAULT` based on their `Interaction` state

```rust
pub fn update_level_select_visuals(
    focused: Res<FocusedLevel>,
    mut buttons: Query<(&LevelButton, &Interaction, &mut BackgroundColor)>,
) {
    for (btn, interaction, mut bg) in buttons.iter_mut() {
        *bg = if btn.0 == focused.0 {
            BackgroundColor(COLOR_FOCUSED)
        } else {
            BackgroundColor(match *interaction {
                Interaction::Hovered => COLOR_HOVERED,
                Interaction::Pressed => COLOR_PRESSED,
                Interaction::None => COLOR_DEFAULT,
            })
        };
    }
}
```

Focus takes priority: if the focused button is also hovered, it stays blue. This avoids confusing flickering between highlight and hover colours.

### Step 6: `handle_level_button_click` — mouse interaction

A small system that reacts to clicks on level buttons. It moves focus and confirms in one step — convenient for mouse users who don't want to click twice:

```rust
pub fn handle_level_button_click(
    levels: Res<AvailableLevels>,
    mut focused: ResMut<FocusedLevel>,
    mut selected: ResMut<SelectedLevel>,
    mut next_state: ResMut<NextState<GameState>>,
    buttons: Query<(&LevelButton, &Interaction), Changed<Interaction>>,
) {
    for (btn, interaction) in buttons.iter() {
        if *interaction == Interaction::Pressed {
            focused.0 = btn.0;
            selected.0 = levels.0[btn.0].clone();
            next_state.set(GameState::InGame);
            return;
        }
    }
}
```

`Changed<Interaction>` ensures this only runs when interaction state actually transitions — not every frame.

### Step 7: Wire everything in `main.rs`

In `src/main.rs`, update the level_select import to include the four new system names (and drop `handle_level_select_input`). Then update the LevelSelect section:

```rust
use level_select::{scan_available_levels, setup_level_select, navigate_level_select,
    update_level_select_visuals, handle_level_button_click};

// ...
        // ---------- LevelSelect ----------
        .add_systems(OnEnter(GameState::LevelSelect), (
            scan_available_levels,
            setup_level_select,
        ).chain())
        .add_systems(OnExit(GameState::LevelSelect), cleanup_screen_ui)
        .add_systems(Update, (
            navigate_level_select,
            update_level_select_visuals,
            handle_level_button_click,
        ).run_if(in_state(GameState::LevelSelect)))
```

The three Update systems are in a single parallel group — they don't depend on each other and can run simultaneously.

> **Run the game now.** The level select screen shows a 3×3 grid of styled buttons. Arrow keys move a blue highlight. Enter or Space starts the highlighted level. Clicking any button starts that level immediately. Digit keys 1–9 still work as shortcuts.

---

## Simplifications

- **Navigation is hardcoded key checks.** The arrow-key logic lives directly in `navigate_level_select` as `if keys.just_pressed(KeyCode::ArrowUp)`. Part 24 will extract this into a device-agnostic action layer.
- **No focus wrapping.** Arrow keys stop at grid boundaries — removing the column-index guards on Left/Right would let the flat index wrap across rows automatically. This is purely a preference; we stop at edges because it feels more deliberate.
- **Hardcoded column count.** `COLS` is a compile-time constant tied to the static `width: Val::Px(640.0)`. A responsive grid with percentage-based widths would compute columns at runtime from `ComputedNode` measurements.

---

## Summary

- **`Button`** entities get automatic `Interaction` tracking — `Hovered` / `Pressed` / `None` — with `Changed<Interaction>` for transition detection.
- **`FlexWrap::Wrap`** on a parent `Node` with a calculated `width` produces a responsive grid layout from a flat list of children.
- **2D grid navigation** maps a flat `usize` index to row/column via integer division and modulus — the same math works regardless of how many levels exist.
- **Focus + Interaction coexist.** The focused button gets a persistent highlight colour; other buttons follow their interaction state.
