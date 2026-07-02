# Implementation Plan: Sort Tag Tree Roots by File Count

Source: `docs/research/fix-sort-by-file-count-for-genre-and-creator.md`

| # | Commit message | Logical unit | Key deliverables | Tests |
|---|---|---|---|---|
| 1 | `feat: sort tag tree roots by current sort mode before rendering` | `sort_tag_tree_roots` helper + wiring | `src/gui/left_panel.rs` | — (no test-only change; tests are step 2) |
| 2 | `test: add tests for tag tree root-level sorting` | Unit tests for `sort_tag_tree_roots` | `src/gui/left_panel.rs` | Unit (8 tests per plan) |
| 3 | `docs: update left_panel module docstring` | Module docstring for new helper | `src/gui/left_panel.rs` | — |
