---
name: review-bevy-tutorial
description: Review a Bevy tutorial markdown file against the actual implementation. Use after writing or editing a tutorial part to catch factual errors, missing steps, and weak justifications.
---

# Review Bevy Tutorial

Review a Bevy tutorial `.md` file against the actual source code. Read source files directly for accuracy; diffs are only used to detect scope creep.

## Before you start

1. Read the tutorial file in full.
2. Identify the project part/tag the tutorial is supposed to represent (e.g., `part-5`, `HEAD`).
3. Read the review checklist in `references/CHECKLIST.md` if you need a detailed reference.

## Review process

### 1. Read the tutorial and extract claims

Note every claim about:
- Files created or modified.
- Functions, structs, components, resources, systems.
- System signatures (parameters, queries, mutability).
- Schedule placement (`Update`, `FixedUpdate`, `OnEnter`, etc.).
- Constants, default values, and magic numbers.
- Data format changes (TOML fields, asset formats, etc.).

### 2. Verify claims against source files

Open the actual source file and check each claim directly. Do not rely solely on `git diff`.

| Category | How to verify |
|---|---|
| **File exists?** | `ls` or check `git diff --stat` |
| **Function/type names** | Read the file, find the definition, compare spelling |
| **System parameters** | Read the function signature, compare type and mutability |
| **Schedule placement** | Read `main.rs`, find the `.add_systems(...)` call |
| **Constants/values** | Read the source, check the value |
| **Code snippets** | Would the shown Rust compile? Check casts, types, etc. |
| **Data changes** | Read the TOML/YAML/JSON file, check field values |
| **Ordering claims** | Read `main.rs` — does the order of systems in a chain match? |

### 3. Check justifications

Tutorials often explain *why* a choice was made. For every "why" claim, try to construct a valid counter-argument. If you can, the justification is weak or false.

Ask yourself:
- Is the opposite also true? ("A is better than B" — is B actually just as good?)
- Does the stated reason actually explain the choice? ("We do X because Y" — would Y still happen without X?)
- Is the problem being prevented a real problem? ("We order A before B to avoid bug C" — would bug C actually occur in the opposite order?)
- Is the claim about concurrency or timing true in Bevy's execution model? (e.g., commands flush at system end, not mid-system; `ResMut` is exclusive within a system.)

Common patterns of faulty justifications:
- **Order matters for correctness** when both orders are equivalent in single-threaded Bevy.
- **Prevents a problem that wouldn't occur anyway** (e.g., removing a resource that is already overwritten on next start).
- **A is better than B because of property P, but B also has P**.
- **False claims about Bevy internals** (e.g., direct `ResMut` writes are immediate; only `commands.insert_resource` is deferred).

### 4. Distinguish later patches from actual errors

The source tree may contain code from parts *after* the one being reviewed. When you spot a discrepancy, check both the `part-N` tag and the latest commit (`HEAD`).

There are three possibilities:

1. **Tutorial matches `part-N` but diverges from `HEAD`.** Expected — later parts evolved the code. The tutorial is correct for its part.
2. **Tutorial diverges from `part-N` in a way that matches `HEAD`.** The tutorial may have been intentionally updated to reflect a bug fix discovered later. Verify the `part-N` tag shows the bug and `HEAD` shows the fix. Treat this as an intentional improvement, not an error.
3. **Tutorial diverges from both `part-N` and `HEAD`.** Genuine factual error — flag it.

### 5. Check for missing or extra steps

- **Missing:** The implementation changed something the tutorial doesn't mention.
- **Extra:** The tutorial describes a step that doesn't exist in the source.

### 6. Scope creep check

Run:

```bash
git diff --stat
```

- List every file touched.
- Mark files clearly outside the tutorial's stated topic.
- If a file is borderline, explain why it might or might not be in scope.

If scope creep is detected, say **SCOPE CREEP** prominently in the report.

### 7. Report

Use this structure:

```markdown
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

## Common problems to watch for

1. **Shorthand notation** — tutorial writes `OnEnter(InGame)` but code uses `OnEnter(GameState::InGame)`.
2. **Missing type casts** — tutorial writes `gold >= TOWER_COST` but the types differ (`f32` vs. `u32`).
3. **Omitted components** — tutorial doesn't mention a component the code attaches (e.g., a cleanup tag).
4. **Wrong mutability** — tutorial describes `Res<T>` but code uses `ResMut<T>`.
5. **Faulty justifications** — see step 3. If the tutorial explains *why*, verify the reasoning holds up. These are easy to miss because the code itself may be correct.
