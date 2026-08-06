# Part N: Title — Subtitle

> **Time to read:** ~X minutes  
> **New concepts:** `TypeA`, `TypeB`, `pattern_name`  
> **Prerequisite:** Part N-1 (or a working project up to that point)

---

## Recap: What We Already Have

In one or two sentences, state where the project stands at the end of the previous part. This orients the reader and provides continuity across the series.

> Example:  
> We have a minimal Bevy app that opens a window, spawns a 2D camera, and renders a red square at the origin. Nothing moves yet, and there is no gameplay.

---

## Goal: What We Will Build

State clearly and concisely what the reader will accomplish in this part. Mention *why* it matters for the finished game.

- Bullet points work well for multi-step goals.
- Keep it high-level: the Walkthrough will handle the details.

> Example:  
> We will draw a grid-based map and give the player a way to scroll it. This teaches tile coordinates and camera control — the foundation of every tower defense level.

---

## New Bevy APIs & Concepts

Introduce the concepts the reader needs to understand *before* they encounter them in code. You do **not** need to exhaustively document every type used — only the ones the reader should *learn* in this part.

For each concept, cover:
1. **What it is** (one sentence).
2. **Why it exists** (the problem it solves).
3. **Common pitfall or caveat** (optional but recommended).

Keep explanations short. A paragraph or two per concept is enough. Link to the Bevy docs if the reader wants to go deeper.

> Example:
> ### `Camera2d`
> `Camera2d` is a *bundle* — a collection of components pre-configured for a 2D orthographic camera. It exists so you don't have to manually assemble a `Camera`, `Transform`, and render-target configuration every time you need a 2D view. The bundle spawns the camera looking at the world origin with one world unit equal to one pixel by default.
>
> **Pitfall:** Forgetting to spawn a camera leaves the window black, because the renderer has no target to draw into.

---

## Walkthrough

Explain **what to do** and **why**. Code snippets should illustrate the explanation, not replace it. The reader should be able to understand the step *before* looking at the code block.

### Designing the feature

For non-trivial features, begin the walkthrough by describing the **player-visible behavior**. List the observable effects, then derive the components, resources, and constants needed to implement them. This teaches the reader to think in ECS terms — data follows behavior — before any code appears.

> Example:  
> Before writing code, think about what the player should see:  
> 1. **Base and barrel** — a static base plus a rotating barrel that points toward enemies.  
> 2. **Ammo on the barrel** — three small rockets sitting in visible slots.  
> 3. **Depletion** — when the launcher fires, one slot becomes empty.  
> 4. **Refill** — after a short delay, a new rocket appears in the first empty slot.  
> 5. **Fire cooldown** — even with ammo present, the launcher can't dump all rockets instantly.  
>  
> From this we derive the data we need: a tag component `RocketLauncher`, a capacity constant `ROCKET_MAX_AMMO = 3`, a component `AmmoSlots`, and a timer component `AttackTimer`.

### Writing rules for this section

- **Reason before code.** Every code block must be preceded by prose that explains what it does, why it exists, and what the reader should notice. The reader should understand the intent before reading a single line. Start each step with the goal, not the code.
  - ✅ "To make the camera follow the player's cursor, we need to read the cursor position every frame. Bevy provides `Res<CursorPosition>` for this."
  - ❌ "Add this system:"

- **Keep snippets short.** Ideally 3–10 lines. If a function grows, show it in incremental pieces or annotate the new lines.

- **Introduce new syntax before using it.** If you mention `Query<&mut Transform>`, explain what a `Query` is *in the same paragraph or earlier*.

- **Prefer `diff`-style for small additions.** If you're adding two lines to an existing function, show only those two lines with a comment about where they go, rather than reprinting the entire function.

- **Name your snippets.** Use comments or short labels so the reader knows which file they're editing.

- **No copy-pasteable walls.** If a complete file is useful for reference, put it in a collapsible `<details>` block at the *end* of the step, not inline.

- **Explain the non-obvious.** Comment the *what* and *why* of non-obvious code in the snippets themselves. Obvious lines (e.g., `commands.spawn(Camera2d)`) do not need inline comments.

- **Enumerate queries.** Every system section must list its queries and explain what each is for. If a query uses `Without<T>` filters, say why — usually to prove disjointness to Bevy or to exclude irrelevant entities.

- **Prefer prose + source reference for large functions.** When a function exceeds ~15 lines, do not paste the entire body inline. Instead, explain what it does, what queries and resources it needs, and why it does it that way. Then point the reader to the source file: "See `pub fn foo` in `src/bar.rs` for the full implementation."

- **Place observable checkpoints.** After each milestone where the reader can verify partial progress, add a `> **Run the game now.**` callout describing what they should see. This gives the reader confidence and catches mistakes early.

### Simplifications

When you make a simplification (hardcoded value, single variant, no pooling, etc.):

1. State **why** the simplification works for this project.
2. Briefly mention **how a more complex system would handle it** to plant the seed for the learner.

> Example:  
> For now, we hardcode the map size to 20×20 tiles. That keeps the code readable while we focus on rendering. In a larger game you would load level dimensions from an asset file or a resource so designers can tweak them without recompiling.

---

## Summary

In 3–5 bullet points, recap what was built and what the reader learned. Then, preview the next part so the reader knows what to expect.

> Example:
> - We created a `Map` resource that stores tile data in a `Vec<TileKind>`.
> - We used `Sprite` bundles to draw each tile, calculating screen positions from grid coordinates.
> - We added keyboard input to pan the camera with `Input<KeyCode>`.
>
> In the next part, we'll replace our manual sprite spawning with a `Tilemap` for better performance and easier tileset management.
