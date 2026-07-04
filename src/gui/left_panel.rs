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

/// Creates the search row UI containing a text input and a mode toggle button.
/// The search row is hidden when the left panel is collapsed.
fn create_search_row(
    app: &FileTreeApp,
    menu_style: MenuStyle,
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

    let mode_button = button(text(mode_label).size(menu_style.text_size))
        .on_press(Message::ToggleSearchMode);

    row![search_input, mode_button].spacing(menu_style.spacing).into()
}

/// Checks whether a metadata field value contains the given query string
/// (case-insensitive). Returns `true` if the value is present and contains
/// the query, or `false` otherwise.
fn field_matches(value: &Option<String>, query: &str) -> bool {
    value
        .as_deref()
        .map(|v| v.to_ascii_lowercase().contains(&query.to_ascii_lowercase()))
        .unwrap_or(false)
}

/// Checks whether a file's metadata matches the given search mode and query.
/// Extracts metadata from the file path and checks the relevant field.
/// Used internally by `filter_file_node` for metadata-based filtering.
fn file_matches_mode(path: &Path, mode: TextSearchMode, query: &str) -> bool {
    let meta = extract_media_metadata(path);
    match mode {
        TextSearchMode::Creator => field_matches(&meta.creator, query),
        TextSearchMode::Album => field_matches(&meta.album, query),
        TextSearchMode::Title => field_matches(&meta.title, query),
        TextSearchMode::Genre => field_matches(&meta.genre, query),
        TextSearchMode::All => {
            field_matches(&meta.creator, query)
                || field_matches(&meta.album, query)
                || field_matches(&meta.title, query)
                || field_matches(&meta.genre, query)
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
        Some(cloned)
    } else {
        None
    }
}

/// Recursively filters a `TagTreeNode` tree, keeping only nodes whose label
/// matches the search query. When a non-leaf node matches, all its children
/// are kept. When a node does not match, only children that match are kept
/// (recursive prune). Returns `None` when no match is found.
pub(crate) fn filter_tag_node(
    node: &TagTreeNode,
    query: &str,
) -> Option<TagTreeNode> {
    if query.is_empty() {
        return Some(node.clone());
    }

    let label_matches =
        node.label.to_ascii_lowercase().contains(&query.to_ascii_lowercase());

    if node.children.is_empty() {
        // Leaf node (file_paths but no sub-children)
        return if label_matches { Some(node.clone()) } else { None };
    }

    // Non-leaf node
    if label_matches {
        // Keep node with all children
        Some(node.clone())
    } else {
        // Prune children, keep only matching subtrees
        let filtered_children: Vec<TagTreeNode> = node
            .children
            .iter()
            .filter_map(|child| filter_tag_node(child, query))
            .collect();
        if filtered_children.is_empty() {
            None
        } else {
            let mut cloned = node.clone();
            cloned.children = filtered_children;
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
            create_search_row(app, menu_style),
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
        let _element = create_search_row(&app, menu_style);
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
        let _element = create_search_row(&app, menu_style);
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
            let _element = create_search_row(&app, menu_style);
        }
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
        let result = filter_tag_node(&node, "");
        assert!(result.is_some());
        assert_eq!(result.unwrap().label, "Rock");
    }

    #[test]
    fn test_filter_tag_label_match() {
        let node = tag_leaf("Jazz");
        let result = filter_tag_node(&node, "jazz");
        assert!(result.is_some());
    }

    #[test]
    fn test_filter_tag_label_no_match() {
        let node = tag_leaf("Jazz");
        let result = filter_tag_node(&node, "Rock");
        assert!(result.is_none());
    }

    #[test]
    fn test_filter_tag_case_insensitive() {
        let node = tag_leaf("Electronic");
        let result = filter_tag_node(&node, "ELECTRONIC");
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
        let result = filter_tag_node(&parent, "Rock");
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
        let result = filter_tag_node(&parent, "Rock");
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
        let result = filter_tag_node(&genre, "target_track");
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
        let result = filter_tag_node(&parent, "nonexistent");
        assert!(result.is_none());
    }

    #[test]
    fn test_filter_tag_partial_query_match() {
        let node = tag_leaf("Progressive Rock");
        let result = filter_tag_node(&node, "rock");
        assert!(result.is_some());
    }
}
