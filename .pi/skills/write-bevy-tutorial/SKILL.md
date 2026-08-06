---
name: write-bevy-tutorial
description: Write or edit a Bevy tutorial part following a consistent, learner-first structure. Use for new parts, rewrites, or converting implementation work into tutorial prose.
---

# Write Bevy Tutorial

Write a Bevy tutorial part (or edit an existing one) so the prose teaches the reader *why* the code is structured the way it is, not just *what* to type.

This skill is project-agnostic: it assumes the project has a `tutorial/` directory and a tutorial template, but the rules apply to any Bevy tutorial series.

## Before you start

1. Read the project's canonical tutorial template.
   - If the project has `tutorial/TEMPLATE.md`, read it.
   - Otherwise, read this skill's reference: `references/TUTORIAL_TEMPLATE.md`.
2. Read any project-specific `AGENTS.md` rules that mention tutorials.
3. Read the relevant source files so you know what actually changed.

## Part structure

Every tutorial part should contain these sections in order:

1. **Title header** — `Part N: Title — Subtitle`, plus reading time and new concepts.
2. **Recap** — one or two sentences about where the project stands after the previous part.
3. **Goal** — what the reader will build and why it matters for the finished game.
4. **New Bevy APIs & Concepts** — introduce concepts before they appear in code.
5. **Walkthrough** — step-by-step changes with prose before every code block.
6. **Summary** — bullet-point recap and a preview of the next part.

See `references/TUTORIAL_TEMPLATE.md` for the full template and examples.

## Writing process

### 1. Define the part

Decide:
- What the reader will be able to see or do after this part.
- Which Bevy concepts are new and need explanation.
- Which source files change.

Write the **Goal** first. If you cannot state the goal in one or two plain sentences, the part is too large — split it.

### 2. Design the feature in ECS terms

For non-trivial features, start the **Walkthrough** by describing the *observable behavior* first, then derive the ECS data:

1. List the player-visible effects.
2. From those effects, decide which components, resources, and systems are needed.
3. Only then present the code.

This teaches the reader to think in ECS terms: data follows behavior.

### 3. Introduce concepts before code

Each new Bevy concept should appear in **New Bevy APIs & Concepts** before it is used in a snippet. For each concept, explain:
- What it is (one sentence).
- Why it exists (the problem it solves).
- A common pitfall or caveat (optional but recommended).

Keep explanations short. Link to Bevy docs for readers who want depth.

### 4. Write prose before code

Every code block must be preceded by prose that explains:
- What the block does.
- Why it is structured that way.
- What the reader should notice.

The reader should understand the intent before reading a single line.

✅ Good: *To make the camera follow the player's cursor, we need to read the cursor position every frame. Bevy provides `Res<CursorPosition>` for this.*

❌ Bad: *Add this system:*

### 5. Keep snippets small and focused

- Aim for 3–10 lines per snippet.
- If a function grows, show it in incremental pieces or annotate the new lines.
- For small additions to existing code, use `diff`-style comments rather than reprinting the whole function.
- Name the file the snippet belongs to (e.g., `// src/main.rs`).
- If a complete file is useful for reference, put it in a collapsible `<details>` block at the end of the step, not inline.

### 6. Explain the non-obvious

Comment the *what* and *why* of non-obvious code inside the snippets. Do not comment obvious lines (e.g., `commands.spawn(Camera2d)`).

When a system is introduced, list its queries and explain what each one is for. If a query uses `Without<T>` or similar filters, explain why.

### 7. Place observable checkpoints

After every milestone where the reader can verify partial progress, add a callout:

```markdown
> **Run the game now.** You should see ...
```

This gives the reader confidence and catches mistakes early.

### 8. Handle simplifications honestly

When you use a simplification (hardcoded value, single variant, no pooling, etc.):

1. State **why** it works for this tutorial.
2. Briefly mention **how a more complex system would handle it** to plant the seed for the learner.

Example: *For now, we hardcode the map size to 20×20 tiles. That keeps the code readable while we focus on rendering. In a larger game you would load level dimensions from an asset file or a resource so designers can tweak them without recompiling.*

## Style rules

- Assume the reader knows Rust basics (ownership, structs, enums, iterators, `match`) but has **no mastery** of advanced patterns.
- Assume the reader is learning Bevy — explain Bevy-specific concepts (resources, components, systems, queries, states, plugins) on first use, and briefly recap on subsequent use.
- Prefer clarity over cleverness. Use descriptive variable and type names.
- Follow good Rust and Bevy practices. Do not teach bad habits.
- Do not start a step with "Add this function:" or dump a full implementation without describing it first.

## After writing

1. Read the finished part aloud in your head to check for abrupt jumps.
2. Verify every snippet is consistent with the actual source files.
3. Run the `review-bevy-tutorial` skill (or at least `references/CHECKLIST.md`) , with a subagent if possible, to catch missing steps, factual errors, and weak justifications.
