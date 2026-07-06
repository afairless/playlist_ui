//! Left-panel UI construction for the Playlist UI.
//!
//! Builds the left sidebar containing the menu row (Add Directory, sort
//! toggle, panel toggle), the file-extension filter menu, and either a
//! directory tree or a tag-based navigation tree (genre/creator) depending
//! on the current `LeftPanelSelectMode`.
//!
//! Before rendering, tag-tree root nodes are sorted via
//! [`sort_tag_tree_roots`] according to the active sort mode
//! (`Alphanumeric`, `ModifiedDate`, or `FileCount`).  Child-level sorting
//! is handled inside the recursive `render_tag_node` in `render_node.rs`.
//!
//! Public API:
//!     create_left_panel — assemble the full left-panel Element

use crate::fs::file_tree::FileNode;
use crate::fs::media_metadata::extract_media_metadata;
use crate::gui::render_node::{render_file_node, render_tag_node};
use crate::gui::view::{MenuStyle, TreeBrowserStyle};
use crate::gui::{
    FileTreeApp, LeftPanelSelectMode, LeftPanelSortMode, Message, TagTreeNode,
    TextSearchMode,
};
use crate::utils::file_field_matches;
use std::path::Path;

use iced::{
    Element,
    widget::{Space, button, column, row, text, text_input},
};

/// Creates the toggle button for the left panel, displaying either a left or
/// right arrow depending on the current expansion state. The button uses the
/// specified menu style for text size and triggers the `ToggleLeftPanel`
/// message when pressed.
fn create_toggle_left_panel_button(
    app: &FileTreeApp,
    menu_style: MenuStyle,
) -> iced::widget::Button<'_, Message> {
    // toggle appearance of left panel
    button(
        text(if app.left_panel_expanded { "←" } else { "→" })
            .size(menu_style.text_size),
    )
    .on_press(Message::ToggleLeftPanel)
}

/// Constructs the left panel's menu row containing the "Add Directory" button
/// and the file extension menu, applying the specified text size, spacing, and
/// color styling to both buttons.
fn create_left_panel_menu_row<'a>(
    app: &'a FileTreeApp,
    menu_style: MenuStyle,
) -> Element<'a, Message> {
    let toggle_left_panel_button =
        create_toggle_left_panel_button(app, menu_style);
    let directory_button =
        iced::widget::button::<Message, iced::Theme, iced::Renderer>(
            iced::widget::text("Add Directory")
                .size(menu_style.text_size)
                .style(move |_theme| iced::widget::text::Style {
                    color: Some(menu_style.text_color.into()),
                }),
        )
        .on_press(Message::AddDirectory);

    let sort_mode_label = match app.left_panel_sort_mode {
        LeftPanelSortMode::Alphanumeric => "Sort: Name",
        LeftPanelSortMode::ModifiedDate => "Sort: Date Modified",
        LeftPanelSortMode::FileCount => "Sort: File Count",
    };
    let sort_mode_button =
        iced::widget::button::<Message, iced::Theme, iced::Renderer>(
            iced::widget::text(sort_mode_label)
                .size(menu_style.text_size)
                .style(move |_theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(menu_style.text_color.into()),
                }),
        )
        .on_press(Message::ToggleLeftPanelSortMode);

    iced::widget::row![
        toggle_left_panel_button,
        directory_button,
        sort_mode_button,
    ]
    .spacing(menu_style.spacing)
    .into()
}

/// Creates the file extension filter menu for the left panel, including a
/// styled header button that toggles the menu and a list of extension toggle
/// buttons. The menu appearance is controlled by the given text size and color.
fn create_extension_menu(
    app: &FileTreeApp,
    menu_size: u16,
    menu_text_color: [f32; 4],
) -> Element<'_, Message> {
    let header = button(
        text(if app.extensions_menu_expanded {
            "▼ File Extensions"
        } else {
            "▶ File Extensions"
        })
        .size(menu_size)
        .style(move |_theme| iced::widget::text::Style {
            color: Some(menu_text_color.into()),
        }),
    )
    .on_press(Message::ToggleExtensionsMenu);

    if app.extensions_menu_expanded {
        let mut menu = column![];
        for ext in &app.all_extensions {
            let checked = app.selected_extensions.contains(ext);
            let label = if checked {
                format!("[x] .{ext}")
            } else {
                format!("[ ] .{ext}")
            };
            menu = menu.push(
                button(text(label))
                    .on_press(Message::ToggleExtension(ext.clone())),
            );
        }
        column![header, menu].into()
    } else {
        column![header].into()
    }
}

/// Builds the column of directory trees for the left panel, including directory
/// headers and file trees, with configurable spacing between rows and directory
/// name text size.
fn create_left_panel_file_tree_browser(
    app: &FileTreeApp,
    tree_browser_style: TreeBrowserStyle,
    flat_button_style: impl Fn(
        &iced::Theme,
        iced::widget::button::Status,
    ) -> iced::widget::button::Style
    + Copy
    + 'static,
) -> iced::widget::Column<'_, Message> {
    let gap_width = tree_browser_style.remove_button_width / 4;
    let is_searching = !app.search_query.is_empty();

    // Use filtered nodes when search is active, otherwise use original nodes
    let nodes =
        if is_searching { &app.filtered_root_nodes } else { &app.root_nodes };

    // Compute max file_count across all directory nodes
    let max_count =
        nodes.iter().flatten().map(|n| n.file_count).max().unwrap_or(0);

    let mut trees = column![];
    for (i, node_opt) in nodes.iter().enumerate() {
        let dir_path = app.top_dirs.get(i).cloned().unwrap_or_default();

        let content = if let Some(node) = node_opt {
            render_file_node(
                node,
                0,
                tree_browser_style.directory_row_size,
                tree_browser_style.file_row_size,
                app.left_panel_sort_mode,
                flat_button_style,
                max_count,
            )
        } else {
            text("No files found").into()
        };

        if is_searching {
            // Hide remove buttons during search to avoid index-alignment
            // issues with filtered root nodes
            trees = trees.push(content);
        } else {
            let remove_button =
                button(text("X").size(tree_browser_style.directory_row_size))
                    .width(tree_browser_style.remove_button_width - gap_width)
                    .on_press(Message::RemoveTopDir(dir_path.clone()));

            let row =
                row![content, Space::with_width(gap_width), remove_button,]
                    .align_y(iced::Alignment::Start);

            trees = trees.push(row);
        }
        trees =
            trees.push(Space::with_height(tree_browser_style.tree_row_height));
    }
    trees
}

/// Builds the left panel's tag-based navigation/selection tree UI.
///
/// Iterates over the root nodes of the tag tree in the application state and
/// recursively renders each node using `render_tag_node`. Applies the specified
/// tree browser style for row sizing and spacing. This function is used when
/// the left panel is in tag navigation/selection  mode to display the genre →
/// creator/musician/artist → album → track hierarchy.
fn create_left_panel_tag_tree_browser(
    app: &FileTreeApp,
    tree_browser_style: TreeBrowserStyle,
    flat_button_style: impl Fn(
        &iced::Theme,
        iced::widget::button::Status,
    ) -> iced::widget::button::Style
    + Copy
    + 'static,
) -> iced::widget::Column<'_, Message> {
    let is_searching = !app.search_query.is_empty();

    // Use filtered tag tree roots when search is active
    let tag_roots = if is_searching {
        &app.filtered_tag_tree_roots
    } else {
        &app.tag_tree_roots
    };

    // Compute max file_count across all tag tree root nodes
    let max_count = tag_roots.iter().map(|n| n.file_count).max().unwrap_or(0);

    // Sort root indices according to the current sort mode, then render in
    // that order. We use index-based sorting to avoid borrowing a local copy
    // when the return lifetime is tied to &app.
    let mut indices: Vec<usize> = (0..tag_roots.len()).collect();
    sort_tag_tree_roots(&mut indices, tag_roots, app.left_panel_sort_mode);

    let mut trees = column![];
    for &i in &indices {
        trees = trees.push(render_tag_node(
            &tag_roots[i],
            0,
            vec![],
            tree_browser_style.directory_row_size,
            app.left_panel_sort_mode,
            flat_button_style,
            max_count,
        ));
        trees =
            trees.push(Space::with_height(tree_browser_style.tree_row_height));
    }
    trees
}

/// Sorts indices into the `roots` slice according to the given sort mode.
///
/// After this function returns, `indices` is permuted so that iterating
/// `roots[indices[i]]` yields nodes in the desired order.
///
/// * `Alphanumeric` — ascending by label (case-insensitive).
/// * `ModifiedDate` — descending by modification time of the first file path,
///   falling back to alphabetical order when timestamps are unavailable.
/// * `FileCount` — descending by `file_count`, then ascending by label as
///   tiebreaker.
fn sort_tag_tree_roots(
    indices: &mut [usize],
    roots: &[TagTreeNode],
    sort_mode: LeftPanelSortMode,
) {
    match sort_mode {
        LeftPanelSortMode::Alphanumeric => {
            indices.sort_by(|&i, &j| {
                roots[i]
                    .label
                    .to_lowercase()
                    .cmp(&roots[j].label.to_lowercase())
            });
        },
        // NOTE: Non-leaf tag tree nodes have empty file_paths, so for
        // root-level nodes this comparator always sees None == None and
        // produces no effective sort. This is a pre-existing limitation
        // shared with the child-level sort in render_tag_node. The
        // `.then_with` fallback ensures roots are at least sorted
        // alphabetically when timestamps are missing.
        LeftPanelSortMode::ModifiedDate => {
            indices.sort_by(|&i, &j| {
                let a_time = roots[i]
                    .file_paths
                    .first()
                    .and_then(|p| std::fs::metadata(p).ok())
                    .and_then(|m| m.modified().ok());
                let b_time = roots[j]
                    .file_paths
                    .first()
                    .and_then(|p| std::fs::metadata(p).ok())
                    .and_then(|m| m.modified().ok());
                b_time.cmp(&a_time).then_with(|| {
                    roots[i]
                        .label
                        .to_lowercase()
                        .cmp(&roots[j].label.to_lowercase())
                })
            });
        },
        LeftPanelSortMode::FileCount => {
            indices.sort_by(|&i, &j| {
                roots[j].file_count.cmp(&roots[i].file_count).then_with(|| {
                    roots[i]
                        .label
                        .to_lowercase()
                        .cmp(&roots[j].label.to_lowercase())
                })
            });
        },
    }
}

/// Creates the search row UI containing a text input, an optional clear
/// button (✕), and a mode toggle button. The clear button is only shown
/// when `search_query` is non-empty. The search row is hidden when the
/// left panel is collapsed.
fn create_search_row(
    app: &FileTreeApp,
    menu_style: MenuStyle,
    flat_button_style: impl Fn(
        &iced::Theme,
        iced::widget::button::Status,
    ) -> iced::widget::button::Style
    + Copy
    + 'static,
) -> Element<'_, Message> {
    let mode_label = match app.search_mode {
        TextSearchMode::All => "🔍 All",
        TextSearchMode::DirectoryPath => "🔍 Path",
        TextSearchMode::TrackFilename => "🔍 File",
        TextSearchMode::Creator => "🔍 Artist",
        TextSearchMode::Album => "🔍 Album",
        TextSearchMode::Title => "🔍 Title",
        TextSearchMode::Genre => "🔍 Genre",
    };

    let search_input = text_input::<Message, iced::Theme, iced::Renderer>(
        "Search...",
        &app.search_query,
    )
    .on_input(Message::SearchQueryChanged);

    let clear_button = if !app.search_query.is_empty() {
        Some(
            button(text("✕").size(menu_style.text_size))
                .on_press(Message::SearchCleared)
                .style(flat_button_style)
                .into(),
        )
    } else {
        None
    };

    let mode_button = button(text(mode_label).size(menu_style.text_size))
        .on_press(Message::ToggleSearchMode);

    let mut children: Vec<Element<'_, Message>> = vec![search_input.into()];
    if let Some(btn) = clear_button {
        children.push(btn);
    }
    children.push(mode_button.into());

    row(children).spacing(menu_style.spacing).into()
}

/// Checks whether a file's metadata matches the given search mode and query.
/// Extracts metadata from the file path and checks the relevant field.
/// Used internally by `filter_file_node` for metadata-based filtering.
fn file_matches_mode(path: &Path, mode: TextSearchMode, query: &str) -> bool {
    let meta = extract_media_metadata(path);
    match mode {
        TextSearchMode::Creator => file_field_matches(&meta.creator, query),
        TextSearchMode::Album => file_field_matches(&meta.album, query),
        TextSearchMode::Title => file_field_matches(&meta.title, query),
        TextSearchMode::Genre => file_field_matches(&meta.genre, query),
        TextSearchMode::All => {
            file_field_matches(&meta.creator, query)
                || file_field_matches(&meta.album, query)
                || file_field_matches(&meta.title, query)
                || file_field_matches(&meta.genre, query)
        },
        TextSearchMode::DirectoryPath | TextSearchMode::TrackFilename => false,
    }
}

/// Recursively filters a `FileNode` tree, keeping only nodes that match the
/// current search query and mode. Returns `Some(FileNode)` when the node or
/// at least one descendant matches, or `None` when no match is found.
pub(crate) fn filter_file_node(
    node: &FileNode,
    query: &str,
    mode: TextSearchMode,
) -> Option<FileNode> {
    if query.is_empty() {
        return Some(node.clone());
    }

    let query_lower = query.to_ascii_lowercase();

    // Check if this node itself matches
    let node_matches = match mode {
        TextSearchMode::All => {
            let path_match = node
                .path
                .to_string_lossy()
                .to_ascii_lowercase()
                .contains(&query_lower);
            let name_match =
                node.name.to_ascii_lowercase().contains(&query_lower);
            let metadata_match = match &node.node_type {
                crate::fs::file_tree::NodeType::File => {
                    file_matches_mode(&node.path, mode, query)
                },
                crate::fs::file_tree::NodeType::Directory => false,
            };
            path_match || name_match || metadata_match
        },
        TextSearchMode::DirectoryPath => node
            .path
            .to_string_lossy()
            .to_ascii_lowercase()
            .contains(&query_lower),
        TextSearchMode::TrackFilename => {
            node.name.to_ascii_lowercase().contains(&query_lower)
        },
        TextSearchMode::Creator
        | TextSearchMode::Album
        | TextSearchMode::Title
        | TextSearchMode::Genre => match &node.node_type {
            crate::fs::file_tree::NodeType::File => {
                file_matches_mode(&node.path, mode, query)
            },
            crate::fs::file_tree::NodeType::Directory => false,
        },
    };

    // For file nodes: keep if matching, drop otherwise
    if matches!(node.node_type, crate::fs::file_tree::NodeType::File) {
        return if node_matches { Some(node.clone()) } else { None };
    }

    // For directory nodes: recursively filter children
    let filtered_children: Vec<FileNode> = node
        .children
        .iter()
        .filter_map(|child| filter_file_node(child, query, mode))
        .collect();

    if node_matches || !filtered_children.is_empty() {
        let mut cloned = node.clone();
        cloned.children = filtered_children;
        cloned.file_count =
            cloned.children.iter().map(|c| c.file_count).sum();
        Some(cloned)
    } else {
        None
    }
}

/// Recursively filters a `TagTreeNode` tree, keeping only nodes that
/// match the search query according to the active search mode.
///
/// Matching strategy:
/// - **Metadata modes** (`Creator`, `Album`, `Title`, `Genre`): match
///   against node labels only — the tag tree labels are the metadata
///   values (no disk I/O).
/// - **`All` mode**: match by label or by file path / filename.
/// - **`DirectoryPath` mode**: match by label or by file path
///   substring.
/// - **`TrackFilename` mode**: match by label or by filename substring.
///
/// When a non-leaf node matches by label, all its children are kept.
/// When it does not match, children are pruned recursively. Returns
/// `None` when no match is found in the subtree.
pub(crate) fn filter_tag_node(
    node: &TagTreeNode,
    query: &str,
    mode: TextSearchMode,
) -> Option<TagTreeNode> {
    if query.is_empty() {
        return Some(node.clone());
    }

    let query_lower = query.to_ascii_lowercase();
    let label_matches = node.label.to_ascii_lowercase().contains(&query_lower);

    // For path/file modes, check file_paths directly (no metadata I/O).
    let path_matches = !node.file_paths.is_empty()
        && match mode {
            TextSearchMode::DirectoryPath => node.file_paths.iter().any(|p| {
                p.to_string_lossy().to_ascii_lowercase().contains(&query_lower)
            }),
            TextSearchMode::TrackFilename => node.file_paths.iter().any(|p| {
                p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_ascii_lowercase()
                    .contains(&query_lower)
            }),
            TextSearchMode::All => node.file_paths.iter().any(|p| {
                let path_str = p.to_string_lossy().to_ascii_lowercase();
                let name_str = p
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_ascii_lowercase();
                path_str.contains(&query_lower)
                    || name_str.contains(&query_lower)
            }),
            // Metadata modes: label matching is sufficient — the node's
            // label already represents the metadata category.
            TextSearchMode::Creator
            | TextSearchMode::Album
            | TextSearchMode::Title
            | TextSearchMode::Genre => false,
        };

    if node.children.is_empty() {
        // Leaf node (track)
        let matches = match mode {
            TextSearchMode::Creator
            | TextSearchMode::Album
            | TextSearchMode::Title
            | TextSearchMode::Genre => label_matches,
            TextSearchMode::All => label_matches || path_matches,
            TextSearchMode::DirectoryPath | TextSearchMode::TrackFilename => {
                label_matches || path_matches
            },
        };
        return if matches { Some(node.clone()) } else { None };
    }

    // Non-leaf node
    if label_matches {
        // Label matches — keep the node with all children
        Some(node.clone())
    } else {
        // Prune children, keep only matching subtrees
        let filtered_children: Vec<TagTreeNode> = node
            .children
            .iter()
            .filter_map(|child| filter_tag_node(child, query, mode))
            .collect();
        if filtered_children.is_empty() {
            None
        } else {
            let mut cloned = node.clone();
            cloned.children = filtered_children;
            cloned.file_count =
                cloned.children.iter().map(|c| c.file_count).sum();
            Some(cloned)
        }
    }
}

/// Constructs the left panel UI for the application, including the menu row,
/// file extension filter menu, and either the directory or tag tree browser
/// depending on the current navigation/selection mode. The panel's appearance
/// and behavior are controlled by the provided style parameters and button
/// style function. Returns an `Element<Message>` representing the left panel's
/// content, which adapts to the expansion state and navigation/selection mode.
pub(crate) fn create_left_panel(
    app: &FileTreeApp,
    menu_style: MenuStyle,
    tree_browser_style: TreeBrowserStyle,
    flat_button_style: impl Fn(
        &iced::Theme,
        iced::widget::button::Status,
    ) -> iced::widget::button::Style
    + Copy
    + 'static,
) -> Element<'_, Message> {
    //
    // left_panel_menu_row_1
    // --------------------------------------------------

    let left_panel_menu_row_1 = create_left_panel_menu_row(app, menu_style);

    let selection_mode_label = match app.left_panel_selection_mode {
        LeftPanelSelectMode::Directory => "Select by: Directory",
        LeftPanelSelectMode::GenreTag => "Select by: Genre",
        LeftPanelSelectMode::CreatorTag => "Select by: Creator",
    };
    let selection_mode_button =
        iced::widget::button::<Message, iced::Theme, iced::Renderer>(
            iced::widget::text(selection_mode_label)
                .size(menu_style.text_size)
                .style(move |_theme| iced::widget::text::Style {
                    color: Some(menu_style.text_color.into()),
                }),
        )
        .on_press(Message::ToggleLeftPanelSelectMode);

    //
    // left_panel_menu_row_2
    // --------------------------------------------------

    let extension_menu =
        create_extension_menu(app, menu_style.text_size, menu_style.text_color);
    let left_panel_menu_row_2 =
        iced::widget::row![selection_mode_button, extension_menu]
            .spacing(menu_style.spacing);

    //
    // tree_browser
    // --------------------------------------------------

    let tree_browser = match app.left_panel_selection_mode {
        LeftPanelSelectMode::Directory => create_left_panel_file_tree_browser(
            app,
            tree_browser_style,
            flat_button_style,
        ),
        LeftPanelSelectMode::GenreTag | LeftPanelSelectMode::CreatorTag => {
            create_left_panel_tag_tree_browser(
                app,
                tree_browser_style,
                flat_button_style,
            )
        },
    };

    //
    // assemble components into panel
    // --------------------------------------------------

    let left_content = if app.left_panel_expanded {
        column![
            left_panel_menu_row_1,
            Space::with_height(10),
            left_panel_menu_row_2,
            Space::with_height(10),
            create_search_row(app, menu_style, flat_button_style),
            Space::with_height(5),
            tree_browser,
        ]
    } else {
        column![create_toggle_left_panel_button(app, menu_style)]
    };
    left_content.into()
}

#[cfg(test)]
mod tests {
    use super::create_search_row;
    use super::sort_tag_tree_roots;
    use super::{filter_file_node, filter_tag_node};
    use crate::fs::file_tree::FileNode;
    use crate::gui::state::TagTreeNode;
    use crate::gui::view::MenuStyle;
    use crate::gui::{FileTreeApp, LeftPanelSortMode, TextSearchMode};
    use std::path::PathBuf;

    /// Helper to build a TagTreeNode with the given label and file_count.
    fn node(label: &str, file_count: usize) -> TagTreeNode {
        TagTreeNode {
            label: label.to_string(),
            children: vec![],
            file_paths: vec![],
            is_expanded: false,
            file_count,
        }
    }

    #[test]
    fn test_sort_roots_alphanumeric() {
        let roots =
            vec![node("z_genre", 10), node("a_genre", 20), node("m_genre", 15)];
        let mut indices: Vec<usize> = (0..roots.len()).collect();
        sort_tag_tree_roots(
            &mut indices,
            &roots,
            LeftPanelSortMode::Alphanumeric,
        );
        let sorted: Vec<&str> =
            indices.iter().map(|&i| roots[i].label.as_str()).collect();
        assert_eq!(sorted, vec!["a_genre", "m_genre", "z_genre"]);
    }

    #[test]
    fn test_sort_roots_file_count_descending() {
        let roots =
            vec![node("root_c", 50), node("root_b", 100), node("root_a", 30)];
        let mut indices: Vec<usize> = (0..roots.len()).collect();
        sort_tag_tree_roots(&mut indices, &roots, LeftPanelSortMode::FileCount);
        let sorted: Vec<&str> =
            indices.iter().map(|&i| roots[i].label.as_str()).collect();
        assert_eq!(sorted, vec!["root_b", "root_c", "root_a"]);
    }

    #[test]
    fn test_sort_roots_file_count_tiebreaker() {
        let roots = vec![node("b_label", 50), node("a_label", 50)];
        let mut indices: Vec<usize> = (0..roots.len()).collect();
        sort_tag_tree_roots(&mut indices, &roots, LeftPanelSortMode::FileCount);
        let sorted: Vec<&str> =
            indices.iter().map(|&i| roots[i].label.as_str()).collect();
        // Same file_count → alphabetical tiebreaker
        assert_eq!(sorted, vec!["a_label", "b_label"]);
    }

    #[test]
    fn test_sort_roots_alphanumeric_mixed_case() {
        let roots =
            vec![node("Z_genre", 10), node("a_genre", 20), node("M_genre", 15)];
        let mut indices: Vec<usize> = (0..roots.len()).collect();
        sort_tag_tree_roots(
            &mut indices,
            &roots,
            LeftPanelSortMode::Alphanumeric,
        );
        let sorted: Vec<&str> =
            indices.iter().map(|&i| roots[i].label.as_str()).collect();
        assert_eq!(sorted, vec!["a_genre", "M_genre", "Z_genre"]);
    }

    #[test]
    fn test_sort_roots_file_count_single() {
        let roots = vec![node("only", 42)];
        let mut indices: Vec<usize> = (0..roots.len()).collect();
        sort_tag_tree_roots(&mut indices, &roots, LeftPanelSortMode::FileCount);
        let sorted: Vec<&str> =
            indices.iter().map(|&i| roots[i].label.as_str()).collect();
        assert_eq!(sorted, vec!["only"]);
    }

    #[test]
    fn test_sort_roots_empty() {
        let roots: Vec<TagTreeNode> = vec![];
        let mut indices: Vec<usize> = vec![];
        sort_tag_tree_roots(&mut indices, &roots, LeftPanelSortMode::FileCount);
        assert!(indices.is_empty());
    }

    #[test]
    fn test_sort_roots_modified_date_empty_paths() {
        let roots =
            vec![node("c_genre", 10), node("a_genre", 20), node("b_genre", 15)];
        let mut indices: Vec<usize> = (0..roots.len()).collect();
        // All file_paths are empty, so ModifiedDate falls back to alphabetical
        sort_tag_tree_roots(
            &mut indices,
            &roots,
            LeftPanelSortMode::ModifiedDate,
        );
        let sorted: Vec<&str> =
            indices.iter().map(|&i| roots[i].label.as_str()).collect();
        assert_eq!(sorted, vec!["a_genre", "b_genre", "c_genre"]);
    }

    #[test]
    fn test_sort_roots_modified_date_no_panic() {
        // Empty file_paths on all nodes should not panic
        let roots = vec![node("x", 5), node("y", 3)];
        let mut indices: Vec<usize> = (0..roots.len()).collect();
        sort_tag_tree_roots(
            &mut indices,
            &roots,
            LeftPanelSortMode::ModifiedDate,
        );
        // Should complete without panic
        assert_eq!(indices.len(), 2);
    }

    /// A simple flat-button style for use in tests.
    fn test_flat_button_style(
        _theme: &iced::Theme,
        _status: iced::widget::button::Status,
    ) -> iced::widget::button::Style {
        iced::widget::button::Style { background: None, ..Default::default() }
    }

    #[test]
    fn test_create_search_row_does_not_panic() {
        let menu_style = MenuStyle {
            text_size: 20,
            spacing: 10,
            text_color: [0.0, 1.0, 1.0, 1.0],
        };
        let app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        let _element =
            create_search_row(&app, menu_style, test_flat_button_style);
    }

    #[test]
    fn test_create_search_row_with_query() {
        let menu_style = MenuStyle {
            text_size: 20,
            spacing: 10,
            text_color: [0.0, 1.0, 1.0, 1.0],
        };
        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.search_query = "test".to_string();
        let _element =
            create_search_row(&app, menu_style, test_flat_button_style);
    }

    #[test]
    fn test_create_search_row_with_various_modes() {
        let menu_style = MenuStyle {
            text_size: 20,
            spacing: 10,
            text_color: [0.0, 1.0, 1.0, 1.0],
        };
        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        for mode in &[
            TextSearchMode::All,
            TextSearchMode::DirectoryPath,
            TextSearchMode::TrackFilename,
            TextSearchMode::Creator,
            TextSearchMode::Album,
            TextSearchMode::Title,
            TextSearchMode::Genre,
        ] {
            app.search_mode = *mode;
            let _element =
                create_search_row(&app, menu_style, test_flat_button_style);
        }
    }

    #[test]
    fn test_create_search_row_clear_button_present_with_query() {
        let menu_style = MenuStyle {
            text_size: 20,
            spacing: 10,
            text_color: [0.0, 1.0, 1.0, 1.0],
        };
        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.search_query = "something".to_string();
        // Should not panic when query is non-empty (clear button rendered)
        let _element =
            create_search_row(&app, menu_style, test_flat_button_style);
    }

    #[test]
    fn test_create_search_row_clear_button_absent_when_empty() {
        let menu_style = MenuStyle {
            text_size: 20,
            spacing: 10,
            text_color: [0.0, 1.0, 1.0, 1.0],
        };
        let app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        // Empty query — clear button should not be rendered
        let _element =
            create_search_row(&app, menu_style, test_flat_button_style);
    }

    // ── filter_file_node tests ──────────────────────────────────────────

    /// Helper to build a file node for testing.
    fn test_file(name: &str, path: &str) -> FileNode {
        FileNode::new_file(name.to_string(), PathBuf::from(path))
    }

    /// Helper to build a directory node with children.
    fn test_dir(name: &str, path: &str, children: Vec<FileNode>) -> FileNode {
        FileNode::new_directory(name.to_string(), PathBuf::from(path), children)
    }

    #[test]
    fn test_filter_empty_query_returns_some() {
        let node = test_file("song.mp3", "/music/song.mp3");
        let result = filter_file_node(&node, "", TextSearchMode::All);
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "song.mp3");
    }

    #[test]
    fn test_filter_empty_query_preserves_directory_children() {
        let child = test_file("track.flac", "/dir/track.flac");
        let dir = test_dir("my_dir", "/dir", vec![child]);
        let result = filter_file_node(&dir, "", TextSearchMode::All);
        assert!(result.is_some());
        assert_eq!(result.unwrap().children.len(), 1);
    }

    #[test]
    fn test_filter_path_match_directory_path_mode() {
        let node = test_file("song.mp3", "/music/rock/song.mp3");
        // DirectoryPath mode matches against the full path
        let result =
            filter_file_node(&node, "rock", TextSearchMode::DirectoryPath);
        assert!(result.is_some());
    }

    #[test]
    fn test_filter_path_nomatch_directory_path_mode() {
        let node = test_file("song.mp3", "/music/rock/song.mp3");
        let result =
            filter_file_node(&node, "jazz", TextSearchMode::DirectoryPath);
        assert!(result.is_none());
    }

    #[test]
    fn test_filter_filename_match() {
        let node = test_file("my_song.mp3", "/music/my_song.mp3");
        let result =
            filter_file_node(&node, "my_song", TextSearchMode::TrackFilename);
        assert!(result.is_some());
    }

    #[test]
    fn test_filter_filename_nomatch() {
        let node = test_file("my_song.mp3", "/music/my_song.mp3");
        let result = filter_file_node(
            &node,
            "other_song",
            TextSearchMode::TrackFilename,
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_filter_directory_with_matching_child() {
        let child = test_file("target.mp3", "/dir/target.mp3");
        let other = test_file("other.mp3", "/dir/other.mp3");
        let dir = test_dir("my_dir", "/dir", vec![child, other]);
        let result =
            filter_file_node(&dir, "target", TextSearchMode::TrackFilename);
        assert!(result.is_some());
        let filtered = result.unwrap();
        // Directory kept but only with matching children
        assert_eq!(filtered.children.len(), 1);
        assert_eq!(filtered.children[0].name, "target.mp3");
    }

    #[test]
    fn test_filter_directory_with_matching_path() {
        let child = test_file("track.mp3", "/music/jazz/track.mp3");
        let dir = test_dir("jazz_collection", "/music/jazz", vec![child]);
        // Directory path matches "jazz" in DirectoryPath mode
        let result =
            filter_file_node(&dir, "jazz", TextSearchMode::DirectoryPath);
        assert!(result.is_some());
        let filtered = result.unwrap();
        // Directory kept with all children since the directory itself matches
        assert_eq!(filtered.children.len(), 1);
    }

    #[test]
    fn test_filter_directory_with_no_matches() {
        let child = test_file("track.mp3", "/music/track.mp3");
        let dir = test_dir("my_dir", "/music", vec![child]);
        let result = filter_file_node(
            &dir,
            "nonexistent",
            TextSearchMode::TrackFilename,
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_filter_case_insensitive() {
        let node = test_file("Song.MP3", "/Music/Song.MP3");
        let result =
            filter_file_node(&node, "song", TextSearchMode::TrackFilename);
        assert!(result.is_some());
    }

    #[test]
    fn test_filter_all_mode_matches_path() {
        let node = test_file("track.mp3", "/music/jazz/track.mp3");
        let result = filter_file_node(&node, "jazz", TextSearchMode::All);
        assert!(result.is_some());
    }

    #[test]
    fn test_filter_all_mode_matches_filename() {
        let node = test_file("track.mp3", "/music/jazz/track.mp3");
        let result = filter_file_node(&node, "track", TextSearchMode::All);
        assert!(result.is_some());
    }

    #[test]
    fn test_filter_all_mode_no_match() {
        let node = test_file("track.mp3", "/music/jazz/track.mp3");
        let result =
            filter_file_node(&node, "nonexistent", TextSearchMode::All);
        assert!(result.is_none());
    }

    #[test]
    fn test_filter_empty_directory_with_children_retains() {
        // A directory matching by name keeps all children
        let child = test_file("song.mp3", "/target/song.mp3");
        let dir = test_dir("target", "/target", vec![child]);
        let result =
            filter_file_node(&dir, "target", TextSearchMode::DirectoryPath);
        assert!(result.is_some());
        assert_eq!(result.unwrap().children.len(), 1);
    }

    // ── filter_file_node file_count recalculation tests ──────────────────

    #[test]
    fn test_filter_file_node_recalculates_file_count_on_child_prune() {
        // Directory with 3 files, search matches only 1
        // (parent path does not match)
        let children = vec![
            test_file("miles_track.mp3", "/music/jazz/miles_track.mp3"),
            test_file("john_track.mp3", "/music/jazz/john_track.mp3"),
            test_file("thelo_track.mp3", "/music/jazz/thelo_track.mp3"),
        ];
        let dir = test_dir("jazz", "/music/jazz", children);
        let result =
            filter_file_node(&dir, "miles", TextSearchMode::TrackFilename);
        assert!(result.is_some());
        let filtered = result.unwrap();
        assert_eq!(filtered.children.len(), 1);
        assert_eq!(filtered.file_count, 1);
    }

    #[test]
    fn test_filter_file_node_recalculates_file_count_when_parent_matches() {
        // Directory named "jazz" with 3 files, none of which have "jazz"
        // in their filename. In TrackFilename mode, the directory name
        // matches (node_matches = true), but children are filtered by
        // filename only — so only 1 child matches.
        let children = vec![
            test_file("jazz_song.mp3", "/music/other/jazz_song.mp3"),
            test_file("rock_song.mp3", "/music/other/rock_song.mp3"),
            test_file("blues_song.mp3", "/music/other/blues_song.mp3"),
        ];
        let dir = test_dir("jazz", "/music/other", children);
        let result = filter_file_node(
            &dir,
            "jazz",
            TextSearchMode::TrackFilename,
        );
        assert!(result.is_some());
        let filtered = result.unwrap();
        // Directory kept (node_matches) but only 1 child survives
        assert_eq!(filtered.children.len(), 1);
        assert_eq!(filtered.children[0].name, "jazz_song.mp3");
        assert_eq!(filtered.file_count, 1);
    }

    #[test]
    fn test_filter_file_node_maintains_file_count_when_empty_query() {
        // Directory filtered with empty query
        let children = vec![
            test_file("track_a.mp3", "/music/jazz/track_a.mp3"),
            test_file("track_b.mp3", "/music/jazz/track_b.mp3"),
            test_file("track_c.mp3", "/music/jazz/track_c.mp3"),
        ];
        let dir = test_dir("jazz", "/music/jazz", children);
        let result = filter_file_node(&dir, "", TextSearchMode::All);
        assert!(result.is_some());
        let filtered = result.unwrap();
        assert_eq!(filtered.children.len(), 3);
        assert_eq!(filtered.file_count, 3);
    }

    // ── filter_tag_node tests ───────────────────────────────────────────

    /// Helper to build a tag leaf node (with file paths, no children).
    fn tag_leaf(label: &str) -> TagTreeNode {
        TagTreeNode {
            label: label.to_string(),
            children: vec![],
            file_paths: vec![PathBuf::from("/music/song.mp3")],
            is_expanded: false,
            file_count: 1,
        }
    }

    #[test]
    fn test_filter_tag_empty_query_returns_some() {
        let node = tag_leaf("Rock");
        let result = filter_tag_node(&node, "", TextSearchMode::All);
        assert!(result.is_some());
        assert_eq!(result.unwrap().label, "Rock");
    }

    #[test]
    fn test_filter_tag_label_match() {
        let node = tag_leaf("Jazz");
        let result = filter_tag_node(&node, "jazz", TextSearchMode::All);
        assert!(result.is_some());
    }

    #[test]
    fn test_filter_tag_label_no_match() {
        let node = tag_leaf("Jazz");
        let result = filter_tag_node(&node, "Rock", TextSearchMode::All);
        assert!(result.is_none());
    }

    #[test]
    fn test_filter_tag_case_insensitive() {
        let node = tag_leaf("Electronic");
        let result = filter_tag_node(&node, "ELECTRONIC", TextSearchMode::All);
        assert!(result.is_some());
    }

    #[test]
    fn test_filter_tag_non_leaf_matching_keeps_all_children() {
        let child_a = tag_leaf("Artist A");
        let child_b = tag_leaf("Artist B");
        let parent = TagTreeNode {
            label: "Rock".to_string(),
            children: vec![child_a, child_b],
            file_paths: vec![],
            is_expanded: false,
            file_count: 2,
        };
        let result = filter_tag_node(&parent, "Rock", TextSearchMode::All);
        assert!(result.is_some());
        let filtered = result.unwrap();
        assert_eq!(filtered.children.len(), 2);
    }

    #[test]
    fn test_filter_tag_non_leaf_no_match_prunes_children() {
        let child_a = tag_leaf("Pop Artist");
        let child_b = tag_leaf("Rock Artist");
        let parent = TagTreeNode {
            label: "Mixed".to_string(),
            children: vec![child_a, child_b],
            file_paths: vec![],
            is_expanded: false,
            file_count: 2,
        };
        let result = filter_tag_node(&parent, "Rock", TextSearchMode::All);
        assert!(result.is_some());
        let filtered = result.unwrap();
        assert_eq!(filtered.children.len(), 1);
        assert_eq!(filtered.children[0].label, "Rock Artist");
    }

    #[test]
    fn test_filter_tag_deeply_nested_match() {
        let leaf = tag_leaf("target_track");
        let album = TagTreeNode {
            label: "My Album".to_string(),
            children: vec![leaf],
            file_paths: vec![],
            is_expanded: false,
            file_count: 1,
        };
        let artist = TagTreeNode {
            label: "My Artist".to_string(),
            children: vec![album],
            file_paths: vec![],
            is_expanded: false,
            file_count: 1,
        };
        let genre = TagTreeNode {
            label: "Pop".to_string(),
            children: vec![artist],
            file_paths: vec![],
            is_expanded: false,
            file_count: 1,
        };
        let result =
            filter_tag_node(&genre, "target_track", TextSearchMode::All);
        assert!(result.is_some());
        let filtered = result.unwrap();
        // Genre kept, but only with the matching chain
        assert_eq!(filtered.label, "Pop");
        assert_eq!(filtered.children.len(), 1);
        assert_eq!(filtered.children[0].label, "My Artist");
        assert_eq!(filtered.children[0].children.len(), 1);
        assert_eq!(filtered.children[0].children[0].label, "My Album");
        assert_eq!(
            filtered.children[0].children[0].children[0].label,
            "target_track"
        );
    }

    #[test]
    fn test_filter_tag_non_leaf_no_matches_in_subtree() {
        let child = tag_leaf("Some Artist");
        let parent = TagTreeNode {
            label: "Genre".to_string(),
            children: vec![child],
            file_paths: vec![],
            is_expanded: false,
            file_count: 1,
        };
        let result =
            filter_tag_node(&parent, "nonexistent", TextSearchMode::All);
        assert!(result.is_none());
    }

    #[test]
    fn test_filter_tag_partial_query_match() {
        let node = tag_leaf("Progressive Rock");
        let result = filter_tag_node(&node, "rock", TextSearchMode::All);
        assert!(result.is_some());
    }

    // ── filter_tag_node mode-awareness tests ─────────────────────────

    #[test]
    fn test_filter_tag_node_all_mode_matches_label() {
        let node = tag_leaf("Jazz");
        let result = filter_tag_node(&node, "jazz", TextSearchMode::All);
        assert!(result.is_some());
    }

    #[test]
    fn test_filter_tag_node_all_mode_matches_file_path() {
        let node = TagTreeNode {
            label: "some_label".to_string(),
            children: vec![],
            file_paths: vec![PathBuf::from("/music/jazz/track.mp3")],
            is_expanded: false,
            file_count: 1,
        };
        // Label doesn't match, but file path does — All mode keeps it
        let result = filter_tag_node(&node, "jazz", TextSearchMode::All);
        assert!(result.is_some());
    }

    #[test]
    fn test_filter_tag_node_label_matches_in_any_mode() {
        let node = tag_leaf("Rock");
        for mode in &[
            TextSearchMode::All,
            TextSearchMode::DirectoryPath,
            TextSearchMode::TrackFilename,
            TextSearchMode::Creator,
            TextSearchMode::Album,
            TextSearchMode::Title,
            TextSearchMode::Genre,
        ] {
            let result = filter_tag_node(&node, "rock", *mode);
            assert!(
                result.is_some(),
                "Label match should work in mode {mode:?}"
            );
        }
    }

    #[test]
    fn test_filter_tag_node_metadata_mode_excludes_file_path() {
        let node = TagTreeNode {
            label: "Unrelated".to_string(),
            children: vec![],
            file_paths: vec![PathBuf::from("/music/jazz/track.mp3")],
            is_expanded: false,
            file_count: 1,
        };
        // File path contains "jazz" but label does not — Genre mode
        // should NOT keep the node (metadata modes check labels only)
        let result = filter_tag_node(&node, "jazz", TextSearchMode::Genre);
        assert!(result.is_none());
    }

    #[test]
    fn test_filter_tag_node_creator_mode_checks_label_only() {
        let node = TagTreeNode {
            label: "Miles Davis".to_string(),
            children: vec![],
            file_paths: vec![PathBuf::from("/music/track.mp3")],
            is_expanded: false,
            file_count: 1,
        };
        let result = filter_tag_node(&node, "miles", TextSearchMode::Creator);
        assert!(result.is_some());
    }

    #[test]
    fn test_filter_tag_node_album_mode_checks_label_only() {
        let node = TagTreeNode {
            label: "Kind of Blue".to_string(),
            children: vec![],
            file_paths: vec![PathBuf::from("/music/track.mp3")],
            is_expanded: false,
            file_count: 1,
        };
        let result = filter_tag_node(&node, "blue", TextSearchMode::Album);
        assert!(result.is_some());
    }

    #[test]
    fn test_filter_tag_node_title_mode_checks_label_only() {
        let node = TagTreeNode {
            label: "So What".to_string(),
            children: vec![],
            file_paths: vec![PathBuf::from("/music/track.mp3")],
            is_expanded: false,
            file_count: 1,
        };
        let result = filter_tag_node(&node, "what", TextSearchMode::Title);
        assert!(result.is_some());
    }

    #[test]
    fn test_filter_tag_node_path_mode_matches_file_path() {
        let node = TagTreeNode {
            label: "TrackName".to_string(),
            children: vec![],
            file_paths: vec![PathBuf::from("/music/jazz/track.mp3")],
            is_expanded: false,
            file_count: 1,
        };
        // Label doesn't match, but path does — DirectoryPath mode
        let result =
            filter_tag_node(&node, "jazz", TextSearchMode::DirectoryPath);
        assert!(result.is_some());
    }

    #[test]
    fn test_filter_tag_node_filename_mode_matches_filename() {
        let node = TagTreeNode {
            label: "Unrelated".to_string(),
            children: vec![],
            file_paths: vec![PathBuf::from("/music/genre/my_song.mp3")],
            is_expanded: false,
            file_count: 1,
        };
        // Label doesn't match, but filename does — TrackFilename mode
        let result =
            filter_tag_node(&node, "my_song", TextSearchMode::TrackFilename);
        assert!(result.is_some());
    }

    #[test]
    fn test_filter_tag_node_all_mode_checks_both_label_and_path() {
        // Label match
        let node_label = tag_leaf("Jazz");
        assert!(
            filter_tag_node(&node_label, "jazz", TextSearchMode::All).is_some()
        );

        // File path match (label doesn't match)
        let node_path = TagTreeNode {
            label: "Unrelated".to_string(),
            children: vec![],
            file_paths: vec![PathBuf::from("/music/jazz/track.mp3")],
            is_expanded: false,
            file_count: 1,
        };
        assert!(
            filter_tag_node(&node_path, "jazz", TextSearchMode::All).is_some()
        );

        // Neither matches
        let node_neither = tag_leaf("Classical");
        assert!(
            filter_tag_node(&node_neither, "jazz", TextSearchMode::All)
                .is_none()
        );
    }

    // ── filter_tag_node recursive mode-awareness tests ───────────────

    #[test]
    fn test_filter_tag_node_recursive_with_mode_preserves_mode() {
        // Non-leaf node that doesn't match by label but has a child
        // whose file path matches. In All mode, this should work.
        let leaf = TagTreeNode {
            label: "Some Track".to_string(),
            children: vec![],
            file_paths: vec![PathBuf::from("/music/jazz/so_what.mp3")],
            is_expanded: false,
            file_count: 1,
        };
        let parent = TagTreeNode {
            label: "GenreNode".to_string(),
            children: vec![leaf],
            file_paths: vec![],
            is_expanded: false,
            file_count: 1,
        };
        // All mode: label doesn't match, but child's file path does
        let result = filter_tag_node(&parent, "jazz", TextSearchMode::All);
        assert!(result.is_some());
        let filtered = result.unwrap();
        assert_eq!(filtered.children.len(), 1);
    }

    // ── filter_tag_node file_count recalculation tests ──────────────────

    #[test]
    fn test_filter_tag_node_recalculates_file_count_on_child_prune() {
        // Non-leaf with 3 children, search matches only 1 child
        let child_a = tag_leaf("Miles Davis");
        let child_b = tag_leaf("John Coltrane");
        let child_c = tag_leaf("Thelonious Monk");
        let parent = TagTreeNode {
            label: "Jazz".to_string(),
            children: vec![child_a, child_b, child_c],
            file_paths: vec![],
            is_expanded: false,
            file_count: 3,
        };
        let result =
            filter_tag_node(&parent, "miles", TextSearchMode::All);
        assert!(result.is_some());
        let filtered = result.unwrap();
        assert_eq!(filtered.children.len(), 1);
        assert_eq!(filtered.file_count, 1);
    }

    #[test]
    fn test_filter_tag_node_maintains_file_count_on_label_match() {
        // Non-leaf whose label matches, all children kept
        let child_a = tag_leaf("Miles Davis");
        let child_b = tag_leaf("John Coltrane");
        let parent = TagTreeNode {
            label: "Jazz".to_string(),
            children: vec![child_a, child_b],
            file_paths: vec![],
            is_expanded: false,
            file_count: 2,
        };
        let result = filter_tag_node(&parent, "jazz", TextSearchMode::All);
        assert!(result.is_some());
        let filtered = result.unwrap();
        assert_eq!(filtered.children.len(), 2);
        assert_eq!(filtered.file_count, 2);
    }

    #[test]
    fn test_filter_tag_node_nested_file_count_recalculation() {
        // Two-level tree: genre -> artists -> tracks
        // Only 1 track matches, so intermediate node and root should have
        // correct file_count.
        let track_match = tag_leaf("So What");
        let track_other = tag_leaf("Blue Train");
        let artist_miles = TagTreeNode {
            label: "Miles Davis".to_string(),
            children: vec![track_match],
            file_paths: vec![],
            is_expanded: false,
            file_count: 1,
        };
        let artist_coltrane = TagTreeNode {
            label: "John Coltrane".to_string(),
            children: vec![track_other],
            file_paths: vec![],
            is_expanded: false,
            file_count: 1,
        };
        let genre = TagTreeNode {
            label: "Jazz".to_string(),
            children: vec![artist_miles, artist_coltrane],
            file_paths: vec![],
            is_expanded: false,
            file_count: 2,
        };
        let result =
            filter_tag_node(&genre, "So What", TextSearchMode::All);
        assert!(result.is_some());
        let filtered = result.unwrap();
        assert_eq!(filtered.label, "Jazz");
        // Only "Miles Davis" artist kept
        assert_eq!(filtered.children.len(), 1);
        assert_eq!(filtered.children[0].label, "Miles Davis");
        // Artist-level file_count recalculated
        assert_eq!(filtered.children[0].file_count, 1);
        // Root-level file_count recalculated via sum of children
        assert_eq!(filtered.file_count, 1);
    }

    #[test]
    fn test_filter_tag_node_path_mode_recalculates_file_count() {
        // Parent non-matching, child matches via DirectoryPath mode
        let child = TagTreeNode {
            label: "Track".to_string(),
            children: vec![],
            file_paths: vec![PathBuf::from("/music/jazz/so_what.mp3")],
            is_expanded: false,
            file_count: 1,
        };
        let parent = TagTreeNode {
            label: "GenreNode".to_string(),
            children: vec![child],
            file_paths: vec![],
            is_expanded: false,
            file_count: 1,
        };
        // Label doesn't match, but child's path contains "jazz"
        let result =
            filter_tag_node(&parent, "jazz", TextSearchMode::DirectoryPath);
        assert!(result.is_some());
        let filtered = result.unwrap();
        assert_eq!(filtered.children.len(), 1);
        assert_eq!(filtered.file_count, 1);
    }
}
