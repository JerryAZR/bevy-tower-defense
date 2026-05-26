# Agent Rules

1. **Never push before review.** Always wait for explicit approval before running `git push`.

## Tutorial Project

This is a tutorial project. Code and prose must teach, not just work.

### Code Commenting

- Comment the **what** and **why** of non-obvious code. Unlike production code (where "what" comments are discouraged), tutorial code should explain itself to a learner.
- Obvious lines (e.g., `commands.spawn(Camera2d)`) don't need comments.

### Tutorial Structure

Each tutorial document (`tutorial/XX-topic.md`) should follow this structure:

1. **Instruction** — what we will build and why it matters.
2. **New Bevy APIs** — introduce any new concepts, types, or patterns, along with common pitfalls.
3. **Walkthrough** — step-by-step code changes with explanations of what each piece does and why it's there.

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
