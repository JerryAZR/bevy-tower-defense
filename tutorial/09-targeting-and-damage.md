# Part 9: Targeting and Damage — The Core Loop

Part 8 let us click to place towers. Now those towers do something: rotate toward enemies and deal damage. With this, the game loop becomes playable — enemies spawn, walk the path, towers shoot, enemies die.

---

## What we will build

- **Enemies get hit points** — a `Health` component tracks remaining HP.
- **Turrets get attack stats** — range, damage, and a cooldown timer.
- **One system handles everything** — `attack_enemies` rotates turrets toward the nearest target and applies instant damage.
- **Inline despawn** — when health drops to zero, the enemy is removed immediately within the same system iteration.

No projectiles. No tower types. Just a clean targeting + damage loop.

---

## Why one combined system?

A common impulse is to split targeting into three systems: rotation, damage, despawn. But these steps are tightly coupled:

1. You already found the target to rotate — it's right there to damage.
2. You already applied damage — you can check the result immediately.

Separate systems mean re-querying or using marker components to pass state between them. Combined, everything happens in one pass:

```
For each turret:
    Find nearest enemy in range
    If found: rotate toward it
    If found AND cooldown expired: deal damage
    If enemy killed: despawn
```

No intermediate state. No command-buffer races. Clean.

---

## New components

### `Health` — on enemies (`src/enemy.rs`)

```rust
#[derive(Component)]
pub struct Health(pub f32);
```

No change to existing enemy logic. Path following and cleanup are unaffected.

### Attack stats — on turrets (`src/tower.rs`)

```rust
#[derive(Component)]
pub(crate) struct AttackRange(pub f32);

#[derive(Component)]
pub(crate) struct Damage(pub f32);

#[derive(Component)]
pub(crate) struct AttackTimer(pub Timer);
```

Placed on each `TowerTurret` entity. Constants at the module top:

```rust
const ATTACK_RANGE: f32 = 192.0;   // 3 tiles (64 * 3)
const DAMAGE: f32 = 34.0;          // 3 hits to kill a 100 HP enemy
const ATTACK_COOLDOWN: f32 = 0.5;  // fires twice per second
```

---

## The `attack_enemies` system

```rust
pub fn attack_enemies(
    time: Res<Time>,
    atlas: Res<TowerAtlas>,
    mut turrets: Query<(Entity, &mut Transform, &mut AttackTimer, &Damage, &AttackRange), (With<TowerTurret>, Without<Enemy>)>,
    mut enemies: Query<(Entity, &Transform, &mut Health), (With<Enemy>, With<PathFollower>, Without<TowerTurret>)>,
    mut commands: Commands,
) {
```

### `Without<T>`: telling Bevy queries are disjoint

Both queries touch `Transform` — turrets mutably, enemies read-only. Even though no entity has *both* `TowerTurret` and `Enemy`, Bevy's schedule validator can't see that from the code. `Without<Enemy>` and `Without<TowerTurret>` make the structural disjointness explicit, preventing a `B0001` query conflict at runtime.

### Single query, scoped borrow

One enemies query with `&mut Health`. We snapshot positions into a `Vec<(Entity, Vec2)>` via `.iter().collect()` — the query borrow drops when `collect()` returns, freeing `enemies` for `get_mut()` calls later in the turret loop:

### Enemy position collection

```rust
let enemy_positions: Vec<(Entity, Vec2)> = enemies
    .iter()
    .map(|(e, t, _)| (e, t.translation.truncate()))
    .collect();
```

A snapshot of current enemy positions. The `_` discards `&mut Health` from the iterator — the query borrow drops when `collect()` returns, freeing `enemies` for `get_mut()` calls later.

### Per-turret loop

```rust
for (turret_entity, mut turret_transform, mut timer, damage, range) in turrets.iter_mut() {
    timer.0.tick(time.delta());
    let turret_pos = turret_transform.translation.truncate();
```

Tick the cooldown. Extract turret position.

### Nearest-enemy search

```rust
let mut nearest: Option<(Entity, f32)> = None;
for &(entity, pos) in &enemy_positions {
    let dist = turret_pos.distance(pos);
    if dist <= range.0 {
        if nearest.map_or(true, |(_, best)| dist < best) {
            nearest = Some((entity, dist));
        }
    }
}
```

Linear scan over the enemy position snapshot. Nearest-to-turret is the simplest targeting rule — works well for a demo with few enemies. More sophisticated logic (lowest-health, nearest-to-base) would be a future improvement.

### Rotation

```rust
if let Some((target, _)) = nearest {
    let direction = enemy_positions.iter()
        .find(|(e, _)| *e == target)
        .map(|(_, p)| *p - turret_pos)
        .unwrap_or(Vec2::X);
    let angle = direction.y.atan2(direction.x) - std::f32::consts::FRAC_PI_2;
    turret_transform.rotation = Quat::from_rotation_z(angle);
```

Same `atan2` + `Quat::from_rotation_z` pattern from Part 7, but with a `- π/2` offset because the turret sprite faces **up** while the enemy sprite faces **right**. Turret tracks the target every tick — smooth continuous rotation even between shots.

### Damage on cooldown

```rust
if timer.0.just_finished() {
    if let Ok((_, _, mut health)) = enemies.get_mut(target) {
        if health.0 > 0.0 {
            health.0 -= damage.0;
            if health.0 <= 0.0 {
                commands.entity(target).despawn();
            }
        }
    }
}
```

`get_mut` returns the full query tuple `(Entity, &Transform, &mut Health)`. We destructure with `(_, _, mut health)` to get just the `Health`. `just_finished()` returns `true` on exactly one tick per cooldown cycle.

The `health.0 > 0.0` guard prevents a **double-despawn** bug: when two turrets target the same enemy in the same tick, the first turret queues a despawn command, and the second skips the already-dead enemy instead of issuing a duplicate despawn that would trigger an entity-invalid warning.

Despawn is inlined — no separate system needed. Dead enemies with `PathFollower` still have it, and `cleanup_finished_enemies` won't touch them because they're already gone.

### Muzzle flash

When a turret fires, we spawn two fire sprite (#295) entities as children — visual feedback that a shot happened. They auto-despawn after 0.15 seconds.

We use Bevy's **parent/child hierarchy**: a marker entity with a `DespawnTimer` is added as a child of the turret, and the fire sprites are children of that marker. When the timer expires, `despawn_timed` despawns the marker — Bevy's hierarchy system recursively removes the fire sprites:

The spawn logic inside `attack_enemies` (on `just_finished()`):

```rust
// Spawn muzzle flash as children of the firing turret
let texture = atlas.texture.clone();
let layout = atlas.layout.clone();
let mut flash_id = None;
commands.entity(turret_entity).with_children(|turret_children| {
    flash_id = Some(turret_children.spawn((
        MuzzleFlash,
        DespawnTimer(Timer::from_seconds(MUZZLE_FLASH_DURATION, TimerMode::Once)),
        Transform::default(),
        Visibility::default(),
    )).id());
});
if let Some(flash_id) = flash_id {
    commands.entity(flash_id).with_children(|flash_children| {
        for i in 0..2 {
            let x_offset = if i == 0 { -6.0 } else { 6.0 };
            flash_children.spawn((Sprite::from_atlas_image(
                texture.clone(),
                TextureAtlas { layout: layout.clone(), index: FIRE_SPRITE },
            ),
                Transform::from_xyz(x_offset, 32.0, 2.2),
            ));
        }
    });
}
```

The `flash_id` two-step pattern avoids a borrow conflict: `commands` is already borrowed by the first `with_children` closure, so we capture the ID and spawn children in a second call.

The cleanup system:
```rust
// spawn_timed system — ticks timers, despawns expired entities
pub fn despawn_timed(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut DespawnTimer)>
 ) {
    for (entity, mut timer) in query.iter_mut() {
        timer.0.tick(time.delta());
        if timer.0.just_finished() {
            commands.entity(entity).despawn();
        }
    }
}
```

This runs in `Update` — it's purely visual cleanup, not gameplay logic.

**Heads up:** the `DespawnTimer` parent entity needs `Transform::default()` and `Visibility::default()` — without them, Bevy's `GlobalTransform`/`Visibility` propagation fails and children won't render or will trigger B0004 warnings.

## Changes to existing code

### `spawn_test_enemy` — adds `Health(100.0)`

```rust
commands.spawn((
    // ...existing components...
    MoveSpeed(192.0),
    Health(100.0),   // new
));
```

### `place_tower_on_click` — adds attack components to turret

```rust
commands.spawn((
    TowerTurret,
    // ...sprite...
    Transform::from_xyz(pos.x, pos.y, 2.1),
    AttackRange(ATTACK_RANGE),
    Damage(DAMAGE),
    AttackTimer(Timer::from_seconds(ATTACK_COOLDOWN, TimerMode::Repeating)),
));
```

The `AttackTimer` uses `TimerMode::Repeating` — automatically resets after each `just_finished()` tick.

---

## Wiring in `main.rs`

```rust
add_systems(FixedUpdate, (
    move_enemies,
    attack_enemies,
    cleanup_finished_enemies,
).chain())
```

### Why `.chain()`?

All three systems run in `FixedUpdate`. `move_enemies` updates positions and removes `PathFollower` from base-reachers. `attack_enemies` then targets remaining path-followers. `cleanup_finished_enemies` finally sweeps up anything that reached the base. Without `.chain()` the order is unspecified and could vary between runs. In simulation games the difference between "this tick vs next tick" is usually invisible, but an explicit order makes behavior predictable and simplifies reasoning:

| Order | System | Role |
|---|---|---|
| 1 | `move_enemies` | Move enemies; remove `PathFollower` from base-reachers |
| 2 | `attack_enemies` | Turrets target and damage enemies still on the path |
| 3 | `cleanup_finished_enemies` | Despawn enemies that reached the base |

---

## Running the project

```bash
cargo run
```

Expected behavior:

- Enemy spawns and walks the path as before.
- **Place a tower** on a grass tile near the path.
- The turret **rotates to track** the enemy as it walks past.
- Every 0.5 seconds the turret fires — the enemy takes 34 damage. A **muzzle flash** (two fire sprites at the barrels) briefly appears.
- After 3 hits (102 ≥ 100 HP), the enemy **disappears** while walking.
- If the enemy survives past the tower, it still reaches the base and is cleaned up as before.

---

## Design decisions

| Decision | Rationale |
|---|---|
| **One combined system** | Avoids re-query and command-buffer coordination between separate rotate/damage/despawn systems. |
| **`Without<T>` on both queries** | `Without<Enemy>` + `Without<TowerTurret>` proves the queries are structurally disjoint to Bevy's validator, preventing B0001. |
| **Snapshot enemy positions** | Prevents iterator invalidation if a despawn happens mid-loop. |
| **Inline despawn** | No separate dead-enemy system. No need for a marker component. Direct. |
| **`AttackTimer` wrapping `Timer`** | Self-documenting component. `timer.0` fields for `newtype` access. |
| **`With<PathFollower>` filter** | Excludes enemies that already reached the base. No wasted shooting. |
| **Turret always rotates** | Tracks target every tick, not just when firing. Looks polished. |
| **No spawn-tracking** | `attack_enemies` doesn't know or care about the spawn point. Purely distance-based. |
| **Chained systems** | Explicit `move_enemies → attack_enemies → cleanup` order makes tick-to-tick behavior predictable and simplifies reasoning. |
| **Muzzle flash as children** | Parent/child hierarchy auto-despawns flashes when the marker entity expires. Visual feedback with zero cleanup code. |
| **`despawn_timed` in `Update`** | Visual cleanup runs at render rate, not gameplay timestep. Keeps `FixedUpdate` focused on simulation. |

---

## Recap

In this part we:

1. Added `Health(100.0)` to enemies so they can be damaged.
2. Added `AttackRange`, `Damage`, and `AttackTimer` components to turrets.
3. Built `attack_enemies` — one system that finds the nearest enemy, rotates the turret, and applies instant damage on a cooldown.
4. Despawned dead enemies inline after damage, avoiding a separate dead-enemy system.
5. Used **single query with scoped borrow** — snapshot positions, then `get_mut` for damage.
6. Added **`Without<Enemy>` / `Without<TowerTurret>`** to make query disjointness explicit, satisfying Bevy's schedule validator.
7. **Chained** the three FixedUpdate systems in a deliberate order so behavior is predictable tick-to-tick.
8. Added **muzzle flash** visual feedback — two fire sprites as children of the turret, auto-despawned via `DespawnTimer` and `despawn_timed`.

The game loop is now complete: enemies spawn, walk the path, towers shoot them down (or they reach the base). Part 10 will add a population system: waves of enemies over time, potentially multiple enemies simultaneously.
