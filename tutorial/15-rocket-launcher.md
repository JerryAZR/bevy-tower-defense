# Part 15: Rocket Launcher — Projectiles, Ammo, and Splash Damage

> **Time to read:** ~12 minutes  
> **New concepts:** `GlobalTransform`, entity references in components  
> **Prerequisite:** Part 14 (gold economy)

---

## Recap: What We Already Have

The game loop is complete: enemies walk the path, towers shoot them, gold gates placement, and win/lose conditions transition to the GameOver screen. But every tower behaves identically — instant damage, no variety, no visual payoff when an enemy dies.

---

## Goal: What We Will Build

We replace the instant-damage tower with a **rocket launcher** that:

- Shows **visible ammo slots** (rockets resting on the barrel) that deplete when firing and refill over time.
- Fires **homing projectiles** that track a target enemy and rotate to face it.
- Deals **splash damage** on impact — one rocket can damage multiple clustered enemies.
- Spawns an **explosion visual** that auto-despawns after 0.15 s.

We build this in stages: first the visual tower, then firing and projectile flight, then refilling the empty slots.

---

## New Bevy APIs & Concepts

### `GlobalTransform`

`Transform` stores an entity's position *relative to its parent* (local space). `GlobalTransform` stores the computed position in world space — exactly what you need when a child entity's visual position matters independently of its parent.

In our case, ammo-slot rockets are children of the launcher barrel. Their `Transform` is an offset like `(-12, 8)` from the barrel center. To spawn a projectile that starts where the slot *visually* is, we read `GlobalTransform::translation()`, which includes the barrel's rotation and position.

> **Pitfall:** Querying only `Transform` on a child gives local coordinates. If you spawn a new entity at that position, it appears at the wrong place in world space. Always use `GlobalTransform` when you need world-space coordinates of children.

### Storing `Entity` in a component

`Projectile` stores `target: Entity` — the enemy it is chasing. This is a direct entity reference, not a query. Each frame, `move_projectiles` looks up that enemy's current `Transform` to update the projectile's destination.

> **Pitfall:** If the target is despawned (e.g., killed by another tower), the `Entity` becomes invalid. We handle this gracefully: `enemies.get(projectile.target)` returns `Err`, and the projectile continues to its last-known position instead of panicking.

---

## Walkthrough

### Designing the launcher

Before writing code, think about what the player should see:

1. **Base and barrel** — a static base plus a rotating barrel that points toward enemies.
2. **Ammo on the barrel** — three small rockets sitting in visible slots. These are part of the tower, not independent entities (yet).
3. **Depletion** — when the launcher fires, one slot becomes empty.
4. **Refill** — after a short delay, a new rocket appears in the first empty slot.
5. **Fire cooldown** — even if all three rockets are present, the launcher can't dump them instantly. There must be a pause between shots.

From this we derive the data we need:

- A tag component `RocketLauncher` so systems know which entities are rocket launchers.
- Sprite indices for the base (`ROCKET_LAUNCHER_BASE`) and barrel (`ROCKET_LAUNCHER_BARREL`).
- A capacity constant `ROCKET_MAX_AMMO = 3`.
- A component `AmmoSlots` that remembers which slot positions are currently occupied by a rocket child entity.
- A timer component `AmmoRegenTimer` that controls how quickly empty slots refill.
- A timer component `AttackTimer` that gates how fast the launcher can fire. We reuse the same `AttackTimer` from Part 9 instead of inventing a new component — `attack_enemies` queries for `TowerTurret`, so it will never touch a `RocketLauncher` even if both carry `AttackTimer`.

We also need offsets so the three rockets appear in the right places on the barrel:

```rust
const ROCKET_LAUNCHER_BASE: usize = 182;
const ROCKET_LAUNCHER_BARREL: usize = 228;
const ROCKET_SPRITE: usize = 251;
const ROCKET_MAX_AMMO: u8 = 3;
const AMMO_REFILL_SECS: f32 = 2.0;
const ATTACK_PAUSE_SECS: f32 = 0.3;
const AMMO_SLOT_OFFSETS: [(f32, f32); 3] = [(0.0, 8.0), (-12.0, 8.0), (12.0, 8.0)];
```

And the components:

```rust
#[derive(Component)]
pub(crate) struct RocketLauncher;

#[derive(Component)]
pub(crate) struct AmmoSlots {
    pub slots: Vec<Option<Entity>>,
}

#[derive(Component)]
pub(crate) struct AmmoRegenTimer(pub Timer);
```

`AmmoSlots` starts with `vec![None; 3]`. Each `Some(entity)` means a rocket child is occupying that slot. `None` means it's empty.

### Preserving the old tower

The instant-damage tower from Part 9 is still useful — we'll let the player choose between tower types in Part 16. Extract its spawn logic into a helper so we can keep it around without cluttering the click handler:

```rust
fn spawn_instant_tower(commands: &mut Commands, atlas: &TowerAtlas, pos: Vec2) {
    commands.spawn((
        Tower,
        GameEntity,
        Sprite::from_atlas_image(
            atlas.texture.clone(),
            TextureAtlas { layout: atlas.layout.clone(), index: TOWER_BASE },
        ),
        Transform::from_xyz(pos.x, pos.y, 2.0),
    ));

    commands.spawn((
        TowerTurret,
        GameEntity,
        Sprite::from_atlas_image(
            atlas.texture.clone(),
            TextureAtlas { layout: atlas.layout.clone(), index: TOWER_TOP },
        ),
        Transform::from_xyz(pos.x, pos.y, 2.1),
        AttackRange(ATTACK_RANGE),
        Damage(DAMAGE),
        AttackTimer(Timer::from_seconds(ATTACK_COOLDOWN, TimerMode::Repeating)),
    ));
}
```

### Spawning the rocket launcher

`spawn_rocket_launcher` in `src/tower.rs` does three things in sequence:

1. **Spawn the static base** — a `Tower` entity with the base sprite (index 182) at the tile center.
2. **Spawn the rotating barrel** — a `RocketLauncher` entity with the barrel sprite, `AttackRange` (reused from Part 9), `AttackTimer`, `AmmoRegenTimer`, and an empty `AmmoSlots`. `AmmoSlots` starts as `vec![None; 3]` because we haven't spawned the children yet.
3. **Attach three rocket children** — using `with_children`, we spawn a rocket sprite at each `AMMO_SLOT_OFFSET` and capture its `Entity` ID. The closure borrows `commands` mutably, so we can't mutate `AmmoSlots` inside it. Instead we collect the IDs into a local `Vec`, then `insert` the completed `AmmoSlots` after the closure returns.
### Updating the placement preview

The preview should show the new tower. Update `spawn_placement_preview` to use the rocket launcher sprites:

```rust
commands.spawn((
    TowerPreview,
    GameEntity,
    tinted(ROCKET_LAUNCHER_BASE),
    Transform::from_xyz(0.0, 0.0, 2.0),
    Visibility::Hidden,
));
commands.spawn((
    TowerPreview,
    GameEntity,
    tinted(ROCKET_LAUNCHER_BARREL),
    Transform::from_xyz(0.0, 0.0, 2.1),
    Visibility::Hidden,
));
```

### Wiring up placement

In `place_tower_on_click`, replace the old inline spawn with a call to the new helper:

```rust
// For Part 15 only the rocket launcher is spawnable.
spawn_rocket_launcher(&mut commands, &atlas, pos);
```

> **Run the game now.** You should be able to place rocket launchers on grass tiles. They look correct — base, barrel, and three rockets — but they don't do anything yet. We haven't written the systems that make them fire.
>
> Notice the launcher does **not** have a `Damage` component and is tagged `RocketLauncher` instead of `TowerTurret`. That's why the old `attack_enemies` system ignores it completely — it only looks for entities with both `TowerTurret` and `Damage`.

### Firing rockets

`launch_rockets` in `src/tower.rs` is a `FixedUpdate` system. It needs four queries to do its job:

- **`RocketLauncher` entities with `AttackTimer`, `AttackRange`, `AmmoSlots`, and `&mut Transform`** — these are the barrels. `With<RocketLauncher>` limits the query to launchers, and `Without<Enemy>` keeps Bevy happy by proving we don't overlap with the enemy query below. We need `&mut Transform` to rotate the barrel, `&mut AttackTimer` to tick the cooldown, and `&mut AmmoSlots` to consume a rocket.
- **All `Enemy` entities with `Transform` and `PathFollower`** — a read-only snapshot of enemy positions so we can find the nearest target. `With<PathFollower>` matches `attack_enemies` from Part 9: enemies that reach the base lose this component (or are about to be despawned by `process_base_reachers`), so we don't waste rockets on them. `Without<TowerTurret>` and `Without<RocketLauncher>` are standard disjointness filters.
- **All entities with `&GlobalTransform`** — a broad read-only query so we can look up the ammo slot child's world-space position by its `Entity` ID. We don't filter here because we only call `.get(ammo_entity)` on known slot IDs.
- **`Commands`** — to despawn the consumed ammo child and spawn the new projectile entity.

The algorithm is the same as the old tower: snapshot enemy positions, find the nearest one in range, and rotate the barrel to face it. The difference is what happens when the `AttackTimer` fires.

Instead of dealing instant damage, the launcher checks `AmmoSlots` for the first filled position. If one exists, it:
- Reads the child's `GlobalTransform` to get its exact world-space position.
- Removes that slot from `AmmoSlots` (so it becomes `None`).
- Despawns the child rocket sprite that was visually sitting in that slot.
- Spawns a new **projectile** entity at that position, aimed at the target enemy.

Using `GlobalTransform` here is the key detail: if we used the child's local `Transform` (an offset like `(-12, 8)`), the projectile would spawn at the wrong place in world space. `GlobalTransform::translation()` includes the barrel's position and rotation, so the rocket appears to lift cleanly off the launcher.

A projectile is an independent entity — not a child of the tower — so it needs to know what to chase. We define a `Projectile` component that stores the target entity, fallback position, speed, damage, and splash radius:

```rust
#[derive(Component)]
pub(crate) struct Projectile {
    pub target: Entity,        // homing target — updated each frame
    pub target_position: Vec2, // fallback if target dies
    pub speed: f32,
    pub damage: f32,
    pub splash_radius: f32,
}
```

Constants for the projectile:

```rust
const ROCKET_SPEED: f32 = 600.0;
const ROCKET_DAMAGE: f32 = 50.0;
const SPLASH_RADIUS: f32 = 60.0;
const EXPLOSION_SPRITE: usize = 21;
```

When a projectile reaches its target, we tag it with `Exploding` so `explode_projectiles` can handle it next:

```rust
#[derive(Component)]
pub(crate) struct Exploding;
```

See `pub fn launch_rockets` in `src/tower.rs` for the full implementation.


### Homing and movement

`move_projectiles` needs two queries:

- **`Projectile` entities with `&mut Transform`** — to move and rotate the rocket. `Without<Exploding>` keeps us from touching projectiles that have already reached their target (those are handled by `explode_projectiles` next). `Without<Enemy>` proves disjointness from the enemy query.
- **`Enemy` entities with `&Transform`** — read-only lookup of enemy positions for homing. `Without<Projectile>` proves no entity is both an enemy and a projectile.

Each frame the system does three things for every projectile:

1. **Homing** — it looks up the target enemy by its stored `Entity` ID. If the enemy is still alive, `projectile.target_position` is updated to the enemy's current position. If the enemy is dead (despawned), the lookup fails gracefully and the projectile continues toward its last-known position.
2. **Rotation** — it computes the angle from the rocket's current position to `target_position` and sets `transform.rotation` so the sprite points in the direction of travel.
3. **Movement** — it moves the projectile along the vector to the target at `ROCKET_SPEED` pixels per second. If the remaining distance is less than one frame's travel, it snaps to the target position and inserts the `Exploding` tag. Otherwise it advances by `speed * delta_secs` and waits for the next frame.

See `pub fn move_projectiles` in `src/tower.rs` for the full implementation.

> **Note the `Without` filters.** As in Part 9, both queries access `Transform` and one writes to it, so Bevy needs explicit proof they don't overlap. `Without<Enemy>` on projectiles and `Without<Projectile>` on enemies makes them disjoint.

`explode_projectiles` needs two queries:

- **`Exploding` projectiles with `Transform` and `Projectile`** — read-only, because we only need position and damage data. `Without<Enemy>` proves disjointness.
- **`Enemy` entities with `Transform`, `Health`, and `Bounty`** — we need `&mut Health` to apply damage and `&Bounty` to award gold on kill. `Without<Exploding>` proves no overlap with the projectile query.

It also needs `ResMut<Gold>` to award bounties, `Res<TowerAtlas>` for the explosion sprite, and `Commands` to spawn the explosion visual and despawn the projectile.

For each `Exploding` projectile, the system:

1. **Computes splash zone** — uses the projectile's `Transform` position as the explosion center.
2. **Finds victims** — iterates every enemy and checks if its position is within `splash_radius` of the center.
3. **Applies damage** — subtracts `ROCKET_DAMAGE` from the enemy's `Health`. If health drops to ≤ 0, awards the `Bounty` and despawns the enemy.
4. **Skips the dead** — before applying damage, it checks `health.0 > 0.0`. This guard matters because two rockets can explode on the same frame and both splash the same enemy. Without the check, the first rocket would kill the enemy, the second would subtract health *again* from an already-dead entity, and the player would collect the bounty twice.
5. **Spawns visual** — spawns a short-lived explosion sprite (index 21) with a `DespawnTimer` of 0.15 s.
6. **Cleans up** — despawns the projectile itself.

See `pub fn explode_projectiles` in `src/tower.rs` for the full implementation.

> **Run the game now.** Rocket launchers should track enemies, fire homing rockets, and deal splash damage on impact. Watch the ammo slots — each shot removes one rocket from the barrel. After three shots the launcher goes silent because we haven't added refill yet.

`refill_ammo` needs one query:

- **`RocketLauncher` entities with `AmmoRegenTimer` and `AmmoSlots`** — `With<RocketLauncher>` limits the query to launchers. We need `&mut AmmoRegenTimer` to tick the timer and `&mut AmmoSlots` to store the new child's entity ID.

It also needs `Commands` to spawn the new child rocket and `Res<TowerAtlas>` for the sprite atlas.

`refill_ammo` runs in `FixedUpdate`. For each launcher, it ticks the `AmmoRegenTimer`. When the timer fires, it finds the first empty slot in `AmmoSlots` (the first `None`), spawns a new rocket sprite child at the corresponding `AMMO_SLOT_OFFSET`, and stores the child's `Entity` ID back into the slot so it becomes `Some(entity)`.

Spawning the child uses `with_children` on the launcher entity, which keeps the rocket attached to the barrel and automatically inherits the barrel's position and rotation. The `DespawnTimer` is not needed here because these child rockets are managed by `AmmoSlots` — they are only removed when the launcher fires, not by a timer.

See `pub fn refill_ammo` in `src/tower.rs` for the full implementation.

Now the full loop is visible: rockets deplete on fire, then slowly reappear one by one.

### Schedule

Register the new systems in `main.rs`, keeping `attack_enemies` in the chain (it finds no `TowerTurret` entities, so it is a no-op):

```rust
.add_systems(FixedUpdate, (
    spawn_wave_enemies,
    move_enemies,
    attack_enemies,      // kept — no TowerTurret entities, so it's a no-op
    refill_ammo,
    launch_rockets,
    move_projectiles,
    explode_projectiles,
    process_base_reachers,
    check_game_state,
    earn_passive_income,
).chain().run_if(in_state(GameState::InGame)))
```

Also bump `TOWER_COST` in `economy.rs` from `100` to `150` to match the rocket launcher's higher power.

---

## Simplifications

- **Hardcoded constants** — damage, speed, splash radius, and ammo count are compile-time constants. A full game would store these in a `TowerType` definition loaded from a config file.
- **One tower type** — only the rocket launcher is spawnable. The old instant-damage helper (`spawn_instant_tower`) is preserved but unused, ready for Part 16's multi-tower selection UI.
- **No projectile pooling** — every rocket is spawned and despawned. For hundreds of simultaneous projectiles you would use an object pool or a `Vec<Projectile>` buffer inside the tower.

---

## Summary

- We replaced the instant-damage tower with a **rocket launcher** that has visible ammo slots on its barrel.
- **`AmmoSlots`** and **`AmmoRegenTimer`** implement a refill system: 3 rockets max, one restored every 2 seconds.
- **`Projectile`** stores a target `Entity` for homing; `move_projectiles` updates the destination each frame and rotates the rocket sprite to face it.
- **`GlobalTransform`** let us spawn projectiles at the exact world-space position of an ammo-slot child, so detachment looks natural.
- **`explode_projectiles`** applies splash damage to all enemies in a 60 px radius, spawning a short-lived explosion visual.
- We kept `attack_enemies` registered and preserved `spawn_instant_tower` so re-adding the old tower in Part 16 is just a matter of wiring it to the UI.

In the next part, we'll build a **tower selection UI** that lets the player choose between the instant-damage tower and the rocket launcher before placing it — finally making both tower types playable.
