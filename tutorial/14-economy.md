# Part 14: Gold Economy — The Resource Loop

> **Time to read:** ~25 minutes  
> **New concepts:** `Option<&T>` in queries, transient components, `as u32` truncation for display  
> **Prerequisite:** Part 13 (multiple levels)

---

## Recap: What We Already Have

The game has three playable levels with auto-discovery, a state machine for menus and gameplay, and a complete combat loop: towers shoot, enemies follow paths, and win/lose conditions trigger a game-over screen. But towers are free — the player can paint them across every grass tile. That is a sandbox, not a strategy game.

---

## Goal: What We Will Build

1. **`Gold(f32)` resource** — the player's current balance, starting at 300.
2. **Three income sources** — starting gold, passive income (3/sec), and kill bounties (25 gold per enemy).
3. **Placement cost** — 100 gold per tower; placement is denied if the player cannot pay.
4. **Visual feedback** — preview turns green when affordable, red when denied, white when unaffordable.
5. **In-game HUD** — "Gold: 300" in the top-left corner, updated every frame.

This matters because the core of tower defense is **resource tension**: every tower placement is a trade-off between immediate defense and future income. Without a cost, there is no decision.

---

## New Bevy APIs & Concepts

### `Option<&T>` in queries

Bevy queries can include optional components using `Option<&T>` or `Option<&mut T>`. When the component exists on an entity, the query returns `Some(&T)`; when it does not, it returns `None`. The entity is still included in the iteration either way.

```rust
Query<(&mut Sprite, Option<&PlacementDenied>), With<TowerPreview>>
```

This lets a single system handle multiple states without separate queries or resources. In our case, `update_placement_preview` checks `Option<&PlacementDenied>` to decide whether to tint red (flash active), green (affordable), or white (unaffordable).

> **Pitfall:** `Option<&mut T>` in a query requires mutable access. If another system also queries for `&mut T` on the same entities, Bevy will panic at runtime due to conflicting access. Use `Option<&T>` when you only need to read.

### Transient components

A *transient component* is not a new Bevy type — it is the same `#[derive(Component)]` struct you have already used for `Enemy`, `Health`, and `MapTile`. The difference is **how** you use it: instead of attaching it at spawn and leaving it for the entity's lifetime, you add and remove it dynamically to represent temporary state.

`PlacementDenied(Timer)` is our transient component: it is inserted on the preview sprites when the player clicks but cannot afford a tower, and removed by `tick_placement_denied` when the timer expires. The component *is* the state — no separate boolean flag or resource needed.

> **Why not a `Local<bool>` or a resource?** `Local` is scoped to a single system function, not to entities. A resource could hold a `HashMap<Entity, bool>`, but that adds indirection. A persistent component with a `bool` or enum state would also work, but then you need a system to reset it. The transient component approach fits naturally here because the timer-based removal system (`tick_placement_denied`) already queries for it — adding and removing the component is the same machinery.


> **Pitfall:** Forgetting to remove a transient component leaves it on the entity forever. Always pair an insertion with a system that removes it, or use a timer that expires predictably.

### `as u32` truncation for display

`Gold` is stored as `f32` because passive income adds a fractional amount every fixed timestep (3.0 × (1/60) ≈ 0.05 gold per tick at 60 Hz). Using `u32` would require a separate accumulator to avoid losing the fractional part. Casting `f32` to `u32` truncates toward zero, which naturally floors the value for display:

```rust
gold.0 as u32  // 305.7 → 305
```

This is a common pattern in game UIs: store precise floating-point values internally, display rounded integers to the player.

---

## Walkthrough

### Designing the feature

**Player-visible behavior:**

1. On entering a level, the player sees "Gold: 300" in the top-left corner.
2. Hovering over a grass tile shows a green preview (affordable).
3. Clicking deducts 100 gold and places a tower. The HUD updates immediately.
4. After placing 3 towers, gold hits 0. The preview turns white (valid tile, no money).
5. Clicking with 0 gold flashes the preview red for 0.3 seconds — clear feedback that the action was rejected.
6. As enemies die, gold increases by 25 per kill. The HUD updates in real time.
7. Gold also ticks up passively (~3 per second), so the player eventually recovers enough to place another tower.

**ECS data needed:**

- `Gold(f32)` resource — current balance, inserted in `load_level_data`, removed in `cleanup_level`.
- `Bounty(u32)` component — attached to each enemy at spawn, read on death.
- `PlacementDenied(Timer)` component — transient, attached to preview sprites on denied click.
- `GoldHud` marker component — on the HUD text entity so `update_gold_hud` can find it.
- Constants: `STARTING_GOLD` (300), `PASSIVE_INCOME_RATE` (3.0/sec), `TOWER_COST` (100), `DENIED_FLASH_DURATION` (0.3).
- `economy.rs` module — self-contained file for all gold-related types and systems.

**Design decision: why `f32` for gold?** Passive income adds `3.0 * dt` every fixed timestep. At the default 60 Hz `FixedUpdate`, that is `3.0 * (1/60) = 0.05` gold per tick. With `u32`, we would need a second accumulator variable to avoid rounding away the fractional part every frame. `f32` lets us accumulate smoothly and cast to `u32` only for display.

---

### Step 1: Add `bounty` to `EnemyTypeDef`

The bounty must flow from the level designer's TOML file through to the enemy entity. The first link in that chain is the data definition. In `src/level.rs`, add a `bounty: u32` field to `EnemyTypeDef`:

```rust
#[derive(Debug, Deserialize, Clone)]
pub struct EnemyTypeDef {
    pub sprite: usize,
    pub speed: f32,
    pub health: f32,
    /// Gold awarded when this enemy is killed.
    #[serde(default)]
    pub bounty: u32,
}
```

`#[serde(default)]` makes the field optional in TOML files. If omitted, it defaults to `0`. This is convenient for backwards compatibility but dangerous for gameplay — a forgotten `bounty = 25` means enemies drop no gold and the economy feels broken.

> **Why `#[serde(default)]` here?** The tutorial demonstrates the pattern, but in a production project you might prefer to *not* use `default` and let the deserializer fail loudly with `missing field 'bounty'`. Silent misconfiguration is worse than a clear error.

---

### Step 2: Thread bounty through the spawn pipeline

The bounty value must travel from `EnemyTypeDef` → `SpawnEvent` → enemy entity component. Three places in `src/enemy.rs` change.

**Add `Bounty` to imports:**

```rust
use crate::economy::Bounty;
```

**Add `bounty` to `SpawnEvent`:**

```rust
pub struct SpawnEvent {
    time: f32,
    sprite: usize,
    speed: f32,
    health: f32,
    bounty: u32,
    path: String,
}
```

**In `build_spawn_schedule`, copy the bounty from the definition:**

```rust
events.push(SpawnEvent {
    time,
    sprite: def.sprite,
    speed: def.speed,
    health: def.health,
    bounty: def.bounty,  // new
    path: wave.path.clone(),
});
```

**In `spawn_wave_enemies`, attach `Bounty` as a component:**

```rust
commands.spawn((
    Sprite::from_atlas_image(/* ... */),
    Transform::from_xyz(x, y, 1.0),
    Enemy,
    PathFollower { /* ... */ },
    MoveSpeed(event.speed),
    Health(event.health),
    Bounty(event.bounty),  // new
    GameEntity,
));
```

What does `spawn_wave_enemies` query now?
- All previous queries (unchanged).
- `Bounty(event.bounty)` is inserted as a component, not queried — it travels with the entity.

---

### Step 3: Create `src/economy.rs`

Create a self-contained module for all gold-related types and systems. This keeps the economy logic isolated — other modules only import what they need.

```rust
// src/economy.rs
use bevy::prelude::*;
use crate::state::GameEntity;

pub const STARTING_GOLD: f32 = 300.0;
pub const PASSIVE_INCOME_RATE: f32 = 3.0;
pub const TOWER_COST: u32 = 100;
pub const DENIED_FLASH_DURATION: f32 = 0.3;

#[derive(Resource)]
pub struct Gold(pub f32);

#[derive(Component)]
pub struct GoldHud;

#[derive(Component)]
pub struct Bounty(pub u32);

#[derive(Component)]
pub struct PlacementDenied(pub Timer);
```

What are these?
- `Gold` — the central resource. `f32` so passive income accumulates smoothly.
- `GoldHud` — marker on the HUD text entity.
- `Bounty` — per-enemy kill reward.
- `PlacementDenied` — transient timer component for the red flash.

Constants are `pub` because `tower.rs` needs `TOWER_COST` and `DENIED_FLASH_DURATION`.

**`spawn_gold_hud`** — spawns the HUD text on `OnEnter(GameState::InGame)`:

```rust
pub fn spawn_gold_hud(mut commands: Commands, gold: Res<Gold>) {
    commands.spawn((
        GoldHud,
        GameEntity,
        Text::new(format!("Gold: {}", gold.0 as u32)),
        TextFont {
            font_size: 32.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(16.0),
            left: Val::Px(16.0),
            ..default()
        },
    ));
}
```

What does it query?
- `Commands` — to spawn the HUD entity.
- `Res<Gold>` — to display the initial balance.

The entity carries `GameEntity` so `cleanup_level` despawns it automatically when the level ends. `GoldHud` lets `update_gold_hud` find exactly this text node.

**`update_gold_hud`** — updates the text every frame:

```rust
pub fn update_gold_hud(
    gold: Res<Gold>,
    mut query: Query<&mut Text, With<GoldHud>>,
) {
    let display = gold.0 as u32;
    for mut text in query.iter_mut() {
        *text = Text::new(format!("Gold: {}", display));
    }
}
```

What does it query?
- `Res<Gold>` — the current balance.
- `Query<&mut Text, With<GoldHud>>` — the HUD text entity.

Running in `Update` (not `FixedUpdate`) means the display updates every rendered frame. If a kill bounty arrives between fixed timesteps, the player sees the new value immediately.

**`earn_passive_income`** — adds gold every fixed timestep:

```rust
pub fn earn_passive_income(time: Res<Time>, mut gold: ResMut<Gold>) {
    gold.0 += PASSIVE_INCOME_RATE * time.delta_secs();
}
```

What does it query?
- `Res<Time>` — to get the fixed timestep duration.
- `ResMut<Gold>` — to add the income.

Running in `FixedUpdate` alongside movement and combat ensures income scales correctly regardless of render frame rate.

**`tick_placement_denied`** — removes expired flash timers:

```rust
pub fn tick_placement_denied(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut PlacementDenied)>,
) {
    for (entity, mut denied) in query.iter_mut() {
        denied.0.tick(time.delta());
        if denied.0.just_finished() {
            commands.entity(entity).remove::<PlacementDenied>();
        }
    }
}
```

What does it query?
- `Res<Time>` — to tick the timers.
- `Commands` — to remove the component.
- `Query<(Entity, &mut PlacementDenied)>` — all entities with an active denied flash.

Running in `Update` (not `FixedUpdate`) because this is a visual feedback system, not simulation logic. `FixedUpdate` handles gameplay state (movement, combat, income); `Update` handles presentation (HUD text, color tints, visual timers). The timer expires correctly in either schedule — `time.delta()` returns the wall-clock frame delta in `Update` — but keeping visual systems in `Update` separates presentation from simulation.

---

### Step 4: Update `load_level_data` to insert `Gold`

In `src/gameplay.rs`, add `Gold` to the resources inserted at level start:

```rust
use crate::economy::{Gold, STARTING_GOLD};

pub fn load_level_data(mut commands: Commands, selected: Res<SelectedLevel>) {
    let level = load_level(&selected.0);
    let map = build_map_from_level(&level);
    let rules = build_rules();

    commands.insert_resource(map);
    commands.insert_resource(rules);
    commands.insert_resource(level);
    commands.insert_resource(PlacedTowers::default());
    commands.insert_resource(BaseLives(5));
    commands.insert_resource(Gold(STARTING_GOLD));  // new
}
```

`Gold` starts fresh at 300 for every level because `load_level_data` inserts `Gold(STARTING_GOLD)` every time we enter `InGame`.

---

### Step 5: Update `cleanup_level` to remove `Gold`

In `src/state.rs`, add `Gold` removal:

```rust
use crate::economy::Gold;

pub fn cleanup_level(
    mut commands: Commands,
    entities: Query<Entity, With<GameEntity>>,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
    // ... existing removes ...
    commands.remove_resource::<Gold>();  // new
}
```

While `load_level_data` would overwrite `Gold` on the next level anyway, explicitly removing it makes the intent clear: every level starts with a clean slate.

---

### Step 6: Tint the preview based on affordability

In `src/tower.rs`, `update_placement_preview` now reads `Res<Gold>` and queries `Option<&PlacementDenied>` to decide the tint color.

What does it query now?
- `Res<Gold>` — to check affordability.
- `Query<(&mut Transform, &mut Visibility, &mut Sprite, Option<&PlacementDenied>), With<TowerPreview>>` — the preview sprites, plus the optional denied flash.

```rust
use crate::economy::{Gold, PlacementDenied, TOWER_COST};

pub fn update_placement_preview(
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform)>,
    map_layout: Res<MapLayout>,
    placed: Res<PlacedTowers>,
    gold: Res<Gold>,
    mut preview_q: Query<(&mut Transform, &mut Visibility, &mut Sprite, Option<&PlacementDenied>), With<TowerPreview>>,
) {
    let (cam, cam_transform) = *camera;

    let Some(tile) = hovered_placeable_tile(
        &window, &cam, &cam_transform, &map_layout, &placed,
    ) else {
        for (_, mut vis, ..) in preview_q.iter_mut() {
            *vis = Visibility::Hidden;
        }
        return;
    };

    let pos = tile_to_world(tile, map_layout.width as f32, map_layout.height as f32);
    let can_afford = gold.0 >= TOWER_COST as f32;

    for (mut transform, mut vis, mut sprite, denied) in preview_q.iter_mut() {
        transform.translation.x = pos.x;
        transform.translation.y = pos.y;
        *vis = Visibility::Visible;

        if denied.is_some() {
            sprite.color = Color::srgba(1.0, 0.3, 0.3, 0.5);  // red flash
        } else if can_afford {
            sprite.color = Color::srgba(0.3, 1.0, 0.3, 0.5);  // green
        } else {
            sprite.color = Color::srgba(1.0, 1.0, 1.0, 0.5);  // white/neutral
        }
    }
}
```

The priority is deliberate: `PlacementDenied` overrides everything. Even if the player earns enough gold during the flash, the preview stays red until the timer expires. This gives consistent feedback for the action the player just attempted.

---

### Step 7: Gate tower placement behind gold check

In `src/tower.rs`, `place_tower_on_click` now checks affordability before spawning. If the player cannot pay, it inserts `PlacementDenied` on the preview sprites and returns early.

What does it query now?
- `ResMut<Gold>` — to deduct cost or check affordability.
- `Query<Entity, With<TowerPreview>>` — to attach the denied flash.
- All previous queries unchanged.

```rust
use crate::economy::{Gold, PlacementDenied, TOWER_COST, DENIED_FLASH_DURATION};

pub fn place_tower_on_click(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform)>,
    map_layout: Res<MapLayout>,
    mut placed: ResMut<PlacedTowers>,
    mut gold: ResMut<Gold>,
    atlas: Res<TowerAtlas>,
    preview_q: Query<Entity, With<TowerPreview>>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    let (cam, cam_transform) = *camera;

    let Some(tile) = hovered_placeable_tile(
        &window, &cam, &cam_transform, &map_layout, &placed,
    ) else {
        return;
    };

    // Check affordability
    if gold.0 < TOWER_COST as f32 {
        for preview_entity in preview_q.iter() {
            commands.entity(preview_entity).insert(PlacementDenied(
                Timer::from_seconds(DENIED_FLASH_DURATION, TimerMode::Once),
            ));
        }
        return;
    }

    // Deduct cost *before* spawning to prevent double-placement on one click
    gold.0 -= TOWER_COST as f32;
    placed.0.insert(tile);
    let pos = tile_to_world(tile, map_layout.width as f32, map_layout.height as f32);

    // Spawn tower base and turret (unchanged from Part 13)
    // ...
}
```

---

### Step 8: Award bounties on kill

In `src/tower.rs`, `attack_enemies` now reads `&Bounty` from enemies and adds it to `Gold` when health drops to ≤ 0.

What does it query now?
- `ResMut<Gold>` — to add the bounty.
- `Query<(Entity, &Transform, &mut Health, &Bounty), (With<Enemy>, With<PathFollower>, Without<TowerTurret>)>` — enemies, now including their bounty.

```rust
use crate::economy::{Gold, Bounty};

pub fn attack_enemies(
    time: Res<Time>,
    atlas: Res<TowerAtlas>,
    mut gold: ResMut<Gold>,
    mut turrets: Query<(Entity, &mut Transform, &mut AttackTimer, &Damage, &AttackRange), (With<TowerTurret>, Without<Enemy>)>,
    mut enemies: Query<(Entity, &Transform, &mut Health, &Bounty), (With<Enemy>, With<PathFollower>, Without<TowerTurret>)>,
    mut commands: Commands,
) {
    // ... snapshot enemy positions ...

    for (turret_entity, mut turret_transform, mut timer, damage, range) in turrets.iter_mut() {
        // ... find nearest enemy ...

        if let Some((target, _)) = nearest {
            // ... rotate turret ...

            if timer.0.just_finished() {
                if let Ok((entity, _, mut health, bounty)) = enemies.get_mut(target) {
                    if health.0 > 0.0 {
                        health.0 -= damage.0;
                        if health.0 <= 0.0 {
                            gold.0 += bounty.0 as f32;
                            commands.entity(entity).despawn();
                        }
                    }
                }
                // ... muzzle flash ...
            }
        }
    }
}
```

The bounty is added before `commands.entity(entity).despawn()` so the gold resource is updated while the enemy entity still exists. The cast `bounty.0 as f32` is lossless for practical values (under 16 million).

---

### Step 9: Add bounty values to level TOML files

Add `bounty = 25` to every enemy type definition in all three level files:

```toml
[enemy_types.soldier]
sprite = 245
speed = 192.0
health = 100.0
bounty = 25

[enemy_types.runner]
sprite = 246
speed = 320.0
health = 60.0
bounty = 25

[enemy_types.heavy]
sprite = 247
speed = 96.0
health = 300.0
bounty = 25

[enemy_types.scout]
sprite = 248
speed = 160.0
health = 80.0
bounty = 25
```

All four types get the same value for now, but the per-type field is ready for tougher enemies to reward more later.

---

### Step 10: Wire the systems in `main.rs`

Add `mod economy` and import the four systems. Then register them in the appropriate schedules:

```rust
mod economy;
use economy::{spawn_gold_hud, update_gold_hud, earn_passive_income, tick_placement_denied};

// In the App builder:
.add_systems(OnEnter(GameState::InGame), (
    load_level_data,
    setup_spawn_schedule,
    spawn_tilemap,
    spawn_placement_preview,
    spawn_gold_hud,  // new
).chain())
.add_systems(FixedUpdate, (
    spawn_wave_enemies,
    move_enemies,
    attack_enemies,
    process_base_reachers,
    check_game_state,
    earn_passive_income,  // new
).chain().run_if(in_state(GameState::InGame)))
.add_systems(Update, (
    update_placement_preview,
    place_tower_on_click,
    despawn_timed,
    update_gold_hud,        // new
    tick_placement_denied,  // new
).run_if(in_state(GameState::InGame)))
```

`spawn_gold_hud` joins the `OnEnter(InGame)` chain **after** `load_level_data` because it reads the `Gold` resource that `load_level_data` inserts. The chain ordering guarantees this.

`earn_passive_income` runs in `FixedUpdate` so income scales with simulation time, not render frame rate.

`update_gold_hud` and `tick_placement_denied` run in `Update` because they only affect visual presentation — the HUD text and the preview tint. `FixedUpdate` is reserved for simulation logic.

---

### Step 11: Verify

```bash
cargo run
```

You should see:

- **Title screen** listing three levels as before.
- Choose a level → **"Gold: 300"** appears top-left.
- Hover over grass → preview is **green** (affordable).
- Click → tower spawns, gold drops to **200**.
- Place 3 towers → gold hits **0**, preview turns **white**.
- Click with 0 gold → preview flashes **red** for 0.3 s.
- As enemies die, gold increases by **25** per kill.
- Gold ticks up passively (~3/sec).
- After enough income, preview turns green again.

---

### Simplifications

| Simplification | Why it works | Future direction |
|---|---|---|
| **Tower cost is hardcoded** | One tower type, one price. | Add `cost: u32` to `TowerTypeDef` and look it up at placement time. |
| **No gold cap** | Simplifies earning logic. | Add a `max_gold` resource and clamp on earn to encourage spending. |
| **Flat bounty across types** | All enemy types reward 25 gold. | Tougher enemies (heavy, scout) could reward more to incentivize prioritization. |
| **`#[serde(default)]` on bounty** | Backwards-compatible level files. | Remove `default` and require `bounty` in every TOML to fail loud on misconfiguration. |
| **Single-color preview feedback** | No extra UI chrome needed. | Text popups ("+25") or particle effects for income events. |
| **Passive income is flat** | 3 gold/sec regardless of game state. | Income could scale with wave number, level difficulty, or active tower count. |

---

## Summary

- We created `src/economy.rs` — a focused module for all gold-related types and systems.
- We stored gold as **`f32`** to handle fractional passive income without a separate accumulator, flooring with `as u32` for display.
- We added **`Bounty(u32)`** to the data pipeline: TOML → `EnemyTypeDef` → `SpawnEvent` → enemy entity component → read on kill by `attack_enemies`.
- We gated tower placement behind a **gold check** with a red-flash feedback on denial, using `PlacementDenied(Timer)` as a transient component.
- We used **`Option<&PlacementDenied>`** in a query to handle three preview states (red, green, white) with a single system.
- We placed economy logic in **`FixedUpdate`** (income, combat) and visual updates in **`Update`** (HUD, flash timer) for correct timing.
- We added a simple in-game **HUD** showing the current gold balance.

In **Part 15** we will add a **rocket launcher tower type** with ammo depletion, reload mechanics, and a distinct firing pattern — introducing multiple tower types and the data-driven architecture needed to support them.
