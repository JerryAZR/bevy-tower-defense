# Part 9: Targeting and Damage — The Core Loop

> **Time to read:** ~30 minutes  
> **New concepts:** `Timer` / `TimerMode`, query disjointness with `Without<T>`, parent/child hierarchy, snapshot pattern for scoped borrows  
> **Prerequisite:** Part 8 (tower placement with click-to-build)

---

## Recap: What We Already Have

We can place towers on grass tiles by clicking. Each tower consists of a static base and a separate turret entity. Enemies walk the path, but towers do nothing — they sit on the map as inert decorations.

---

## Goal: What We Will Build

We will make towers shoot enemies, completing the core game loop:

1. **Enemies get hit points** — a `Health` component tracks remaining HP.
2. **Turrets get attack stats** — range, damage, and a cooldown timer.
3. **One system handles targeting, rotation, and damage** — `attack_enemies` finds the nearest target, rotates the turret, and applies damage on cooldown.
4. **Inline despawn** — when health drops to zero, the enemy is removed immediately within the same system iteration.
5. **Muzzle flash feedback** — two fire sprites briefly appear at the turret barrels when it fires.

No projectiles. Damage is applied instantly. This keeps the system focused on the targeting and damage loop without the additional complexity of projectile physics and collision.

---

## New Bevy APIs & Concepts

### `Timer` and `TimerMode`

Bevy provides `Timer` for time-based state machines. A `Timer` counts down from a configured duration and can be checked with `just_finished()` — which returns `true` on exactly one frame when the timer expires.

`TimerMode` controls what happens after expiry:
- **`TimerMode::Once`** — the timer stops and stays finished.
- **`TimerMode::Repeating`** — the timer automatically resets and starts counting down again.

For a turret attack cooldown, `TimerMode::Repeating` is ideal: every time `just_finished()` returns `true`, the turret fires and the timer immediately begins its next cycle.

**Pitfall:** Forgetting to call `tick(time.delta())` every frame. A timer that is never ticked never finishes.

### Query disjointness with `Without<T>`

Bevy's scheduler validates that no two queries in the same system borrow the same component mutably. Even when no entity could possibly match both queries, Bevy cannot prove this from the types alone.

Consider a system with two queries:
- `Query<&mut Transform, With<TowerTurret>>`
- `Query<&mut Transform, With<Enemy>>`

No entity has both `TowerTurret` and `Enemy`, but both queries request `&mut Transform`. Bevy rejects this with a `B0001` query conflict error at runtime.

The fix is `Without<T>` filters. `Without<Enemy>` on the turret query plus `With<Enemy>` on the enemy query is already enough to prove disjointness — no entity can match both. We add `Without<TowerTurret>` to the enemy query as well for symmetry and clarity:

- `Query<&mut Transform, (With<TowerTurret>, Without<Enemy>)>`
- `Query<&mut Transform, (With<Enemy>, Without<TowerTurret>)>`

These filters make the structural disjointness explicit. Bevy's validator can now see that the two `Transform` borrows cannot overlap.

### Parent/child hierarchy

Bevy supports entity hierarchies via `Parent` and `Children` components. When you call `commands.entity(parent).with_children(|children| { ... })`, the spawned entities automatically get `Parent(parent)`.

Hierarchies are useful for:
- Attaching visual effects to moving entities (muzzle flashes on turrets).
- Grouping related sprites (tower base + turret).
- Auto-cleanup: despawning a parent despawns all its children recursively.

**Pitfall:** A parent entity must have `Transform` and `Visibility` components for child rendering to work correctly. Without them, Bevy's propagation systems skip the children and may issue `B0004` warnings.

### Snapshot pattern for scoped borrows

A Bevy query borrow lasts until the query variable is dropped. If you iterate over enemies to find the nearest one, then try to mutate that same enemy's health through the same query, the borrow checker complains — the iterator still holds a borrow.

The snapshot pattern solves this:
1. **Read** all needed data into a temporary `Vec` (the snapshot).
2. The original query borrow is released when the snapshot is complete.
3. **Write** back through the original query using `get_mut(entity)`.

This is the standard ECS idiom for "find something, then modify it" workflows. If you have seen Java's `ConcurrentModificationException` or C#'s "Collection was modified" errors, the underlying problem is the same: iterating and mutating the same container simultaneously is dangerous. Rust simply catches this at compile time through ownership rather than throwing a runtime exception.

---

## Walkthrough

### Designing the feature

Before writing code, let's decide how targeting and damage should work.

**Player-visible behavior:**

1. Turrets automatically find the nearest enemy within range.
2. The turret rotates to face that enemy continuously, even between shots.
3. Every 0.5 seconds, the turret fires: the enemy takes 34 damage.
4. When an enemy's health reaches zero, it disappears immediately.
5. A brief muzzle flash appears at the turret barrels when firing.

**Design decision: one combined system.** A common impulse is to split targeting into three separate systems: rotate, damage, despawn. But these steps are tightly coupled — you already found the target to rotate, so it's right there to damage; you already applied damage, so you can check the result immediately. Separate systems would mean re-querying or using marker components to pass state between them. Combined, everything happens in one pass:

```
For each turret:
    Find nearest enemy in range
    If found: rotate toward it
    If found AND cooldown expired: deal damage
    If enemy killed: despawn
```

**ECS data needed:**

- `Health` component on enemies.
- `AttackRange`, `Damage`, and `AttackTimer` components on turrets.
- `MuzzleFlash` and `DespawnTimer` components for the visual feedback.
- `attack_enemies` system that queries turrets and enemies simultaneously.
- `despawn_timed` system for cleaning up expired muzzle flashes.

### Step 1: Add `Health` to enemies

Enemies need to survive more than one hit. Add a `Health` component to `src/enemy.rs`:

```rust
#[derive(Component)]
pub struct Health(pub f32);
```

`Health` is `pub` because the tower module needs to read and write it. No other enemy logic changes — path following and cleanup are unaffected by the presence of this component.

### Step 2: Add attack stats to turrets

In `src/tower.rs`, add three new components for turret combat stats:

```rust
#[derive(Component)]
pub(crate) struct AttackRange(pub f32);

#[derive(Component)]
pub(crate) struct Damage(pub f32);

#[derive(Component)]
pub(crate) struct AttackTimer(pub Timer);
```

These are `pub(crate)` because only the tower module and `main.rs` need to see them. We also add constants at the top of the module:

```rust
const ATTACK_RANGE: f32 = 192.0;   // 3 tiles (64 * 3)
const DAMAGE: f32 = 34.0;          // 3 hits to kill a 100 HP enemy
const ATTACK_COOLDOWN: f32 = 0.5;  // fires twice per second
```

> **Why a `Timer` component instead of a resource?** Each turret has its own independent cooldown. A global timer would mean all turrets fire in unison. Wrapping `Timer` in a component lets every turret track its own state.

### Step 3: Add muzzle flash components

Still in `src/tower.rs`, we need components for the visual feedback system:

```rust
#[derive(Component)]
pub(crate) struct DespawnTimer(pub Timer);

#[derive(Component)]
pub(crate) struct MuzzleFlash;
```

`DespawnTimer` is a reusable component — any entity with it will be auto-despawned when its timer expires. `MuzzleFlash` is a marker so we could query for flashes specifically if needed later.

We also add a constant for the fire sprite index and flash duration:

```rust
const FIRE_SPRITE: usize = 295;
const MUZZLE_FLASH_DURATION: f32 = 0.15;
```

### Step 4: Implement `attack_enemies`

This is the core system. It needs to:

1. **Query turrets** — find all entities with `TowerTurret`, `Transform`, `AttackTimer`, `Damage`, and `AttackRange`.
2. **Query enemies** — find all entities with `Enemy`, `Transform`, `Health`, and `PathFollower` (we exclude base-reachers).
3. **Snapshot enemy positions** — collect `(Entity, Vec2)` pairs so we can search for nearest without holding the enemies borrow.
4. **Per-turret loop** — tick cooldown, find nearest enemy in range, rotate toward it.
5. **Deal damage** — if cooldown expired, reduce target health and despawn if dead.
6. **Spawn muzzle flash** — create a parent entity with a `DespawnTimer`, then attach two fire sprites as children.

What does it query?
- `Res<Time>` — to tick timers.
- `Res<TowerAtlas>` — to spawn muzzle flash sprites.
- `Query<(Entity, &mut Transform, &mut AttackTimer, &Damage, &AttackRange), (With<TowerTurret>, Without<Enemy>)>` — turrets.
- `Query<(Entity, &Transform, &mut Health), (With<Enemy>, With<PathFollower>, Without<TowerTurret>)>` — active enemies.
- `Commands` — to despawn dead enemies and spawn flashes.

Notice the `Without<Enemy>` and `Without<TowerTurret>` filters. Both queries touch `Transform` — turrets mutably (we rotate them), enemies read-only (we just read positions). Without these filters, Bevy's schedule validator raises a `B0001` query conflict because it cannot prove the queries are disjoint from the types alone.

```rust
pub fn attack_enemies(
    time: Res<Time>,
    atlas: Res<TowerAtlas>,
    mut turrets: Query<(
        Entity, &mut Transform, &mut AttackTimer, &Damage, &AttackRange
    ), (With<TowerTurret>, Without<Enemy>)>,
    mut enemies: Query<(
        Entity, &Transform, &mut Health
    ), (With<Enemy>, With<PathFollower>, Without<TowerTurret>)>,
    mut commands: Commands,
) {
    // Snapshot enemy positions, then release the query borrow.
    let enemy_positions: Vec<(Entity, Vec2)> = enemies
        .iter()
        .map(|(e, t, _)| (e, t.translation.truncate()))
        .collect();

    for (turret_entity, mut turret_transform, mut timer, damage, range) in turrets.iter_mut() {
        timer.0.tick(time.delta());
        let turret_pos = turret_transform.translation.truncate();

        // Find nearest enemy within range (linear scan).
        let mut nearest: Option<(Entity, f32)> = None;
        for &(entity, pos) in &enemy_positions {
            let dist = turret_pos.distance(pos);
            if dist <= range.0 {
                if nearest.map_or(true, |(_, best)| dist < best) {
                    nearest = Some((entity, dist));
                }
            }
        }

        if let Some((target, _)) = nearest {
            // Rotate toward target.
            let direction = enemy_positions.iter()
                .find(|(e, _)| *e == target)
                .map(|(_, p)| *p - turret_pos)
                .unwrap_or(Vec2::X);
            let angle = direction.y.atan2(direction.x) - std::f32::consts::FRAC_PI_2;
            turret_transform.rotation = Quat::from_rotation_z(angle);

            // Deal damage if cooldown expired.
            if timer.0.just_finished() {
                if let Ok((_, _, mut health)) = enemies.get_mut(target) {
                    health.0 -= damage.0;
                    if health.0 <= 0.0 {
                        commands.entity(target).despawn();
                    }
                }

                // Spawn muzzle flash as children of the turret.
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
                            flash_children.spawn((
                                Sprite::from_atlas_image(
                                    texture.clone(),
                                    TextureAtlas { layout: layout.clone(), index: FIRE_SPRITE },
                                ),
                                Transform::from_xyz(x_offset, 32.0, 2.2),
                            ));
                        }
                    });
                }
            }
        }
    }
}
```

Let's walk through the key sections:

**Snapshot:** We collect `(Entity, Vec2)` for all enemies into a `Vec`. The `map` discards `&mut Health` — we don't need it for the search. The `collect()` drops the iterator, releasing the `enemies` query borrow.

**Nearest-enemy search:** A linear scan over the snapshot. We track the closest enemy whose distance is within `range.0`. For a demo with few enemies, this is fast and simple. A larger game might use a spatial hash or quadtree.

**Rotation:** The turret sprite faces up by default, while the enemy sprite faces right. We subtract `π/2` from the angle to align the turret correctly. The turret tracks the target every tick, producing smooth continuous rotation even between shots.

**Damage:** `timer.0.just_finished()` returns `true` on exactly one frame per cooldown cycle. `enemies.get_mut(target)` re-borrows the specific enemy from the query so we can mutate its `Health`. If health drops to zero, we queue a despawn with `commands.entity(target).despawn()`.

> **Note:** `despawn()` does not execute immediately — it queues the command for the end of the frame. If two turrets target the same enemy and both fire on the same frame, both turrets successfully borrow the enemy (it still exists during the system), both reduce its health, and both queue a despawn. Bevy flushes commands after the system finishes; the first despawn succeeds, and the second triggers a console warning because the entity no longer exists. A guard against zero-health enemies is added in a later part.

**Muzzle flash:** We spawn a parent marker entity with `DespawnTimer` as a child of the turret, then spawn two fire sprites as children of that marker. When the timer expires, `despawn_timed` despawns the marker, and Bevy's hierarchy system recursively removes the fire sprites. The parent entity needs `Transform::default()` and `Visibility::default()` — without them, child rendering fails.

The `flash_id` two-step pattern avoids a borrow conflict: `commands` is already borrowed by the first `with_children` closure, so we capture the spawned ID and spawn children in a second call.

### Step 5: Add `despawn_timed`

This system cleans up any entity with an expired `DespawnTimer`. It runs in `Update` (visual cleanup, not gameplay logic):

```rust
pub fn despawn_timed(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut DespawnTimer)>,
) {
    for (entity, mut timer) in query.iter_mut() {
        timer.0.tick(time.delta());
        if timer.0.just_finished() {
            commands.entity(entity).despawn();
        }
    }
}
```

This is intentionally generic — any entity with a `DespawnTimer` gets cleaned up. In a larger game you might want separate timers for different effect types, but a single system is enough for muzzle flashes.

### Step 6: Update spawn systems

Add `Health(100.0)` to the enemy spawn in `spawn_test_enemy`:

```rust
commands.spawn((
    // ...existing components...
    MoveSpeed(192.0),
    Health(100.0),
));
```

Add attack components to the turret spawn in `place_tower_on_click`:

```rust
commands.spawn((
    TowerTurret,
    // ...sprite and transform...
    AttackRange(ATTACK_RANGE),
    Damage(DAMAGE),
    AttackTimer(Timer::from_seconds(ATTACK_COOLDOWN, TimerMode::Repeating)),
));
```

`TimerMode::Repeating` means the timer automatically resets after each `just_finished()` tick. The turret fires every 0.5 seconds indefinitely.

### Step 7: Wire everything in `main.rs`

Import the new systems:

```rust
use tower::{
    PlacedTowers, setup_tower_atlas, spawn_placement_preview,
    update_placement_preview, place_tower_on_click,
    attack_enemies, despawn_timed,
};
```

Add `attack_enemies` to the `FixedUpdate` chain, and `despawn_timed` to `Update`:

```rust
.add_systems(FixedUpdate, (
    move_enemies,
    attack_enemies,
    cleanup_finished_enemies,
).chain())
.add_systems(Update, (
    update_placement_preview,
    place_tower_on_click,
    despawn_timed,
))
```

**Why `.chain()` in `FixedUpdate`?** All three systems run at a fixed timestep. `move_enemies` updates positions and removes `PathFollower` from base-reachers. `attack_enemies` then targets remaining path-followers. `cleanup_finished_enemies` finally sweeps up anything that reached the base. Without `.chain()` the order is unspecified. In this case the difference between "this tick vs next tick" is usually invisible, but explicit ordering makes behavior predictable and simplifies reasoning.

| Order | System | Role |
|---|---|---|
| 1 | `move_enemies` | Move enemies; remove `PathFollower` from base-reachers |
| 2 | `attack_enemies` | Turrets target and damage enemies still on the path |
| 3 | `cleanup_finished_enemies` | Despawn enemies that reached the base |

### Step 8: Verify

```bash
cargo run
```

You should see:

- The same `15×10` map, enemy, and click-to-place from Part 8.
- **Place a tower** on a grass tile near the path.
- The turret **rotates to track** the enemy as it walks past.
- Every 0.5 seconds the turret fires — the enemy takes 34 damage. A **muzzle flash** (two fire sprites at the barrels) briefly appears.
- After 3 hits (102 damage ≥ 100 HP), the enemy **disappears** while walking.
- If the enemy survives past the tower, it still reaches the base and is cleaned up as before.

### Simplifications

| Simplification | Why it works | Future direction |
|---|---|---|
| **Instant damage (no projectiles)** | Projectiles need spawn, flight time, and collision. Instant damage completes the loop with less code. | A future part will add projectile sprites that fly from turret to target. |
| **One combined system** | Rotation, damage, and despawn are tightly coupled. Separate systems would require re-querying or marker components. | A larger game might split targeting (shared across tower types) from damage (type-specific). |
| **Nearest-enemy targeting** | Simple and fast for few enemies. | Future parts may add lowest-health priority, nearest-to-base priority, or different target selection per tower type. |
| **Linear scan for targeting** | O(n) per turret is fine with one enemy and one tower. | A dense swarm would need a spatial hash or quadtree for O(log n) or O(1) neighbor lookups. |
| **No health bars** | Enemies die in 3 hits; the player can infer health from hit count. | UI health bars or damage numbers in a future part. |
| **Muzzle flash as children** | Parent/child auto-cleanup means zero despawn logic for the fire sprites themselves. | Complex effects (particle bursts, screen shake) would need dedicated effect systems. |

---

## Summary

- We added `Health(100.0)` to enemies so they can be damaged and killed.
- We added `AttackRange`, `Damage`, and `AttackTimer` components to turrets for combat stats.
- We built `attack_enemies` — one system that snapshots enemy positions, finds the nearest target per turret, rotates toward it, and applies instant damage on a repeating cooldown.
- We despawned dead enemies inline with `commands.entity(target).despawn()`, avoiding a separate cleanup system.
- We used `Without<Enemy>` / `Without<TowerTurret>` filters to make query disjointness explicit, preventing `B0001` conflicts.
- We chained the three `FixedUpdate` systems (`move_enemies` → `attack_enemies` → `cleanup_finished_enemies`) for predictable tick-to-tick behavior.
- We added **muzzle flash** visual feedback using parent/child hierarchy and a generic `despawn_timed` cleanup system.

The game loop is now complete: enemies spawn, walk the path, towers shoot them down (or they reach the base). In **Part 10** we will replace the single test enemy with a data-driven wave system — multiple enemy types, timed waves, and overlapping spawns, all defined in the level TOML.
