# Bevy Tutorial Review Checklist

Use this checklist when reviewing a Bevy tutorial part against the implementation.

## Tutorial structure

- [ ] Title header includes part number, title, subtitle, reading time, and new concepts.
- [ ] Recap section exists and is one or two sentences.
- [ ] Goal section exists and explains what the reader will build and why it matters.
- [ ] New Bevy APIs & Concepts section introduces concepts before they appear in code.
- [ ] Walkthrough section exists and follows a logical order.
- [ ] Summary section includes 3–5 bullet points and a preview of the next part.

## Prose quality

- [ ] Every code block is preceded by prose explaining intent.
- [ ] No step opens with "Add this function:" or similar without explanation.
- [ ] New syntax is explained before (or in the same paragraph as) its first use.
- [ ] Simplifications state why they work and mention the real-world alternative.
- [ ] Bevy-specific concepts are explained on first use and recapped later.
- [ ] Assumed knowledge matches Rust basics + Bevy beginner.

## Code snippets

- [ ] Snippets are short (3–10 lines) where possible.
- [ ] Large functions (>15 lines) are summarized with a pointer to the source file.
- [ ] Small additions use `diff`-style comments rather than reprinting the whole function.
- [ ] Each snippet is labeled with the file it belongs to.
- [ ] Non-obvious lines have `what` and `why` comments.
- [ ] Obvious lines are not over-commented.
- [ ] System queries are enumerated and explained.
- [ ] Complete-file references are in collapsible `<details>` blocks at the end of a step.

## Factual accuracy

- [ ] File names and paths match the source tree.
- [ ] Function, struct, component, resource, and system names are spelled correctly.
- [ ] System parameters match the implementation (type, mutability, generics).
- [ ] Schedule placement matches `main.rs` (or wherever systems are registered).
- [ ] Constants and default values match the source.
- [ ] Type casts and conversions in snippets would compile.
- [ ] Data format changes (TOML, YAML, etc.) match the actual files.
- [ ] Ordering claims about system chains match the implementation.

## Justifications

For each "why" claim in the tutorial:
- [ ] The stated reason actually explains the choice.
- [ ] The opposite order or alternative would not produce the same result.
- [ ] The problem being prevented would actually occur without the claimed fix.
- [ ] Claims about Bevy timing, commands, and concurrency are accurate.

## Completeness and scope

- [ ] No implementation changes are missing from the tutorial.
- [ ] No tutorial steps describe code that does not exist in the source.
- [ ] `git diff --stat` files are all within the tutorial's stated topic.
- [ ] Any scope creep is flagged clearly.

## Divergence check

- [ ] If the tutorial diverges from the current source, check the `part-N` tag.
- [ ] If the tutorial matches `HEAD` but not `part-N`, verify it is an intentional fix.
- [ ] If the tutorial diverges from both `part-N` and `HEAD`, flag it as a factual error.
