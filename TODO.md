# Implementation Plan: File Count Indicator in Left Panel Categories

Source: `docs/research/file-count-indicator-in-left-panel.md`

## Sequence overview

| # | Commit message | Logical unit | Key deliverables | Tests |
|---|---|---|---|---|
| 1 | `feat: add file_count field to FileNode and TagTreeNode` | Data model + tree population | `src/fs/file_tree.rs`, `src/gui/state.rs`, `src/fs/media_metadata.rs`, `src/fs/media_metadata_async.rs`, `src/gui/view.rs` | Unit |
| 2 | `feat: add count labels and dynamic background styling` | Rendering | `src/gui/view.rs`, `src/gui/left_panel.rs`, `src/gui/render_node.rs` | Unit, Property-based |
| 3 | `chore: cleanup and final verification` | Cleanup | — | — |

## Step details

### Step 1 — Add `file_count` fields and populate during tree construction

**Rationale**: Both `FileNode` and `TagTreeNode` need a `file_count` field. `FileNode` uses constructors exclusively, so adding the field inside `new_file()` (sets to 1) and `new_directory()` (sums children) is safe. `TagTreeNode` is constructed via struct literals in 17 places — every site must be updated. `#[serde(default)]` ensures old persisted data still deserialises.

**Files modified**:

- `src/fs/file_tree.rs` — add `pub file_count: usize` field to `FileNode` struct; update `new_file()` to set `file_count = 1`; update `new_directory()` to compute `file_count = children.iter().map(|c| c.file_count).sum()`
- `src/gui/state.rs` — add `pub file_count: usize` with `#[serde(default)]` to `TagTreeNode`; add bincode backward-compatibility note (option 1: data loss accepted)
- `src/fs/media_metadata.rs` — add `file_count: 1` to all leaf `TagTreeNode` literals and `file_count: <sum>` to parent nodes in `build_genre_tag_tree()` and `build_creator_tag_tree()`
- `src/fs/media_metadata_async.rs` — same changes as `media_metadata.rs` for `build_tag_genre_tree_async()` and `build_tag_musician_tree_async()`
- `src/gui/view.rs` — add `file_count: 1` / `file_count: 0` to test `TagTreeNode` literals

**Tests** (in `src/fs/file_tree.rs`):

- `file_count_none`: empty `FileNode::new_directory` with no children — expects `file_count == 0`
- `file_count_one`: single-file directory — expects `file_count == 1`
- `file_count_many`: multi-level nested directories with multiple leaf files — expects `file_count` equal to the total leaf count

### Step 2 — Add count labels and dynamic background styling

**Rationale**: The `flat_button_style` closure is threaded from `view.rs` → `create_left_panel()` → `create_left_panel_file_tree_browser()` / `create_left_panel_tag_tree_browser()` → `render_file_node()` / `render_tag_node()`. We add a `file_count_highlight()` helper and a `directory_button_style()` factory, then change the closure to accept `Option<usize>` so directory nodes get tinted backgrounds while file nodes stay flat.

**Files modified**:

- `src/gui/render_node.rs`:
  - Add `file_count_highlight(count: usize, max_count: usize) -> Color` — log-scale interpolation between faint blue and deep navy
  - Add `directory_button_style(count: usize, max_count: usize) -> impl Fn(&Theme, Status) -> Style` — returns a button `Style` with `background` set via `file_count_highlight`
  - `render_file_node()`: for `NodeType::Directory`, append `({file_count})` to label; apply `directory_button_style` instead of `flat_button_style`; accept `max_count` parameter
  - `render_tag_node()`: for non-leaf nodes, append `({file_count})` to label; apply `directory_button_style`; leaf nodes keep flat style; accept `max_count` parameter
- `src/gui/left_panel.rs`:
  - `create_left_panel_file_tree_browser()`: compute `max_count` from `root_nodes`; pass to `render_file_node`; update closure parameter
  - `create_left_panel_tag_tree_browser()`: compute `max_count` from `tag_tree_roots`; pass to `render_tag_node`; update closure parameter
  - `create_left_panel()`: update closure parameter threading
- `src/gui/view.rs`:
  - Update `flat_button_style` closure signature to `|count: Option<usize>, _theme, _status|` — branch on `count`: `None` → flat (no background), `Some(n)` → call `file_count_highlight(n, max_count)`
  - Pass `flat_button_style` with `None` at the top level (count will be supplied downstream during per-tree `max_count` computation, or we restructure to pass it differently)
  - Update all function signatures in the call chain

**Note on approach**: Rather than making `flat_button_style` carry a `max_count` parameter everywhere, the simplest design is: `flat_button_style` becomes a simple no-background style for leaf/file nodes; directory nodes in `render_file_node` and non-leaf tag nodes in `render_tag_node` use `directory_button_style(count, max_count)` directly. This avoids threading `max_count` through the entire call chain — only the per-tree top-level functions compute it and pass it down.

**Tests** (in `src/gui/render_node.rs` or new test module):

- `highlight_zero`: `file_count_highlight(0, 42)` returns the light baseline
- `highlight_min`: `file_count_highlight(1, 42)` — minimum non-zero count
- `highlight_max`: `file_count_highlight(42, 42)` — max boundary returns darkest colour
- `highlight_monotonic`: property-based — for any `count < other_count` (same `max_count`), the luminance is monotonic
- `label_format_directory`: verify `render_file_node` label for a directory node contains `({count})`
- `label_format_tag_leaf`: verify `render_tag_node` for a leaf node does NOT contain a count suffix

### Step 3 — Cleanup and final verification

**Rationale**: Remove any dead parameters, run the full suite, manually verify.

**Actions**:

- Remove `flat_button_style` parameter from directory-rendering paths if it is no longer needed after Step 2's refactor
- Run `cargo test --all` — all tests must pass
- Run `cargo build` — zero warnings, zero errors
- Remove any stale `flat_button_style` arguments from tag-tree browser paths if they only pass through unused
