# Part 14: Gold Economy — The Resource Loop

In Part 13 we wired up three levels with auto-discovery.  But towers were free —
you could paint them across every grass tile.  That's a sandbox, not a strategy
game.

This part adds the core tower-defense resource loop: **earn gold → spend gold
on towers → kill enemies to earn more gold**.

---

## What we will build

- **`Gold(f32)` resource** — the player's current balance.
- **In-game HUD** — "Gold: 300" in the top-left corner, updated every frame.
- **Three income sources:**
  1. Starting gold (300) — enough for 3 towers.
  2. Passive income (3 gold/sec) — slowly trickles in during gameplay.
  3. Kill bounties (25 gold per enemy, defined per-type in the level TOML).
- **Placement cost** — 100 gold per tower (hardcoded for now).
- **Visual feedback** — preview turns green when affordable, red when placement
  is denied (not enough gold).

---

## Simplifications

- **Tower cost is hardcoded** (`pub const TOWER_COST: u32 = 100`).  In a
  multiple-tower-type game you'd add a `cost` field to a tower definition and
  look it up at placement time.
- **No gold cap.**  The player can stockpile arbitrarily.  A production game
  might add one as a balancing knob.
- **Flat bounty across types** (25 for all enemies in our levels).  The
  infrastructure supports per-type bounties via the TOML — we just set them all
  to the same value for now.

---

## Walkthrough

### 1. New module: `src/economy.rs`

We add a new file that holds everything gold-related.  This keeps the economy
self-contained — other modules only import the types and systems they need.

**Constants** define the starting gold (300), passive income rate (3/sec),
tower cost (100), and the duration of the red denied-flash (0.3 s).  These
are `pub` so other modules can reference them where needed.

**`Gold(f32)`** is the central resource.  We use `f32` instead of `u32`
because passive income adds a fraction every fixed timestep (`rate × dt`).
With `u32` we'd need a second accumulator variable to avoid losing the
fractional part.  The HUD floors it for display (`gold.0 as u32`).

**`Bounty(u32)`** is a component attached to each enemy at spawn.  The tower
attack system reads it when the enemy dies and adds the value to `Gold`.  It
could have been stored in a lookup table (enemy type → bounty) and checked
on kill, but carrying it on the entity itself is simpler and avoids an extra
query.

**`GoldHud`** is a marker component on the HUD text entity — it lets
`update_gold_hud` find exactly that one text node among all entities.

**`PlacementDenied(Timer)`** is a transient component.  When the player clicks
but can't afford a tower, we insert it onto the two preview sprites.  The
preview system sees it and tints red.  `tick_placement_denied` ticks the
timer and removes the component when it expires, returning the preview to
normal.  This is cleaner than a boolean flag because the component *is* the
state — no separate tracking needed.

**Four systems:**

- `spawn_gold_hud` — runs on `OnEnter(GameState::InGame)`, spawns a `"Gold: 300"`
  text node positioned absolute top-left.  The entity also gets `GameEntity`
  so it is cleaned up automatically when the level ends, plus a `GoldHud`
  marker so `update_gold_hud` can find it.
- `update_gold_hud` — runs every frame in `Update`, reads `Gold` and writes
  the floored value into the text component.
- `earn_passive_income` — runs in `FixedUpdate`, adds `PASSIVE_INCOME_RATE × dt`
  to `Gold`.
- `tick_placement_denied` — runs in `Update`, ticks the timer on any entity
  with `PlacementDenied` and removes the component when it expires.

### 2. Data: add `bounty` to `EnemyTypeDef`

In `level.rs`, we add a `bounty: u32` field to `EnemyTypeDef` with
`#[serde(default)]`, making it optional in TOML files.  If omitted, it
defaults to 0.

**A note on defensive programming:** `#[serde(default)]` is a silent fallback.
If you forget to add `bounty = 25` to your level config, enemies will drop 0
gold and you'll wonder why your economy feels broken.  In a production project
you might prefer to *not* use `default` — let the deserializer fail loudly
with a clear error like `missing field 'bounty'`.  We use `default` here to
demonstrate the pattern, but use it carefully in your own projects.

### 3. Enemy spawning: carry bounty through to the entity

The value flows through three places in `enemy.rs`:

**`SpawnEvent`** gets a new `bounty: u32` field.  This is our snapshot —
everything needed to spawn one enemy is packed into this struct when the
schedule is built at level load time.

**`build_spawn_schedule`** reads `def.bounty` and copies it into each event.

**`spawn_wave_enemies`** attaches `Bounty(event.bounty)` as a component on
the enemy entity, alongside `Health`, `Enemy`, and the others.  Now when the
attack system despawns the entity, it can read the bounty first.

### 4. Tower systems: gold check, denied flash, and kill bounty

Three systems in `tower.rs` are affected.

**`update_placement_preview`** changes its query to include `&mut Sprite` (so
we can tint the preview) and `Option<&PlacementDenied>` (so we know if the
red flash is active).  It also takes a `Res<Gold>` parameter.  The logic
becomes:

- If `PlacementDenied` is present → tint red (the player just tried and failed).
- Else if `gold.0 >= TOWER_COST as f32` → tint green (affordable).
- Else → tint white/neutral (valid tile, not enough gold).

`Option<&PlacementDenied>` in a Bevy query means "this component may or may
not exist."  When it's `Some`, the flash is active; when `None`, we fall
through to the affordability check.  No separate resource needed.

**`place_tower_on_click`** now takes `ResMut<Gold>` and a query over preview
entities.  After confirming the tile is valid, it checks affordability.  If
the player can't pay, it inserts `PlacementDenied` on the preview sprites
(triggering the red flash) and returns early.  Otherwise it deducts the cost
and spawns the tower.

**`attack_enemies`** adds `&Bounty` to its enemy query and `ResMut<Gold>`
as a parameter.  When an enemy's health drops to ≤ 0, we add `bounty.0 as f32`
to `gold.0` before despawning.  The cast from `u32` to `f32` is lossless for
practical gold values (under 16 million).

### 5. Level lifecycle: insert and clean up Gold

`load_level_data` in `gameplay.rs` inserts `Gold(STARTING_GOLD)` alongside
`BaseLives` and the other per-level resources.  `cleanup_level` in `state.rs`
removes it.  While `insert_resource` would overwrite the value on the next
level anyway, explicitly removing it makes the intent clear: each level gets
a fresh balance.

### 6. Wiring: `main.rs`

Register `mod economy` and import the four systems.  Then add them to the
appropriate schedules:

- `spawn_gold_hud` joins the `OnEnter(GameState::InGame)` chain **after**
  `load_level_data` — it reads the `Gold` resource that `load_level_data`
  inserts, so ordering matters here.
- `earn_passive_income` joins the `FixedUpdate` chain.
- `update_gold_hud` and `tick_placement_denied` join the `Update` block.

### 7. Level data: bounty in TOML

Add `bounty = 25` to every enemy type definition in all three level files.
All four enemy types get the same value for now, but the per-type field is
ready when we want tougher enemies to reward more.

---

## Running the project

```bash
cargo run
```

Expected behavior:

1. Level select screen appears as before.
2. Choose a level → "Gold: 300" appears top-left.
3. Hover over grass — preview is **green**.
4. Click — tower spawns, gold drops to 200.
5. Place 3 towers → gold hits 0, preview turns **white** (no longer green).
6. Click with 0 gold → preview flashes **red** for 0.3 s.
7. As enemies die, gold increases by 25 per kill.
8. Gold also ticks up passively (~3/sec).
9. After enough income, preview turns green again.

---

## Design decisions

| Decision | Rationale |
|---|---|
| **`Gold` as `f32`** | Avoids a separate accumulator; a simple `as u32` floors for display. |
| **Economy logic in `FixedUpdate`** | Same cadence as movement/combat — simulation-consistent. |
| **HUD in `Update`** | Every rendered frame; no visual lag. |
| **Denied flash in `Update`** | 64 ms FixedUpdate would make a 0.3 s flash feel sluggish. |
| **Color-based preview feedback** | No extra UI chrome; leverages the existing preview sprites. |
| **`Bounty` as a component** | Each enemy entity carries its own value; no lookup needed on kill. |
| **`PlacementDenied` as a transient component** | The component *is* the state — added on click, removed on expiry. No flags. |

---

## Alternatives and enhancements

- **Text popups for income events** — instead of just updating the counter,
  spawn "+25" text that floats upward and fades.  Teaches particle-style
  effects.
- **Gold cap** — adds a `max_gold` resource and clamps on earn.  Encourages
  spending.
- **Data-driven tower costs** — add a `cost: u32` field to a `TowerTypeDef`
  alongside range, damage, and sprite index.  Look it up at placement time
  instead of using `TOWER_COST`.
- **Fail-loud deserialization** — remove `#[serde(default)]` from `bounty` and
  require it in every level TOML.  Prevents silent misconfiguration.

---

## Recap

In this part we:

1. Created `src/economy.rs` — a focused module for gold-related types and systems.
2. Stored gold as `f32` to handle fractional passive income without an accumulator.
3. Added `Bounty` to the data pipeline: TOML → `SpawnEvent` → enemy entity component.
4. Gated tower placement behind a gold check with a red-flash feedback on denial.
5. Added a kill-bounty reward in `attack_enemies`.
6. Placed economy logic in `FixedUpdate` and visual updates in `Update`.
7. Added a simple in-game HUD showing the current gold balance.

The game now has the core resource loop that makes tower defense strategic
rather than just a placement sandbox.
