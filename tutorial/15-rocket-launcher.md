# Part 15: Rocket Launcher — Projectiles, Ammo, and Splash Damage

> **Time to read:** ~20 minutes  
> **New concepts:** `GlobalTransform`, entity references in components  
> **Prerequisite:** Part 14 (gold economy)

---

## Recap: What We Already Have

The game loop is complete: enemies walk the path, towers shoot them, gold gates placement, and win/lose conditions transition to the GameOver screen. But every tower behaves identically — instant damage, no variety, no visual payoff when an enemy dies.

---

## Goal: What We Will Build

1. **Rocket launcher tower** — replaces the instant-damage tower with a barrel that rotates toward enemies and fires visible projectiles.
2. **Ammo slots** — three rockets sit on the barrel, depleting when fired and refilling over time.
3. **Homing projectiles** — rockets track their target enemy and rotate to face it in flight.
4. **Splash damage** — one rocket can damage multiple clustered enemies within a radius.
5. **Explosion visual** — a short-lived sprite spawns on impact and auto-despawns.

This matters because variety is what makes tower defense interesting. Different tower types with distinct mechanics force the player to make strategic choices about placement and timing.

---

## New Bevy APIs & Concepts

### `GlobalTransform`

`Transform` stores an entity's position *relative to its parent* (local space). `GlobalTransform` stores the computed position in world space — exactly what you need when a child entity's visual position matters independently of its parent.

In our case, ammo-slot rockets are children of the launcher barrel. Their `Transform` is an offset like `(-12, 8)` from the barrel center. To spawn a projectile that starts where the slot *visually* is, we read `GlobalTransform::translation()`, which includes the barrel's rotation and position.

> **Pitfall:** Querying only `Transform` on a child gives local coordinates. If you spawn a new entity at that position, it appears at the wrong place in world space. Always use `GlobalTransform` when you need world-space coordinates of children.

### Storing `Entity` in a component

A component can store an `Entity` ID — a direct reference to another entity in the world. This is not a query; it is a pointer. Each frame, the system that owns the component looks up the referenced entity's current `Transform` to update behavior.

Our `Projectile` component stores `target: Entity` — the enemy it is chasing. `move_projectiles` looks up that enemy's position every frame. If the enemy is despawned (killed by another tower), the lookup returns `Err`, and the projectile continues toward its last-known position instead of panicking.

> **Pitfall:** Storing `Entity` IDs creates a dangling-reference risk. If the target is despawned, the ID becomes invalid. Always handle `Query::get(entity)` with `if let Ok(...)`, not `unwrap()`.

---

## Walkthrough

### Designing the feature

**Player-visible behavior:**

1. A tower with a static base and a rotating barrel points toward the nearest enemy.
2. Three small rockets sit in visible slots on the barrel.
3. When the launcher fires, one rocket detaches and flies toward the target as a homing projectile.
4. The slot that fired becomes empty.
5. After a short delay, a new rocket reappears in the first empty slot.
6. When the projectile hits, it explodes and damages all enemies within a radius.
7. A brief explosion sprite appears at the impact point.

**ECS data needed:**

- `RocketLauncher` tag component — distinguishes launcher barrels from other entities.
- `AmmoSlots` component — `Vec<Option<Entity>>` tracking which slot positions have a rocket child.
- `AmmoRegenTimer` component — controls refill timing.
- `Projectile` component — stores target `Entity`, speed, damage, and splash radius.
- `Exploding` tag component — marks projectiles that have reached their target.
- `GlobalTransform` query — to get world-space spawn positions for projectiles.
- Sprite indices and timing constants for the launcher base, barrel, rocket, and explosion.

**Design decision: why keep `attack_enemies` registered?** The old instant-damage tower uses `TowerTurret` + `Damage`. The rocket launcher uses `RocketLauncher` + `AmmoSlots`. Both can coexist in the same schedule because no entity has both `TowerTurret` and `RocketLauncher` tags, and the `FixedUpdate` chain executes systems sequentially. For Part 15 we only spawn rocket launchers, so `attack_enemies` finds no matching entities and is effectively a no-op. We keep it registered so re-adding the old tower later is just a UI change.

---

### Step 1: Preserve the old tower as a helper

The instant-damage tower from Part 9 is still useful — we will let the player choose between tower types in a future part. Extract its spawn logic into a helper so the old code stays available without cluttering the click handler.

In `src/tower.rs`, add `spawn_instant_tower`:

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

This function takes `Commands` and `TowerAtlas` by reference so it can be called from `place_tower_on_click` without consuming those values.

---

### Step 2: Add rocket launcher components and constants

Add the new types to `src/tower.rs`. What do we need?

- `RocketLauncher` — tag component for launcher barrels.
- `AmmoSlots` — tracks which of the three slot positions currently has a rocket child entity.
- `AmmoRegenTimer` — controls how quickly empty slots refill.
- `Projectile` — stores homing target, speed, damage, and splash radius.
- `Exploding` — tag component for projectiles that reached their target.

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
pub(crate) struct Projectile {
    pub target: Entity,
    pub target_position: Vec2,
    pub speed: f32,
    pub damage: f32,
    pub splash_radius: f32,
}

#[derive(Component)]
pub(crate) struct Exploding;
```

And the constants:

```rust
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

`AMMO_SLOT_OFFSETS` places the three rockets slightly above the barrel center. When the barrel rotates, the child rockets rotate with it because they are attached as children.

---

### Step 3: Spawn the rocket launcher

`spawn_rocket_launcher` in `src/tower.rs` does three things:

1. **Spawn the static base** — a `Tower` entity with the base sprite at the tile center.
2. **Spawn the rotating barrel** — a `RocketLauncher` entity with the barrel sprite, `AttackRange`, `AttackTimer`, `AmmoRegenTimer`, and an empty `AmmoSlots`.
3. **Attach three rocket children** — using `with_children`, spawn a rocket sprite at each offset and capture its `Entity` ID.

The tricky part is step 3. The `with_children` closure receives a `ChildBuilder` scoped to spawning children — it cannot modify components on the parent entity. Instead we collect the child IDs into a local `Vec`, then `insert` the completed `AmmoSlots` after the closure returns.

```rust
fn spawn_rocket_launcher(commands: &mut Commands, atlas: &TowerAtlas, pos: Vec2) {
    commands.spawn((
        Tower,
        GameEntity,
        Sprite::from_atlas_image(
            atlas.texture.clone(),
            TextureAtlas { layout: atlas.layout.clone(), index: ROCKET_LAUNCHER_BASE },
        ),
        Transform::from_xyz(pos.x, pos.y, 2.0),
    ));

    let turret_entity = commands.spawn((
        RocketLauncher,
        GameEntity,
        Sprite::from_atlas_image(
            atlas.texture.clone(),
            TextureAtlas { layout: atlas.layout.clone(), index: ROCKET_LAUNCHER_BARREL },
        ),
        Transform::from_xyz(pos.x, pos.y, 2.1),
        AttackRange(ATTACK_RANGE),
        AttackTimer(Timer::from_seconds(ATTACK_PAUSE_SECS, TimerMode::Repeating)),
        AmmoRegenTimer(Timer::from_seconds(AMMO_REFILL_SECS, TimerMode::Repeating)),
        AmmoSlots { slots: vec![None; ROCKET_MAX_AMMO as usize] },
    )).id();

    let mut slot_entities = vec![None; ROCKET_MAX_AMMO as usize];
    commands.entity(turret_entity).with_children(|turret_children| {
        for i in 0..ROCKET_MAX_AMMO {
            let offset = AMMO_SLOT_OFFSETS[i as usize];
            let slot_entity = turret_children.spawn((
                Sprite::from_atlas_image(
                    atlas.texture.clone(),
                    TextureAtlas { layout: atlas.layout.clone(), index: ROCKET_SPRITE },
                ),
                Transform::from_xyz(offset.0, offset.1, 2.2),
            )).id();
            slot_entities[i as usize] = Some(slot_entity);
        }
    });
    commands.entity(turret_entity).insert(AmmoSlots { slots: slot_entities });
}
```

`AmmoSlots` is inserted twice: first with all `None` in the spawn bundle, then overwritten with the actual child entity IDs after `with_children` completes. The overwrite is necessary because the closure cannot modify components on the parent entity.

---

### Step 4: Update the placement preview

The preview sprites should show the new tower so the player knows what they are placing. In `spawn_placement_preview`, replace the old `TOWER_BASE` / `TOWER_TOP` indices with the rocket launcher sprites:

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

The preview does not show ammo rockets — those are only visible on placed towers. The preview is intentionally minimal so it does not obscure the map.

---

### Step 5: Wire up placement

In `place_tower_on_click`, replace the old inline spawn with a call to `spawn_rocket_launcher`:

```rust
// For Part 15 only the rocket launcher is spawnable.
spawn_rocket_launcher(&mut commands, &atlas, pos);
```

> **Run the game now.** You should be able to place rocket launchers on grass tiles. They show the base, barrel, and three rockets — but they do not fire yet. The launcher does **not** have a `Damage` component and is tagged `RocketLauncher` instead of `TowerTurret`, so the old `attack_enemies` system ignores it completely.

---

### Step 6: Fire rockets

`launch_rockets` is a `FixedUpdate` system that handles targeting, barrel rotation, and firing. Instead of dealing instant damage like the old tower, it checks `AmmoSlots` for the first filled position, reads the child's `GlobalTransform` for the world-space spawn point, despawns the visual rocket child, and spawns a new `Projectile` entity.

What does it query?
- `Query<(Entity, &mut Transform, &mut AttackTimer, &AttackRange, &mut AmmoSlots), (With<RocketLauncher>, Without<Enemy>)>` — launcher barrels. `Without<Enemy>` is a standard disjointness filter.
- `Query<(Entity, &Transform), (With<Enemy>, With<PathFollower>, Without<TowerTurret>, Without<RocketLauncher>)>` — read-only snapshot of enemy positions for targeting.
- `Query<&GlobalTransform>` — broad read-only query to look up ammo slot child world positions by `Entity` ID.
- `Commands` — to despawn consumed ammo and spawn projectiles.

The algorithm:
1. Snapshot enemy positions.
2. For each launcher, find the nearest enemy within `AttackRange`.
3. Rotate the barrel to face the target.
4. When `AttackTimer` fires, find the first filled slot in `AmmoSlots`.
5. Read the slot child's `GlobalTransform` for the world-space spawn position.
6. Remove the slot from `AmmoSlots` and despawn the child rocket sprite.
7. Spawn a `Projectile` entity at the spawn position, aimed at the target.

See `pub fn launch_rockets` in `src/tower.rs` for the full implementation.

> **Why `GlobalTransform`?** Ammo-slot rockets are children of the barrel. Their local `Transform` is an offset like `(-12, 8)`. If we spawned the projectile at that local offset, it would appear near the world origin, not near the launcher. `GlobalTransform::translation()` includes the barrel's position and rotation, so the projectile spawns exactly where the slot visually is.

---

### Step 7: Move and home projectiles

`move_projectiles` is a `FixedUpdate` system that updates every projectile each tick.

What does it query?
- `Query<(Entity, &mut Transform, &mut Projectile), (Without<Exploding>, Without<Enemy>)>` — active projectiles. `Without<Exploding>` excludes projectiles already tagged for explosion.
- `Query<&Transform, (With<Enemy>, Without<Exploding>, Without<Projectile>)>` — read-only lookup of enemy positions for homing.

Each frame, for every projectile:
1. **Homing** — look up the target enemy by its stored `Entity` ID. If alive, update `projectile.target_position` to the enemy's current position. If dead, the lookup returns `Err` and the projectile continues to its last-known position.
2. **Rotation** — compute the angle to `target_position` and set `transform.rotation` so the rocket sprite faces its destination.
3. **Movement** — advance toward the target at `ROCKET_SPEED` pixels per second. If the remaining distance is less than one frame's travel, snap to the target and insert the `Exploding` tag. Otherwise continue moving.

See `pub fn move_projectiles` in `src/tower.rs` for the full implementation.

> **Why the `Without` filters?** Both queries access `Transform` and one writes to it, so Bevy needs proof they do not overlap. `Without<Enemy>` on the projectile query paired with `With<Enemy>` on the enemy query is what makes them disjoint. The additional `Without<Exploding>` and `Without<Projectile>` filters are not required for the proof; they are added for clarity.

---

### Step 8: Explode projectiles

`explode_projectiles` is a `FixedUpdate` system that handles splash damage and visuals for all projectiles tagged `Exploding`.

What does it query?
- `Query<(Entity, &Transform, &Projectile), (With<Exploding>, Without<Enemy>)>` — read-only; needs position and damage data.
- `Query<(Entity, &Transform, &mut Health, &Bounty), (With<Enemy>, Without<Exploding>)>` — enemies within splash range.
- `ResMut<Gold>` — to award kill bounties.
- `Res<TowerAtlas>` — for the explosion sprite.
- `Commands` — to spawn explosion visuals and despawn projectiles.

For each `Exploding` projectile:
1. Use the projectile's position as the explosion center.
2. Iterate all enemies and check if they are within `splash_radius`.
3. Subtract `projectile.damage` (set to `ROCKET_DAMAGE` when the projectile is spawned) from each enemy's `Health`.
4. Before applying damage, check `health.0 > 0.0`. This guard prevents two overlapping explosions in the same frame from damaging an already-dead enemy twice (which would award double bounty).
5. Spawn a short-lived explosion sprite with a `DespawnTimer` of 0.15 s.
6. Despawn the projectile.

See `pub fn explode_projectiles` in `src/tower.rs` for the full implementation.
Add `launch_rockets`, `move_projectiles`, and `explode_projectiles` to the `FixedUpdate` chain in `src/main.rs` before testing.

> **Run the game now.** Rocket launchers should track enemies, fire homing rockets, and deal splash damage on impact. Watch the ammo slots — each shot removes one rocket. After three shots the launcher goes silent (ammo refill is the next step).

---

### Step 9: Refill ammo

`refill_ammo` is a `FixedUpdate` system that restores empty ammo slots over time.

What does it query?
- `Query<(Entity, &mut AmmoRegenTimer, &mut AmmoSlots), With<RocketLauncher>>` — launchers that may need ammo.
- `Commands` — to spawn new rocket children.
- `Res<TowerAtlas>` — for the rocket sprite.

For each launcher, tick the `AmmoRegenTimer`. When it fires, find the first empty slot (`None`), spawn a new rocket sprite child at the corresponding `AMMO_SLOT_OFFSET`, and store the child's `Entity` ID in the slot. The child is attached via `with_children`, so it inherits the barrel's position and rotation.

See `pub fn refill_ammo` in `src/tower.rs` for the full implementation.
Add `refill_ammo` to the `FixedUpdate` chain in `src/main.rs` as well.

Now the full loop is visible: rockets deplete on fire, then slowly reappear one by one.

---

### Step 10: Register systems and adjust cost

In `src/main.rs`, add the four new systems to the `FixedUpdate` chain. Keep `attack_enemies` — with no `TowerTurret` entities in the world, it is a no-op:

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

Also bump `TOWER_COST` in `economy.rs` from `100` to `150` to match the rocket launcher's higher power:

```rust
pub const TOWER_COST: u32 = 150;
```

---

### Simplifications

| Simplification | Why it works | Future direction |
|---|---|---|
| **Hardcoded constants** | Damage, speed, splash radius, and ammo count are compile-time constants. | Store these in a `TowerType` definition loaded from a config file. |
| **One tower type spawnable** | Only the rocket launcher is placed. The old instant-damage helper is preserved but unused. | A tower selection UI lets the player choose before placing. |
| **No projectile pooling** | Every rocket is spawned and despawned. | For hundreds of simultaneous projectiles, use an object pool or a `Vec<Projectile>` buffer. |
| **No collision detection** | Splash damage uses distance checks, not a physics engine. | A physics-based approach (rapier, avian) for complex hitboxes. |
| **Linear ammo refill** | One rocket restored every 2 seconds, in slot order. | Burst refill, conditional refill (e.g., on kill), or ammo crates. |

---

## Summary

- We replaced the instant-damage tower with a **rocket launcher** that has visible ammo slots on its barrel.
- We added **`AmmoSlots`** and **`AmmoRegenTimer`** to implement a refill system: 3 rockets max, one restored every 2 seconds.
- We created a **`Projectile`** component that stores a target `Entity` for homing; `move_projectiles` updates the destination each frame and rotates the rocket sprite to face it.
- We used **`GlobalTransform`** to spawn projectiles at the exact world-space position of an ammo-slot child, so detachment looks natural.
- We built **`explode_projectiles`** to apply splash damage to all enemies in a 60 px radius, with a `health.0 > 0.0` guard to prevent double-damage from overlapping explosions.
- We kept `attack_enemies` registered and preserved `spawn_instant_tower` so re-adding the old tower later requires only UI wiring.

Future parts may add a tower selection UI, data-driven tower definitions, or additional tower types with unique mechanics.
