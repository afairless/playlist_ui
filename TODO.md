# Implementation Plan: Option A — Skip Re-Filtering on Expand/Collapse

Source: `docs/research/implement-option-a-skip-refilter.md`

| # | Commit message | Logical unit | Key deliverables | Tests |
|---|---|---|---|---|
| 1 | refactor: skip re-filter on `ToggleExpansion` | Modify `ToggleExpansion` handler | `src/gui/update.rs` — replace `recompute_filtered_nodes` call with in-place `restore_expansion_state` on `filtered_root_nodes` | Unit (existing tests pass) |
| 2 | refactor: skip re-filter on `ToggleTagExpansion` | Modify `ToggleTagExpansion` handler | `src/gui/update.rs` — replace `recompute_filtered_tag_nodes` call with in-place `find_tag_node_mut` toggle on `filtered_tag_tree_roots` | Unit (existing tests pass) |
| 3 | test: add structure-preservation tests for expand/collapse during search | Add targeted invariant tests | `src/gui/update.rs` — two new tests verifying filtered tree structure is unchanged by toggle | Unit (new tests) |
| 4 | chore: mark test-only helpers with `#[allow(dead_code)]` | Suppress dead-code warnings on `recompute_filtered_*` | `src/gui/update.rs` — add `#[allow(dead_code)]` to `recompute_filtered_nodes` and `recompute_filtered_tag_nodes` | — |
