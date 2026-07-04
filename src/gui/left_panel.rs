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

use crate::gui::render_node::{render_file_node, render_tag_node};
use crate::gui::view::{MenuStyle, TreeBrowserStyle};
use crate::gui::{
    FileTreeApp, LeftPanelSelectMode, LeftPanelSortMode, Message, TagTreeNode,
    TextSearchMode,
};
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

    // Compute max file_count across all directory nodes in the file tree
    let max_count = app
        .root_nodes
        .iter()
        .flatten()
        .map(|n| n.file_count)
        .max()
        .unwrap_or(0);

    let mut trees = column![];
    for (i, node_opt) in app.root_nodes.iter().enumerate() {
        let dir_path = app.top_dirs.get(i).cloned().unwrap_or_default();

        // Remove button (narrow column)
        let remove_button =
            button(text("X").size(tree_browser_style.directory_row_size))
                .width(tree_browser_style.remove_button_width - gap_width)
                .on_press(Message::RemoveTopDir(dir_path.clone()));

        let content = if let Some(node) = node_opt {
            // Directory tree
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

        // Row: [X][directory tree]
        let row = row![content, Space::with_width(gap_width), remove_button,]
            .align_y(iced::Alignment::Start);

        trees = trees.push(row);
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
    // Compute max file_count across all tag tree root nodes
    let max_count =
        app.tag_tree_roots.iter().map(|n| n.file_count).max().unwrap_or(0);

    // Sort root indices according to the current sort mode, then render in
    // that order. We use index-based sorting to avoid borrowing a local copy
    // when the return lifetime is tied to &app.
    let mut indices: Vec<usize> = (0..app.tag_tree_roots.len()).collect();
    sort_tag_tree_roots(
        &mut indices,
        &app.tag_tree_roots,
        app.left_panel_sort_mode,
    );

    let mut trees = column![];
    for &i in &indices {
        trees = trees.push(render_tag_node(
            &app.tag_tree_roots[i],
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

    let search_input =
        text_input::<Message, iced::Theme, iced::Renderer>(
            "Search...",
            &app.search_query,
        )
        .on_input(Message::SearchQueryChanged);

    let mode_button =
        button(text(mode_label).size(menu_style.text_size))
            .on_press(Message::ToggleSearchMode);

    row![search_input, mode_button].spacing(menu_style.spacing).into()
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
    use super::sort_tag_tree_roots;
    use super::create_search_row;
    use crate::gui::{FileTreeApp, LeftPanelSortMode, TextSearchMode};
    use crate::gui::state::TagTreeNode;
    use crate::gui::view::MenuStyle;
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
}
