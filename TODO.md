# Implementation Plan: Text Search Feature

Source: `docs/research/text-search-feature.md`

| # | Commit message | Logical unit | Key deliverables | Tests |
|---|---|---|---|---|
| 1 | `feat: add TextSearchMode enum, search fields, and Message variants` | Types and model fields | `src/gui/state.rs` (TextSearchMode enum, search_query/search_mode fields, SearchQueryChanged/ToggleSearchMode messages, updated FileTreeApp::new()), `src/gui/mod.rs` (TextSearchMode export) | Unit |
| 2 | `feat: wire SearchQueryChanged and ToggleSearchMode update handlers` | Update handlers | `src/gui/update.rs` (two new match arms) | Unit |
| 3 | `feat: add search bar UI with mode toggle to left panel` | Search bar UI | `src/gui/left_panel.rs` (create_search_row, insertion into create_left_panel) | Smoke |
| 4 | `feat: implement file tree filtering with search` | File tree filtering | `src/gui/left_panel.rs` (filter_file_node, file_matches_metadata_mode, create_left_panel_file_tree_browser_from_nodes, search-aware filtering in create_left_panel) | Unit, Property-based |
| 5 | `feat: implement tag tree filtering with search` | Tag tree filtering | `src/gui/left_panel.rs` (filter_tag_node, create_left_panel_tag_tree_browser_from_nodes, search-aware filtering in create_left_panel) | Unit, Property-based |
| 6 | `chore: clippy, docs, and final polish` | Cleanup | All touched files (clippy fixes, docstring updates) | — |

## Design Decisions

- **Metadata cache**: Deferred (not in v1). `filter_file_node` in metadata modes calls `extract_media_metadata` on every file on every render. A metadata cache (`HashMap<PathBuf, MediaMetadata>`) can be added as a follow-up.
- **Remove buttons during search**: Disabled/hidden when `search_query` is non-empty to avoid index-alignment issues with filtered root nodes.
- **Refactoring**: New `_from_nodes` variants of the tree browser functions are added alongside the existing ones, keeping backward compatibility.
