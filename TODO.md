# Implementation Plan: Sort Left Panel by File Count

Source: `docs/research/sort-left-panel-by-file-count.md`

| # | Commit message | Logical unit | Key deliverables | Tests |
|---|---|---|---|---|
| 1 | `feat: add FileCount variant to LeftPanelSortMode` | Enum extension | `src/gui/state.rs` | Unit (default) |
| 2 | `feat: update sort-mode toggle cycle for three modes` | Cycle logic | `src/gui/update.rs`, `src/gui/view.rs` | Unit (3-mode cycle) |
| 3 | `feat: add FileCount button label in left panel` | Button label | `src/gui/left_panel.rs` | — |
| 4 | `feat: add FileCount sort arm to render_file_node` | Directory tree sort | `src/gui/render_node.rs` | Unit (file count sort) |
| 5 | `feat: thread sort_mode through render_tag_node and sort children` | Tag tree sort | `src/gui/render_node.rs`, `src/gui/left_panel.rs` | Unit (tag node sort) |
| 6 | `test: add tests for all new sorting behaviours` | Test suite | `src/gui/render_node.rs` | Unit (file node + tag node sort regression) |
| 7 | `docs: update module docstrings for FileCount sort mode` | Documentation | `src/gui/state.rs`, `src/gui/mod.rs`, `src/gui/render_node.rs` | — |

**Notes:**

- Step 1 only adds `test_left_panel_sort_mode_default` (the 3-mode cycle test waits for Step 2 when the cycle logic is updated).
- Step 5 must be atomic — adding a `sort_mode` parameter to `render_tag_node` requires updating all callers in the same commit.
- Tests for `render_file_node` FileCount sorting belong in Step 4; tests for `render_tag_node` sorting belong in Step 5; regression tests belong in Step 6.
- Step 2 also updates the existing `test_toggle_left_panel_sort_mode` test (replaces 2-mode assertions with 3-mode cycle).
