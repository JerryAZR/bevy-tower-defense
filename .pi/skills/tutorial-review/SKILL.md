---
name: tutorial-review
description: Review a tutorial markdown file against the actual implementation. Reads source files for accuracy checks; uses git diff only for scope creep detection (files touched outside the tutorial topic).
---

# Tutorial Review

Review a tutorial `.md` file against the actual implementation. Source files are read directly for accuracy — diffs are hard to parse and only used for scope analysis.

## Process

### 1. Read the tutorial

Read the tutorial file in full. Note every claim about:
- Files that were created or modified
- Functions, structs, components, resources, systems
- System signatures (parameters, queries, mutability)
- Schedule placement (Update, FixedUpdate, OnEnter, etc.)
- Constants, default values, magic numbers
- Data format changes (TOML fields)

### 2. Verify claims against source files

For each claim, open the actual source file and check it directly.

| Category | How to verify |
|---|---|
| **File exists?** | `ls` or check `git diff --stat` |
| **Function/type names** | Read the file, find the definition, compare spelling |
| **System parameters** | Read the function signature, compare each parameter's type and mutability |
| **Schedule placement** | Read `main.rs`, find the `.add_systems(...)` call |
| **Constants/values** | Read the source, check the value |
| **Code snippets** | Would the shown Rust compile? Check for missing casts, wrong types |
| **Data changes** | Read the TOML/YAML file, check the field values |
| **Ordering claims** | Read `main.rs` — does the order of systems in a chain match the claim? |

### 3. Check justifications for faulty reasoning

Tutorials often explain *why* a choice was made. Flag justifications that don't hold up:

- **Claiming an order matters for correctness when both orders are equivalent.** Example: "We deduct gold before spawning so the player can't place two towers on one click" — but in single-threaded Bevy, both orders are equivalent. `commands.spawn()` is queued, `gold.0 -=` is immediate, so the outcome is identical regardless of source order.
- **Claiming a step prevents a problem that wouldn't occur anyway.** Example: "We remove the `Gold` resource on level exit so the player's balance doesn't carry over" — but `load_level_data` already overwrites it with `Gold(STARTING_GOLD)` on the next level start. The removal is fine for intent-clarity, but the stated *reason* (carry-over) is wrong.
- **Claiming option A is better than B because A has some property — but B also has it.** Example: "Pattern X is cleaner because it avoids extra state" — but pattern Y also avoids extra state.

These are not code bugs — the *code* is fine — but they mislead the learner about how things work.

### 4. Check for missing or extra steps

- **Missing:** The implementation changed something the tutorial doesn't mention.
- **Extra:** The tutorial describes a step that doesn't exist in the source.

### 5. Scope creep check

```bash
git diff --stat
```

- List every file touched.
- Mark files that are **clearly outside the tutorial's stated topic** (e.g., an economy tutorial touching the tiling system).
- If a file is borderline, explain why it might or might not be in scope.

If scope creep is detected, say **SCOPE CREEP** prominently in the report.

### 6. Report

```
## Review: <tutorial-file> vs. implementation

### ✅ Correct
- (list verified claims)

### ⚠️ Issues
- (each issue with file:line reference and correction)

### 🔍 Missing from tutorial
- (implementation changes not documented)

### 📦 Scope creep
- (unrelated files in the diff, or "none")

### Verdict
- (one-line summary)
```

## Common Problems

1. **Shorthand notation** — tutorial writes `OnEnter(InGame)` but code uses `OnEnter(GameState::InGame)`.
2. **Missing type casts** — tutorial writes `gold >= TOWER_COST` but the types differ (`f32` vs. `u32`).
3. **Omitted components** — tutorial doesn't mention a component the code attaches (e.g., `GameEntity` for cleanup).
4. **Wrong mutability** — tutorial describes `Res<Gold>` but code uses `ResMut<Gold>`.
5. **Faulty justifications** — see step 3. If the tutorial explains *why* something was done, verify the reasoning holds up. These are easy to miss because the code itself is correct.
