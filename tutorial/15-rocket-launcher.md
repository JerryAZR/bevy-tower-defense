# Part 15: Rocket Launcher — Projectiles, Ammo, and Splash Damage

> **Time to read:** ~12 minutes  
> **New concepts:** `GlobalTransform`, entity references in components, `Without<T>` query disjointness  
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

This part introduces projectiles, entity hierarchy for ammo visuals, and the `GlobalTransform` component for reading world-space positions of child entities.

---

## New Bevy APIs & Concepts

### `GlobalTransform`

`Transform` stores an entity's position *relative to its parent* (local space). `GlobalTransform` stores the computed position in world space — exactly what you need when a child entity's visual position matters independently of its parent.

In our case, ammo-slot rockets are children of the launcher barrel. Their `Transform` is an offset like `(-12, 8)` from the barrel center. To spawn a projectile that starts where the slot *visually* is, we read `GlobalTransform::translation()`, which includes the barrel's rotation and position.

> **Pitfall:** Querying only `Transform` on a child gives local coordinates. If you spawn a new entity at that position, it appears at the wrong place in world space. Always use `GlobalTransform` when you need world-space coordinates of children.

### Storing `Entity` in a component

`Projectile` stores `target: Entity` — the enemy it is chasing. This is a direct entity reference, not a query. Each frame, `move_projectiles` looks up that enemy's current `Transform` to update the projectile's destination.

> **Pitfall:** If the target is despawned (e.g., killed by another tower), the `Entity` becomes invalid. We handle this gracefully: `enemies.get(projectile.target)` returns `Err`, and the projectile continues to its last-known position instead of panicking.

### `Without<T>` for query disjointness

Bevy requires proof that two queries in the same system don't overlap when one has `&mut` access. Adding `Without<Enemy>` to the projectile query and `Without<Projectile>` to the enemy query tells Bevy "no entity is both" — even though that is obvious to a human reader.

---

## Walkthrough

### Constants and new components

Add the rocket launcher constants and six new components to `src/tower.rs`:

```rust
// Rocket launcher constants (hardcoded for Part 15)
const ROCKET_LAUNCHER_BASE: usize = 182;
const ROCKET_LAUNCHER_BARREL: usize = 228;
const ROCKET_SPRITE: usize = 251;
const EXPLOSION_SPRITE: usize = 21;
const ROCKET_MAX_AMMO: u8 = 3;
const AMMO_REFILL_SECS: f32 = 2.0;
const ATTACK_PAUSE_SECS: f32 = 0.3;
const ROCKET_SPEED: f32 = 600.0;
const ROCKET_DAMAGE: f32 = 50.0;
const SPLASH_RADIUS: f32 = 60.0;
const AMMO_SLOT_OFFSETS: [(f32, f32); 3] = [(0.0, 8.0), (-12.0, 8.0), (12.0, 8.0)];
```

```rust
#[derive(Component)]
pub(crate) struct RocketLauncher;

#[derive(Component)]
pub(crate) struct AmmoSlots {
    pub slots: Vec<Option<Entity>>,
}

#[derive(Component)]
pub(crate) struct AmmoRegenTimer(pub Timer);

#[derive(Component)]
pub(crate) struct AttackCooldown(pub Timer);

#[derive(Component)]
pub(crate) struct Projectile {
    pub target: Entity,        // homing target — updated each frame
    pub target_position: Vec2, // fallback if target dies
    pub speed: f32,
    pub damage: f32,
    pub splash_radius: f32,
}

#[derive(Component)]
pub(crate) struct Exploding;
```

`AmmoSlots` tracks which slot positions are currently filled with a visible rocket child entity. `AmmoRegenTimer` ticks every 2 seconds to refill one empty slot. `AttackCooldown` gates firing rate so the launcher doesn't dump all 3 rockets instantly.

### Preserving the old tower

Extract the old instant-damage spawn logic into a helper function. It is unused for now, but keeping it makes multi-tower selection (Part 16) straightforward:

```rust
fn spawn_instant_tower(commands: &mut Commands, atlas: &TowerAtlas, pos: Vec2) {
    // Base + turret with AttackTimer and Damage components...
}
```

### Spawning the rocket launcher

The new `spawn_rocket_launcher` helper does three things:

1. Spawn the static base (sprite 182).
2. Spawn the rotating barrel (sprite 228) with `RocketLauncher`, `AttackCooldown`, `AmmoRegenTimer`, and an empty `AmmoSlots`.
3. Spawn 3 rocket children on the barrel, capture their `Entity` IDs, and store them in `AmmoSlots`.

```rust
fn spawn_rocket_launcher(commands: &mut Commands, atlas: &TowerAtlas, pos: Vec2) {
    commands.spawn((
        Tower, GameEntity,
        Sprite::from_atlas_image(atlas.texture.clone(), /* sprite 182 */),
        Transform::from_xyz(pos.x, pos.y, 2.0),
    ));

    let turret_entity = commands.spawn((
        RocketLauncher, GameEntity,
        Sprite::from_atlas_image(atlas.texture.clone(), /* sprite 228 */),
        Transform::from_xyz(pos.x, pos.y, 2.1),
        AttackRange(ATTACK_RANGE),
        AttackCooldown(Timer::from_seconds(ATTACK_PAUSE_SECS, TimerMode::Repeating)),
        AmmoRegenTimer(Timer::from_seconds(AMMO_REFILL_SECS, TimerMode::Repeating)),
        AmmoSlots { slots: vec![None; ROCKET_MAX_AMMO as usize] },
    )).id();

    let mut slot_entities = vec![None; ROCKET_MAX_AMMO as usize];
    commands.entity(turret_entity).with_children(|turret_children| {
        for i in 0..ROCKET_MAX_AMMO {
            let offset = AMMO_SLOT_OFFSETS[i as usize];
            let slot_entity = turret_children.spawn((
                Sprite::from_atlas_image(atlas.texture.clone(), /* sprite 251 */),
                Transform::from_xyz(offset.0, offset.1, 2.2),
            )).id();
            slot_entities[i as usize] = Some(slot_entity);
        }
    });
    commands.entity(turret_entity)
        .insert(AmmoSlots { slots: slot_entities });
}
```

> **Why `with_children` then `insert`?** The closure borrows `commands` mutably, so we can't mutate `AmmoSlots` inside it. Instead, we collect IDs into a local `Vec`, then insert the component after the closure returns.

`place_tower_on_click` now calls `spawn_rocket_launcher` instead of spawning the old tower directly.

### Refilling ammo

`refill_ammo` runs in `FixedUpdate`. For each launcher, it ticks the regen timer. When the timer fires, it finds the first empty slot, spawns a new child rocket, and stores the entity ID:

```rust
pub fn refill_ammo(
    time: Res<Time>,
    mut turrets: Query<(Entity, &mut AmmoRegenTimer, &mut AmmoSlots), With<RocketLauncher>>,
    mut commands: Commands,
    atlas: Res<TowerAtlas>,
) {
    for (turret_entity, mut regen, mut ammo) in turrets.iter_mut() {
        regen.0.tick(time.delta());
        if regen.0.just_finished() {
            if let Some(idx) = ammo.slots.iter().position(|s| s.is_none()) {
                let mut new_entity = None;
                commands.entity(turret_entity).with_children(|children| {
                    let offset = AMMO_SLOT_OFFSETS[idx];
                    new_entity = Some(children.spawn((
                        Sprite::from_atlas_image(atlas.texture.clone(), /* sprite 251 */),
                        Transform::from_xyz(offset.0, offset.1, 2.2),
                    )).id());
                });
                if let Some(e) = new_entity {
                    ammo.slots[idx] = Some(e);
                }
            }
        }
    }
}
```

### Firing rockets

`launch_rockets` targets the nearest enemy (same algorithm as the old tower), rotates the barrel, and fires if the cooldown is ready and ammo is available:

```rust
if cooldown.0.just_finished() {
    if let Some(idx) = ammo.slots.iter().position(|s| s.is_some()) {
        let ammo_entity = ammo.slots[idx].take().unwrap();
        let spawn_pos = slot_transforms.get(ammo_entity)
            .map(|gt| gt.translation().truncate())
            .unwrap_or(turret_pos);
        commands.entity(ammo_entity).despawn();

        commands.spawn((
            Projectile {
                target, target_position: target_pos,
                speed: ROCKET_SPEED, damage: ROCKET_DAMAGE,
                splash_radius: SPLASH_RADIUS,
            },
            GameEntity,
            Sprite::from_atlas_image(atlas.texture.clone(), /* sprite 251 */),
            Transform::from_xyz(spawn_pos.x, spawn_pos.y, 2.3),
        ));
    }
}
```

The projectile spawns at the slot's `GlobalTransform` position, so it visually detaches from the launcher rather than teleporting to the turret center.

### Homing and movement

`move_projectiles` updates each projectile's target position if the enemy is still alive, then rotates the rocket to face the target and moves toward it:

```rust
pub fn move_projectiles(
    time: Res<Time>,
    mut projectiles: Query<(Entity, &mut Transform, &mut Projectile),
                           (Without<Exploding>, Without<Enemy>)>,
    enemies: Query<&Transform, (With<Enemy>, Without<Exploding>, Without<Projectile>)>,
    mut commands: Commands,
) {
    for (entity, mut transform, mut projectile) in projectiles.iter_mut() {
        if let Ok(target_transform) = enemies.get(projectile.target) {
            projectile.target_position = target_transform.translation.truncate();
        }

        let current_pos = transform.translation.truncate();
        let to_target = projectile.target_position - current_pos;
        let distance = to_target.length();

        if distance > 0.0 {
            let direction = to_target.normalize();
            let angle = direction.y.atan2(direction.x) - std::f32::consts::FRAC_PI_2;
            transform.rotation = Quat::from_rotation_z(angle);
        }

        if distance <= projectile.speed * time.delta_secs() {
            transform.translation.x = projectile.target_position.x;
            transform.translation.y = projectile.target_position.y;
            commands.entity(entity).insert(Exploding);
        } else {
            transform.translation += (to_target.normalize()
                * projectile.speed * time.delta_secs()).extend(0.0);
        }
    }
}
```

> **Note the `Without` filters.** Both queries access `Transform`, so Bevy needs explicit proof they don't overlap. `Without<Enemy>` on projectiles and `Without<Projectile>` on enemies makes them disjoint.

### Explosion and splash damage

`explode_projectiles` queries all `Projectile` entities tagged with `Exploding`. For each, it damages every enemy within `splash_radius`, spawns an explosion sprite, and despawns the projectile:

```rust
pub fn explode_projectiles(
    mut commands: Commands,
    atlas: Res<TowerAtlas>,
    mut gold: ResMut<Gold>,
    projectiles: Query<(Entity, &Transform, &Projectile), (With<Exploding>, Without<Enemy>)>,
    mut enemies: Query<(Entity, &Transform, &mut Health, &Bounty), (With<Enemy>, Without<Exploding>)>,
) {
    for (proj_entity, proj_transform, projectile) in projectiles.iter() {
        let pos = proj_transform.translation.truncate();

        for (enemy_entity, enemy_transform, mut health, bounty) in enemies.iter_mut() {
            if pos.distance(enemy_transform.translation.truncate()) <= projectile.splash_radius {
                if health.0 > 0.0 {
                    health.0 -= projectile.damage;
                    if health.0 <= 0.0 {
                        gold.0 += bounty.0 as f32;
                        commands.entity(enemy_entity).despawn();
                    }
                }
            }
        }

        commands.spawn((
            GameEntity,
            DespawnTimer(Timer::from_seconds(0.15, TimerMode::Once)),
            Sprite::from_atlas_image(atlas.texture.clone(), /* sprite 21 */),
            Transform::from_xyz(pos.x, pos.y, 2.4),
        ));

        commands.entity(proj_entity).despawn();
    }
}
```

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
