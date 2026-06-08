# Part 16: Data-Driven Towers

> **Time to read:** ~12 minutes
> **Prerequisite:** Part 15 (rocket launcher)

---

## Recap: What We Already Have

The game has two tower types — an instant-damage turret and a rocket launcher — but every stat is baked into Rust constants. Adding a third tower would mean editing `src/tower.rs` in four different places.

---

## Goal: What We Will Build

We move all tower stats into an external `assets/towers.toml` file and teach the game to load it at startup. Then we refactor the component model so that related data lives together and unused tags are eliminated.

At the end of this part the game behaves identically, but:

- Every tower definition lives in data, not code.
- The runtime registry resolves cross-references (tower → projectile) at load time.
- Components are consolidated: 14 tower-related components become 5 meaningful state components.

---

## Walkthrough

### Designing the data format

Before writing code, think about what belongs together. A tower and its projectile are different concerns:

- The **tower** handles targeting, cooldown, ammo, and cost.
- The **projectile** handles flight, damage, and splash.

Mashing them together means every instant-damage tower carries useless rocket fields. Separating them in the TOML keeps each definition focused, while our load-time post-processing merges them into a single runtime struct.

We also want `preview_top_sprite` — the sprite shown in the placement preview and (later) the selection UI — which may differ from the in-game rotating top.

Finally, `ammo_slot_offsets` is a list of positions. Its length *is* the max ammo capacity. No separate `max_ammo` field needed.

### The TOML file

Create `assets/towers.toml`:

```toml
[towers.rapid]
name = "Rapid Turret"
description = "Fast-firing turret with moderate damage."
base_sprite = 180
top_sprite = 203
preview_top_sprite = 203
cost = 100
attack_range = 192.0
attack_cooldown = 0.5
damage = 34.0
muzzle_flash_sprite = 295

[towers.rocket]
name = "Rocket Launcher"
description = "Slow-firing launcher with homing rockets and splash damage."
base_sprite = 182
top_sprite = 228
preview_top_sprite = 228
cost = 150
attack_range = 192.0
attack_cooldown = 0.3
projectile = "rocket"
ammo_slot_offsets = [[0.0, 8.0], [-12.0, 8.0], [12.0, 8.0]]
ammo_refill_secs = 2.0

[projectiles.rocket]
damage = 50.0
speed = 600.0
sprite = 251
explosion_sprite = 21
splash_radius = 60.0
```

### Raw vs. runtime structs

`serde` and `toml` turn file text into Rust values, but the shape that is easy to *write* is not always the shape that is easy to *use*. Our TOML keeps projectiles in a separate table (`[projectiles.rocket]`) so designers can edit them independently, but at runtime every tower that fires a rocket wants the projectile data right there in its definition.

The solution is a two-stage pipeline:

1. **Raw structs** — exactly what `serde` reads from the TOML. `TowerDefinitionRaw` has `projectile: Option<String>`.
2. **Runtime structs** — what the game actually uses. `TowerDefinition` has `projectile: Option<ProjectileDefinition>`, because `load_tower_registry` looked up the string key in the raw projectile map and cloned the value.

In `src/tower.rs`, define the raw structs that `serde` will populate:

```rust
#[derive(Debug, Deserialize)]
struct TowerRegistryRaw {
    #[serde(rename = "towers")]
    towers: HashMap<String, TowerDefinitionRaw>,
    #[serde(rename = "projectiles")]
    projectiles: HashMap<String, ProjectileDefinitionRaw>,
}

#[derive(Debug, Deserialize, Clone)]
struct TowerDefinitionRaw {
    name: String,
    description: String,
    base_sprite: usize,
    // ... all the same fields as the TOML
    projectile: Option<String>,
    // ...
}

#[derive(Debug, Deserialize, Clone)]
struct ProjectileDefinitionRaw {
    damage: f32,
    speed: f32,
    sprite: usize,
    explosion_sprite: Option<usize>,
    splash_radius: Option<f32>,
}
```

The runtime structs are almost identical, except `projectile` is resolved:

```rust
#[derive(Debug, Clone, Resource)]
pub struct TowerRegistry {
    pub towers: HashMap<String, TowerDefinition>,
}

#[derive(Debug, Clone)]
pub struct TowerDefinition {
    pub name: String,
    pub description: String,
    pub base_sprite: usize,
    pub top_sprite: usize,
    pub preview_top_sprite: usize,
    pub cost: u32,
    pub attack_range: f32,
    pub attack_cooldown: f32,
    pub damage: Option<f32>,
    pub muzzle_flash_sprite: Option<usize>,
    pub projectile: Option<ProjectileDefinition>,  // ← resolved at load time
    pub ammo_slot_offsets: Option<Vec<[f32; 2]>>,
    pub ammo_refill_secs: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct ProjectileDefinition {
    pub damage: f32,
    pub speed: f32,
    pub sprite: usize,
    pub explosion_sprite: Option<usize>,
    pub splash_radius: Option<f32>,
}
```

The loading function reads the raw TOML, then maps each tower's `projectile` string into an actual `ProjectileDefinition` clone:

```rust
pub fn load_tower_registry(mut commands: Commands) {
    let raw: TowerRegistryRaw = {
        let content = std::fs::read_to_string("assets/towers.toml")
            .unwrap_or_else(|e| panic!("Failed to read assets/towers.toml: {}", e));
        toml::from_str(&content)
            .unwrap_or_else(|e| panic!("Failed to parse assets/towers.toml: {}", e))
    };

    let mut towers = HashMap::new();
    for (id, def_raw) in raw.towers {
        let projectile = def_raw.projectile.as_ref().and_then(|p| {
            raw.projectiles.get(p).cloned().map(|pr| ProjectileDefinition {
                damage: pr.damage,
                speed: pr.speed,
                sprite: pr.sprite,
                explosion_sprite: pr.explosion_sprite,
                splash_radius: pr.splash_radius,
            })
        });

        towers.insert(id, TowerDefinition {
            name: def_raw.name,
            description: def_raw.description,
            base_sprite: def_raw.base_sprite,
            top_sprite: def_raw.top_sprite,
            preview_top_sprite: def_raw.preview_top_sprite,
            cost: def_raw.cost,
            attack_range: def_raw.attack_range,
            attack_cooldown: def_raw.attack_cooldown,
            damage: def_raw.damage,
            muzzle_flash_sprite: def_raw.muzzle_flash_sprite,
            projectile,
            ammo_slot_offsets: def_raw.ammo_slot_offsets,
            ammo_refill_secs: def_raw.ammo_refill_secs,
        });
    }

    commands.insert_resource(TowerRegistry { towers });
    commands.insert_resource(SelectedTowerType("rocket".to_string()));
}
```

`SelectedTowerType` stores which tower the player currently has selected. For Part 16 it is hardcoded to `"rocket"` since there is no selection UI yet. Notice that we use the registry **key** (`"rocket"`) — not the display name (`"Rocket Launcher"`). That key is what `registry.towers.get(...)` expects.

### Consolidating components

Our old component list had 14 tower-related types. Many were always used together or never queried at all. After consolidation:

```rust
// Shared by every tower that can shoot (instant or rocket).
#[derive(Component)]
pub(crate) struct TowerAttacker {
    pub range: f32,
    pub timer: Timer,
}

// Instant-tower-specific state. Also acts as the discriminator:
// `With<InstantShooter>` finds instant towers.
#[derive(Component)]
pub(crate) struct InstantShooter {
    pub damage: f32,
    pub muzzle_flash_sprite: usize,
}

// Rocket-tower-specific state. Also acts as the discriminator:
// `With<AmmoState>` finds rocket launchers.
#[derive(Component)]
pub(crate) struct AmmoState {
    pub regen: Timer,
    pub slots: Vec<Option<Entity>>,
}

// Registry key — all tower tops carry this.
#[derive(Component)]
pub(crate) struct TowerTypeId(pub String);
```

**What changed and why:**

| Removed | Why |
|---|---|
| `Tower` | Never queried by any system. |
| `TowerTurret` | `With<InstantShooter>` already selects instant towers. |
| `RocketLauncher` | `With<AmmoState>` already selects rocket launchers. |
| `AttackRange` + `AttackTimer` | Merged into `TowerAttacker` — every shooter needs both. |
| `Damage` | Merged into `InstantShooter` — only instant towers deal direct damage. |
| `MuzzleFlashSprite` | Merged into `InstantShooter` — only instant towers flash. |
| `ProjectileType` | Eliminated. The projectile definition is now inside `TowerDefinition.projectile`, looked up once via `TowerTypeId` when the timer fires. |
| `AmmoRegenTimer` + `AmmoSlots` | Merged into `AmmoState` — every ammo operation needs both the timer and the slot list. |

This leaves us with **five state components** instead of fourteen, and every query signature is shorter.

There is no universal rule for when to merge. We grouped fields that are always read and written together (`range` + `timer` on every shooter). Fields that vary independently — like `Projectile`, where target, position, speed, and damage all change during flight — stay in their own component.

### Refactoring the spawners

Both spawn functions now accept a `tower_key: &str` so they can store the correct registry key in `TowerTypeId`:

```rust
fn spawn_instant_tower(
    commands: &mut Commands,
    atlas: &TowerAtlas,
    def: &TowerDefinition,
    tower_key: &str,   // ← "rapid", not "Rapid Turret"
    pos: Vec2,
) {
    // ... spawn base sprite ...

    commands.spawn((
        TowerTypeId(tower_key.to_string()),
        // ... sprite, transform ...
        TowerAttacker {
            range: def.attack_range,
            timer: Timer::from_seconds(def.attack_cooldown, TimerMode::Repeating),
        },
        InstantShooter {
            damage: def.damage.expect("instant tower must have damage"),
            muzzle_flash_sprite: def.muzzle_flash_sprite
                .expect("instant tower must have muzzle_flash_sprite"),
        },
    ));
}
```

`spawn_rocket_launcher` does the same, but attaches `AmmoState` instead of `InstantShooter`. It also reads the projectile sprite from `def.projectile.as_ref().unwrap().sprite` so the ammo slot children show rockets, not the launcher barrel.

`place_tower_on_click` looks up the selected tower definition, checks `def.cost`, deducts it, and dispatches to the right spawner based on whether `def.damage` is `Some`. It passes `&selected.0` (the registry key) to both spawners.

### Refactoring the attack systems

Each system now queries a smaller, clearer set of components.

**`attack_enemies`** queries for instant towers via `&InstantShooter`:

```rust
mut turrets: Query<
    (Entity, &mut Transform, &mut TowerAttacker, &InstantShooter),
    Without<Enemy>
>,
mut enemies: Query<
    (Entity, &Transform, &mut Health, &Bounty),
    (With<Enemy>, With<PathFollower>)
>,
```

The `Without<Enemy>` filter on the turret query is not strictly necessary for correctness, but it proves to Bevy that the two queries are disjoint. The system snapshots enemy positions, finds the nearest within `attacker.range`, rotates the turret, and — when `attacker.timer.just_finished()` — deals `instant.damage` and spawns a muzzle flash using `instant.muzzle_flash_sprite`.

**`launch_rockets`** queries for rocket launchers via `&mut AmmoState`:

```rust
mut turrets: Query<
    (Entity, &mut Transform, &mut TowerAttacker, &mut AmmoState, &TowerTypeId),
    Without<Enemy>
>,
enemies: Query<
    (Entity, &Transform),
    (With<Enemy>, With<PathFollower>)
>,
```

When the timer fires and a slot is occupied, the system reads the slot entity's `GlobalTransform` to get a world-space spawn position, despawns the slot child, looks up `def.projectile` via `TowerTypeId`, and spawns a `Projectile` component populated from the resolved definition.

**`refill_ammo`** also queries via `&mut AmmoState`:

```rust
mut turrets: Query<(Entity, &TowerTypeId, &mut AmmoState)>,
```

It ticks `ammo.regen`, finds the first empty slot, looks up `ammo_slot_offsets` and the projectile sprite via `TowerTypeId`, and spawns a new rocket child at the correct offset.

See `src/tower.rs` for the full implementations of `attack_enemies`, `launch_rockets`, and `refill_ammo`.

### Cleaning up

`TOWER_COST` is removed from `economy.rs`. All cost checks now read `def.cost` from the registry. This is the last hardcoded tower constant.

---

## Simplifications

- **No validation** — we assume `projectile` IDs in tower definitions refer to existing projectile definitions. A full game would validate at load time and panic with a helpful message.
- **No hot-reload** — changing `towers.toml` requires a restart. Bevy's asset system could watch the file, but that's out of scope for this tutorial.
- **`SelectedTowerType` is hardcoded** — Part 17 will add a UI to change it.
- **`name` is unused at runtime** — it is stored in `TowerDefinition` for future UI tooltips but never read by gameplay systems.
- **Only two tower archetypes exist** — `place_tower_on_click` dispatches on `def.damage.is_some()`, and each attack system filters by the presence of `InstantShooter` or `AmmoState`. Adding a non-attacking tower (e.g., a gold generator) would require an explicit archetype enum or tag component instead of relying on field presence as a discriminator.

---

## Summary

- We created `assets/towers.toml` with separated **tower** and **projectile** definitions.
- **Post-processing at load time** resolves `projectile` string keys into `ProjectileDefinition` clones, so firing systems do a single HashMap lookup instead of two.
- **`TowerAttacker`** merges range and timer for all shooters; **`InstantShooter`** merges damage and flash sprite for instant towers; **`AmmoState`** merges regen timer and slot list for rocket launchers.
- **Tag components** (`Tower`, `TowerTurret`, `RocketLauncher`) were eliminated because `With<InstantShooter>` and `With<AmmoState>` already discriminate the two tower types.
- **`TowerTypeId`** stores the registry key (e.g., `"rocket"`) so systems can look up tower-specific data when needed.
- All hardcoded constants (`TOWER_COST`, `ROCKET_SPEED`, `FIRE_SPRITE`, etc.) are gone.

In the next part, we'll build a **tower selection UI** that reads from `TowerRegistry` to generate its buttons dynamically — finally making both tower types choosable.
