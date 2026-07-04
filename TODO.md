# Implementation Plan: Search Expansion Bugfix

Source: `docs/research/search-expansion-bugfix.md`

Fix stale expansion state in filtered trees when search is active. Three
message handlers skip recomputing the filtered derivative views after
mutating the original trees.

| # | Commit message | Logical unit | Key deliverables | Tests |
|---|---|---|---|---|
| 1 | `fix: recompute filtered tag tree on ToggleTagExpansion` | Tag tree expansion sync | `src/gui/update.rs` (ToggleTagExpansion handler) | Unit (3): no search, during search, non-matching parent with matching children |
| 2 | `fix: recompute filtered file tree on ToggleExpansion` | File tree expansion sync | `src/gui/update.rs` (ToggleExpansion handler) | Unit (2): during search, no search |
| 3 | `fix: recompute filtered trees on ToggleExtension` | Extension change sync | `src/gui/update.rs` (ToggleExtension handler) | Unit (2): during search, no search |
| 4 | `chore: clippy, fmt, and final verification` | Final polish | — | cargo clippy --all-targets -- -D warnings, cargo fmt, cargo test |
