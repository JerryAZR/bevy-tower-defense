# Agent Rules

1. **Never push before review.** Always wait for explicit approval before running `git push`.

## Tutorial Project

This is a tutorial project. Code and prose must teach, not just work.

### Code Commenting

- Comment the **what** and **why** of non-obvious code. Unlike production code (where "what" comments are discouraged), tutorial code should explain itself to a learner.
- Obvious lines (e.g., `commands.spawn(Camera2d)`) don't need comments.

### Tutorial Structure

:books: See `tutorial/TEMPLATE.md` for the canonical tutorial structure, section descriptions, and writing rules.

:spiral_notepad: In short, every tutorial part should contain:
1. **Recap** — what the project already has (one sentence for continuity).
2. **Goal** — what we will build in this part and why it matters.
3. **New Bevy APIs & Concepts** — introduce the concepts the reader should *learn* before they appear in code. Include common pitfalls.
4. **Walkthrough** — step-by-step changes. Explain the intent first, then show short illustrative code snippets.
5. **Summary** — bullet-point recap and a preview of the next part.

When creating a new tutorial, start from `tutorial/TEMPLATE.md` and remove the inline comments.

### Assumed Knowledge

- Assume the reader is comfortable with Rust basics (ownership, structs, enums, iterators, `match`) but has **no mastery** of advanced patterns.
- Assume the reader is learning Bevy — explain Bevy-specific concepts (resources, components, systems, queries, states, plugins) on first use, and briefly recap on subsequent use.

### Simplifications

When we make a simplification (e.g., hardcoded values, single tower type, no pooling):

1. Clearly state **why** the simplification works for our project.
2. Briefly mention **how a more complex system would handle it** (to plant the seed for the learner).

### Code Quality

- Follow good Rust and Bevy practices. Don't teach bad habits.
- Prefer clarity over cleverness.
- Use descriptive variable and type names.
