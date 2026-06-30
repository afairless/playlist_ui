# File Count Indicator in Left Panel Categories

## Motivation

When browsing music files by directory, genre, or creator in the left sidebar,
all category nodes share the same uniform blue highlight. There is no visual cue
for how many audio files each category contains. Adding a **file count label**
and a **dynamic highlight intensity** based on that count gives the user an
at-a-glance sense of category size, improving scanability.

## Scope

Both tree types rendered in the left panel are affected:

| Tree type | Data structure | Rendering function |
|---|---|---|
| Directory tree | `FileNode` (file_tree.rs) | `render_file_node()` |
| Tag tree (Genre / Creator) | `TagTreeNode` (state.rs) | `render_tag_node()` |

## Approach

### 1. File count computation

Two helper functions, one for each tree type, compute the number of audio files
in a subtree. The count is computed **at render time** (not cached).

#### For `FileNode` (directory tree)

```rust
/// Recursively counts all file-node descendants under the given node.
/// Only nodes where `node_type == NodeType::File` are counted.
fn count_files_in_node(node: &FileNode) -> usize {
    match node.node_type {
        NodeType::File => 1,
        NodeType::Directory => {
            node.children.iter().map(count_files_in_node).sum()
        }
    }
}
```

A similar helper `count_files_in_tag_node` treats every leaf `TagTreeNode` that
has a non-empty `file_paths` as one file (since each leaf currently stores
exactly one file path). A non-leaf node sums all its descendants.

### 2. Count label in the display text

For **directory nodes** (`render_file_node`, the `NodeType::Directory` arm):
change the label from:

```
▶ 📁 Music
```

to:

```
▶ 📁 Music  (42)
```

For **tag tree nodes** (`render_tag_node`): append the count to non-leaf nodes.

```
▼ Rock  (128)
  ▼ Queen  (45)
    ▼ A Night at the Opera  (12)
      Bohemian Rhapsody
```

Leaf nodes (tracks) keep their current display (no count).

### 3. Dynamic highlight (background tint)

Currently the `flat_button_style` closure in `view.rs` returns a button with no
background (`background: None`), producing white text on the dark panel
background.

The new approach: **map the file count to a background `[f32; 4]` RGBA
value**. The more files, the darker/deeper the blue tint. Since `flat_button_style`
is already parameterised and used per-button, it can be turned into a
**parameterised button-style function** that takes the file count as input.

We need a mapping from `count: usize` to `iced::Color`. A natural choice is a
**log-scale interpolation** so that a handful of files yields a faint blue and
thousands yield a deep saturated blue:

```rust
/// Maps a file count to a highlight colour using log-scale interpolation.
/// Returns a deep blue for the maximum count, fading to a faint blue for
/// small counts. If `count == 0`, returns the baseline faint blue.
fn file_count_highlight(count: usize, max_count: usize) -> Color {
    let light = Color::from_rgb(0.15, 0.25, 0.55);   // faint blue
    if count == 0 || max_count == 0 {
        return light;
    }
    // Normalise count logarithmically
    let t = ((count as f64).ln() / (max_count as f64).ln()).clamp(0.0, 1.0) as f32;
    let dark  = Color::from_rgb(0.05, 0.12, 0.35);   // deep navy blue
    // Manual component interpolation (iced 0.13's Color has no lerp method)
    let t_inv = 1.0 - t;
    Color::new(
        light.r * t_inv + dark.r * t,
        light.g * t_inv + dark.g * t,
        light.b * t_inv + dark.b * t,
        1.0,
    )
}
```

However, the current `flat_button_style` closure is defined in `view.rs` and
passed down to `create_left_panel`. Each call site needs to pass the count
through. The closure signature would change to accept an extra parameter.

An alternative that avoids threading the count through the existing function
signatures: store a **per-node file count** computed in the state / update step.
This adds a `file_count: usize` field to `FileNode` and `TagTreeNode`, which is
set during tree construction (`scan_directory`, `build_genre_tag_tree`,
`build_creator_tag_tree`). **This is the recommended approach** because:

- No re-traversal of the tree at render time.
- The count is available anywhere the node is used.
- The `flat_button_style` closure can be replaced with a per-item
  `style_button_for_count(count)` helper in `render_node.rs`.

#### Changes to `FileNode` and `TagTreeNode`

```rust
// file_tree.rs
pub(crate) struct FileNode {
    pub name: String,
    pub path: PathBuf,
    pub node_type: NodeType,
    pub children: Vec<FileNode>,
    pub is_expanded: bool,
    pub file_count: usize,         // NEW
}

// state.rs
pub struct TagTreeNode {
    pub label: String,
    pub children: Vec<TagTreeNode>,
    pub file_paths: Vec<std::path::PathBuf>,
    pub is_expanded: bool,
    pub file_count: usize,         // NEW
}
```

`FileNode::new_directory` computes `file_count` as the sum of its children's
`file_count` (since children are passed into the constructor). `FileNode::new_file`
sets `file_count = 1`. No external re-sum is needed after construction.

For directory scanning: in `scan_directory` / `scan_directory_with_expansion`,
`new_directory` is called with the already-built children, so the count is set
correctly in the constructor.

For tag trees: in `build_genre_tag_tree` / `build_creator_tag_tree`, leaf track
nodes get `file_count = 1`. Non-leaf nodes get the sum of their children.

### 4. Rendering changes

#### `render_file_node` (directory nodes)

Add the file count to the label:

```rust
let label = format!(
    "{}{} 📁 {}  ({})",
    indent, expand_symbol, node.name, node.file_count,
);
```

Apply a styled button (no longer the fully flat style, but one that sets
`background` based on `node.file_count`). The button style function:

```rust
fn directory_button_style(
    count: usize,
) -> impl Fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style {
    move |_theme, _status| iced::widget::button::Style {
        background: Some(iced::Background::Color(
            file_count_highlight(count),
        )),
        border: iced::Border::default(),
        shadow: iced::Shadow::default(),
        text_color: iced::Color::WHITE,
    }
}
```

The `flat_button_style` parameter that was passed down from `view.rs` through
`create_left_panel` → `create_left_panel_file_tree_browser` → `render_file_node`
can be **removed for directory nodes** because we now use per-count styling.
File nodes (leaves) can keep the flat style, or we can keep the parameter for
them.

#### `render_tag_node` (tag tree nodes)

Same pattern: non-leaf nodes show count, leaf nodes don't. The label changes:

```rust
let count_suffix = if is_leaf {
    String::new()
} else {
    format!("  ({})", node.file_count)
};
let label = format!("{}{} {}{}", indent, expand_symbol, node.label, count_suffix);
```

Non-leaf nodes use the `directory_button_style(count)` styling; leaf nodes
continue to use the flat style.

### 5. Determining the maximum count for normalisation

The highlight intensity uses a global max count for colour interpolation. This
max can be computed once in the rendering entry points:

- `create_left_panel_file_tree_browser`: iterate `root_nodes` to find the max
  `file_count` across all directory nodes.
- `create_left_panel_tag_tree_browser`: iterate `tag_tree_roots` to find the max
  for tag nodes.

Pass this `max_count` down to `render_file_node` / `render_tag_node` as an
extra parameter so the colour function knows the scale.

Alternatively, for simplicity, use **per-tree max** (the largest node in the
current view), which avoids threading a global max through all recursive calls.

**Recommended**: use a per-tree max captured in the closure at the top-level
rendering function. Each top-level render function computes `max_count` once,
then passes it as a parameter to recursive calls.

### 6. Serialisation impact

`FileTreeApp` is serialised with `serde` and `bincode`. The new `file_count`
field on `TagTreeNode` is a `usize` (or `u64` for bincode compatibility —
bincode 2.0 requires `u64` for encode/decode). We must add `#[serde(default)]`
to the field so that old persisted data (which lacks this field) still deserialises
correctly. `TagTreeNode` already derives `Serialize`, `Deserialize`, and
`bincode::Encode`, `bincode::Decode`.

`file_count` on `FileNode` is `#[serde(skip)]` since `FileNode` is not persisted.

> **⚠️ bincode backward compatibility**: `TagTreeNode` derives `bincode::Decode`
> in addition to serde. Bincode 2.0 reads struct fields **positionally** when using
> the derive macro — adding a new field will cause deserialisation of old persisted
> blobs to fail. The `#[serde(default)]` attribute does **not** affect bincode's own
> `Decode` derive. Options:
>
> 1. Accept data loss — the old sled database will be invalidated on first launch
>    after this change. The README already documents deleting the database as a
>    known workflow.
> 2. Store `file_count` in a separate sled tree keyed by node identity.
> 3. Implement `Decode` manually for `TagTreeNode` to provide a default for
>    the new field when reading old data.
> Option 1 is the simplest and is the recommended approach for this feature.

> **Note**: `src/fs/media_metadata_async.rs` contains async variants
> (`build_tag_genre_tree_async`, `build_tag_musician_tree_async`) that also
> construct `TagTreeNode` instances. These are not currently wired into the
> update path, but if they are ever enabled, they will need the same
> `file_count` logic applied to leaf and parent nodes.

## Implementation steps (commit-by-commit)

### Step 1: Add `file_count` fields and populate them during tree construction

**Files touched**:

- `src/fs/file_tree.rs`
  - `FileNode`: add `pub file_count: usize` field
  - `new_file()`: set `file_count = 1`
  - `new_directory()`: compute `file_count = children.iter().map(|c| c.file_count).sum()`
    inside the constructor body (no external post-set needed)
  - `scan_directory_with_expansion()`: no longer needs to set file_count manually
    since `new_directory` handles it
  - Adjust existing `new_directory()` calls in tests if they pass positional args
- `src/gui/state.rs`
  - `TagTreeNode`: add `pub file_count: usize` field with `#[serde(default)]`
- `src/fs/media_metadata.rs`
  - `build_genre_tag_tree()`: set `file_count = 1` on leaf track nodes; compute
    and set for album / artist / genre parents
  - `build_creator_tag_tree()`: same

**Tests**: Cover the None-One-Many principle:

- **None**: empty `FileNode::new_directory` with no children — expects `file_count == 0`
- **One**: single-file directory — expects `file_count == 1`
- **Many**: multi-level nested directories with multiple leaf files — expects
  `file_count` equal to the total leaf count
- **Tag tree**: leaf track nodes (`file_count == 1`), album nodes (sum of tracks),
  artist nodes (sum of albums), genre nodes (sum of artists)
- **Edge cases**: empty tag tree roots, node with empty `file_paths`

### Step 2: Add count labels and dynamic background styling in directory rendering

**Files touched**:

- `src/gui/render_node.rs`
  - `render_file_node()`: for `NodeType::Directory`, append count to label;
    apply per-count background via a new style helper
  - `render_tag_node()`: same for non-leaf nodes
- `src/gui/view.rs`
  - Update `flat_button_style` — file nodes keep it, directory nodes no longer
    use it
  - Compute `max_count` per tree in the top-level browser functions

**Tests**:

- Unit-test `file_count_highlight()` for boundary values:
  - `file_count_highlight(0, 42)` — count of 0 returns light baseline
  - `file_count_highlight(1, 42)` — minimum non-zero count
  - `file_count_highlight(42, 42)` — max boundary returns darkest
  - Property-based: for any `count < other_count`, output luminance is monotonic
- Verify via helper/test function that label formatting produces `"📁 Dir  (5)"`
  for directory nodes and no count suffix for leaf TagTreeNodes

### Step 3: Cleanup and verification

- Run `cargo test --all` to ensure no regressions
- Run `cargo build` and manually verify the UI renders correctly
- Remove the `flat_button_style` parameter from directory-rendering paths if it
  is no longer needed

> **Clarification on style threading**: The current call chain passes
> `flat_button_style` through `view()` → `create_left_panel()` →
> `create_left_panel_file_tree_browser()` → `render_file_node()`. Rather than
> threading two separate style closures, the simplest approach is to keep
> `flat_button_style` but change its signature to accept an optional count:
> `flat_button_style(count: Option<usize>)`. File nodes pass `None` (keep flat)
> and directory nodes pass `Some(node.file_count)`. The `flat_button_style`
> closure in `view.rs` then branches: `None` → no background (flat),
> `Some(count)` → call `file_count_highlight(count, max_count)`.

## Future considerations

1. **Performance**: Counts are computed once during tree construction. If the
   user adds/removes files at runtime, the trees are rebuilt anyway
   (`scan_directory` or `ToggleExtension`), so counts stay accurate.
2. **Maximum count computation**: If the tree has thousands of nodes, iterating
   to find `max_count` each frame could be wasteful. In practice the left-panel
   tree is at most a few hundred nodes, so this is negligible. Caching `max_count`
   on the `FileTreeApp` state is a straightforward future optimisation.
3. **Colour scheme**: The blue highlight is hard-coded. In the future it could
   be exposed as a `MenuStyle` / theme parameter.
