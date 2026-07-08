# Expand/Collapse Performance During Active Search

## Context

The categories in the left panel — Directory, Genre, and Creator — can be
expanded or collapsed to reveal sub-categories or individual music
tracks/files. These operations are normally fast. However, when a search
term is present in the search text box and the collection is large
(~32,000 tracks), expanding or collapsing a node can take several seconds.

This is a separate bottleneck from the search-typing performance analyzed
in `search-performance-research.md` (which covers responsiveness during
keystrokes). The typing path was addressed by the tantivy full-text index
(`tantivy-integration-research.md`), but the expand/collapse path is still
slow.

## Current Implementation

### File-tree expand/collapse (`ToggleExpansion`)

When the user clicks a directory expand/collapse button while a search is
active:

1. The `expanded_dirs` `HashSet<PathBuf>` is updated.
2. `restore_expansion_state()` walks the **entire** `root_nodes` tree
   to sync `is_expanded` flags.
3. `recompute_filtered_nodes()` is called, which invokes
   `filter_file_node()` on every root node — a **full recursive deep-clone**
   of all ~32,000 tree nodes, including per-node string matching.

```rust
// src/gui/update.rs — ToggleExpansion handler
Message::ToggleExpansion(path) => {
    if app.expanded_dirs.contains(&path) {
        app.expanded_dirs.remove(&path);
    } else {
        app.expanded_dirs.insert(path);
    }
    for root in app.root_nodes.iter_mut().flatten() {
        restore_expansion_state(root, &app.expanded_dirs);
    }
    app.filtered_root_nodes = recompute_filtered_nodes(app);
    Task::none()
},
```

`recompute_filtered_nodes` delegates to `filter_file_node` (not
tantivy), which does a `String` `.contains()` substring match on every
node's name and path, then clones each matched node and its children.

### Tag-tree expand/collapse (`ToggleTagExpansion`)

When the user clicks a Genre or Creator node while a search is active:

1. `find_tag_node_mut()` toggles `is_expanded` on the specific node in
   `tag_tree_roots`.
2. `recompute_filtered_tag_nodes()` is called, which invokes
   `filter_tag_node()` on every root — another **full recursive
   deep-clone** of the entire tag tree, with per-node string matching and
   path checks.

```rust
// src/gui/update.rs — ToggleTagExpansion handler
Message::ToggleTagExpansion(path) => {
    if let Some(node) =
        find_tag_node_mut(&mut app.tag_tree_roots, &path)
    {
        node.is_expanded = !node.is_expanded;
    }
    app.filtered_tag_tree_roots = recompute_filtered_tag_nodes(app);
    Task::none()
},
```

### Why this is slow

Two compounding problems:

1. **Full tree reclone on every expand/collapse.** Changing one
   `is_expanded` flag triggers a complete O(n) clone of all ~32,000
   nodes. The filter query has not changed — only expansion state — yet
   the entire tree is re-filtered and re-cloned.

2. **Two different filtering implementations; expand/collapse uses the
   slower one.**
   - Search query changes use tantivy index + `prune_file_tree` /
     `prune_tag_node` (fast, index-assisted).
   - Expand/collapse uses `filter_file_node` / `filter_tag_node` (pure
     `String::contains` on every node, no index).

   The expand/collapse path does the same work as search but without
   the index — substring-matching across tens of thousands of paths and
   labels on every click.

3. **Filtered trees couple filtering with expansion state.** The
   filtered trees (`filtered_root_nodes`, `filtered_tag_tree_roots`)
   carry `is_expanded` flags inside the tree nodes themselves. There is
   no mechanism to update just the expansion flag in the already-filtered
   tree — changing expansion state forces a full re-filter + reclone.

## Optimization Options

### Option A: Skip re-filtering on expand/collapse — update expansion in-place (Recommended)

**Idea:** `filtered_root_nodes` and `filtered_tag_tree_roots` should
only be recomputed when the search query or mode changes, not when
expansion toggles. On expand/collapse, just update `is_expanded` in the
already-filtered trees in-place.

**File tree:** `ToggleExpansion` currently calls
`recompute_filtered_nodes()`. Instead, skip that call — the
`expanded_dirs` set is already updated, and `restore_expansion_state`
can also be applied to `filtered_root_nodes` (in addition to the
`root_nodes` it already restores). No re-filter needed.

**Tag tree:** `ToggleTagExpansion` updates `is_expanded` in
`tag_tree_roots`, then fully reclones via
`recompute_filtered_tag_nodes()`. Instead, also find and toggle the
corresponding node in `filtered_tag_tree_roots` directly (find by path
and flip the flag). The filtered tree already has the same structural
shape as the root tree (filtering prunes subtrees but doesn't rename or
reparent nodes).

**Effort:** Small — changes confined to `ToggleExpansion` and
`ToggleTagExpansion` handlers in `update.rs`.

**Speedup:** Eliminates O(n) clone on every expand/collapse. Operations
become O(1) flag toggles.

**Trade-offs:**

- The filtered trees must stay structurally in sync with the originals
  (they already do; both are built from the same source; filtering only
  prunes subtrees and never renames/reparents).
- Edge case: if `filtered_tag_tree_roots` has pruned a node's children,
  toggling expansion on that node in the filtered tree is a no-op
  (children were already removed by the filter). This is correct
  behavior — there's nothing to expand.
- Need to handle the case where the corresponding node was pruned from
  the filtered tree (the find-and-toggle will simply be a no-op).

### Option B: Use tantivy-pruned trees for expand/collapse re-filtering

**Idea:** When re-filtering is necessary (e.g., search query changes),
unify on the tantivy path. `recompute_filtered_nodes()` currently uses
`filter_file_node` (no index). Change it to use `prune_file_tree` with
`last_search_matches` — the same index-assisted path that
`perform_search()` already uses.

When combined with Option A (skip re-filtering on expand/collapse), this
further speeds up the cases where re-filtering does run: typing a new
query, toggling extensions, changing search mode.

**Effort:** Medium — `recompute_filtered_nodes` and
`recompute_filtered_tag_nodes` need to accept the matches set and
delegate to `prune_file_tree` / `prune_tag_node` instead of the
text-based filters. The text-based `filter_file_node` and
`filter_tag_node` could eventually be deprecated.

**Speedup:** The tantivy `prune_*` functions still clone every matching
node, but they avoid per-node `String::contains` substring searches —
matching is a `HashSet` lookup. Combined with Option A, this only
matters for query-change events, not expand/collapse.

**Trade-offs:**

- `recompute_filtered_nodes` needs access to `last_search_matches`,
  which is already available on `app`.
- Adds a dependency on the tantivy search being run beforehand (it
  always is; `perform_search` runs on query change and populates
  `last_search_matches`).

### Option C: `Arc`-backed tree node children

**Idea:** Change `FileNode.children` and `TagTreeNode.children` from
`Vec<FileNode>` to `Vec<Arc<FileNode>>`. Cloning a node becomes O(1)
(reference-count bump) instead of O(n) deep-clone. Filter functions
that prune subtrees still need to clone modified ancestor nodes, but
leaves and unmodified directories share the `Arc`.

**Effort:** Medium — invasive change touching `FileNode`, `TagTreeNode`,
all filter functions, the render functions, and tests.

**Speedup:** All tree cloning becomes O(1) per node. Helps both search
and non-search paths. Even without Option A, expand/collapse would be
much faster.

**Trade-offs:**

- Expansion state mutation requires `Arc::make_mut` (copy-on-write)
  or interior mutability (`RefCell<bool>` for `is_expanded`).
- Increases complexity of the tree data structures.
- Reference counting adds minor overhead to reads (but less than
  deep-cloning).

### Option D: Lazy filtering — only filter children when a node is expanded

**Idea:** Don't eagerly filter the entire tree into
`filtered_root_nodes`. Instead, store only the pre-filtered root-level
nodes. When a user expands a non-matching directory or tag node,
filter its children on-the-fly. Only "hot" (visible/expanded) nodes
are filtered.

**Effort:** Large — requires restructuring how filtered nodes are
stored, rendered, and how file counts propagate to ancestors.

**Speedup:** Eliminates upfront O(n) work entirely. Expand becomes
O(children_of_that_node) instead of O(all_nodes).

**Trade-offs:**

- Requires recalculating ancestor file counts when children are lazily
  filtered (the `file_count` displayed on parent nodes must stay
  accurate).
- More complex state management — the filtered tree is no longer a
  simple field on `FileTreeApp`; it's computed on-the-fly during
  rendering or stored as a partially-materialized structure.
- Most scalable approach for very large collections, but overkill for
  32K tracks if Option A or C suffices.

### Option E: Virtual scrolling with flat pre-filtered list

**Idea:** Pre-flatten the filtered tree into a list of `(depth, node)`
tuples. Use virtual scrolling to only render visible items. Expand
/collapse becomes cheap list manipulation (show/hide children rows).

**Effort:** Large — requires significant changes to the iced rendering
and scrolling architecture.

**Speedup:** Rendering time drops to O(visible_items) instead of
O(all_nodes). Combined with lazy filtering, this would be the fastest
possible path.

**Trade-offs:**

- Iced 0.13's `Scrollable` has limited support for virtual scrolling.
- Major refactor of the left panel rendering.
- Best left as a long-term architectural goal.

## Recommended Approach

**Option A** (skip re-filtering on expand/collapse) is the recommended
starting point:

- Highest impact-to-effort ratio — turning O(n) into O(1) for the exact
  operation that's slow.
- Minimal code changes — confined to two `match` arms in `update.rs`.
- No new data structures, no API changes, no test rewrites.
- The filtered trees already have the same structural shape as the
  unfiltered trees (pruning only removes subtrees; it never renames or
  reparents), so finding the corresponding node in the filtered tree is
  straightforward.

**Option B** (tantivy-unified filtering) can be layered on top for
additional speed when re-filtering does happen (query changes, mode
toggles, extension toggles).

**Option C** (`Arc`-based sharing) is worth considering if deep-cloning
remains a bottleneck after A+B are applied (e.g., when switching
between Directory / Genre / Creator views while a search is active).

## Relevant Source Files

| File | Role |
|---|---|
| `src/gui/update.rs` | Expand/collapse handlers, `recompute_filtered_nodes`, `recompute_filtered_tag_nodes` |
| `src/gui/left_panel.rs` | `filter_file_node`, `filter_tag_node` — text-based filtering |
| `src/gui/tantivy_search.rs` | `prune_file_tree`, `prune_tag_node` — index-based filtering |
| `src/gui/render_node.rs` | `render_file_node`, `render_tag_node` — recursive widget tree build |
| `src/gui/state.rs` | `FileTreeApp` struct, `perform_search`, `TagTreeNode` |
| `src/fs/file_tree.rs` | `FileNode` struct, `scan_directory` |

## Related Research

- `search-performance-research.md` — analysis of search-typing performance
  (separate bottleneck, addressed by tantivy integration)
- `tantivy-integration-research.md` — decision and design for the tantivy
  full-text search index
