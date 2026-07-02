//! Recursive tree-node rendering for the Playlist UI.
//!
//! Renders `FileNode` (directory/file trees) and `TagTreeNode`
//! (genre/creator/album/track trees) as nested, indented button rows.
//! Provides log-scale colour highlighting based on per-node file counts,
//! and context menus for adding files to the right panel.
//!
//! Public API:
//!     render_file_node   — draw a directory/file tree node (accepts sort mode)
//!     render_tag_node    — draw a genre/creator/album/track tree node (accepts sort mode)
//!     file_count_highlight — map file count to a highlight colour

use crate::fs::file_tree::{FileNode, NodeType};
use crate::gui::{LeftPanelSortMode, Message, TagTreeNode};
use iced::{
    Color, Element, Length,
    widget::{button, column, container, row, text},
};
use iced_aw::widgets::ContextMenu;
use std::fs;

/// Maps a file count to a highlight colour using log-scale interpolation.
/// Returns a deep blue for the maximum count, fading to a faint blue for
/// small counts. If `count == 0`, returns the baseline faint blue.
fn file_count_highlight(count: usize, max_count: usize) -> Color {
    let light = Color::from_rgb(0.20, 0.30, 0.60); // faint blue
    if count == 0 || max_count == 0 {
        return light;
    }
    // Normalise count logarithmically
    let t = ((count as f64).log2() / (max_count as f64).log2()).clamp(0.0, 1.0)
        as f32;
    let dark = Color::from_rgb(0.05, 0.12, 0.35); // deep navy blue
    let t_inv = 1.0 - t;
    Color::new(
        light.r * t_inv + dark.r * t,
        light.g * t_inv + dark.g * t,
        light.b * t_inv + dark.b * t,
        1.0,
    )
}

/// Returns a button style function that applies a background tint based on
/// the file count relative to the maximum count in the tree.
fn directory_button_style(
    count: usize,
    max_count: usize,
) -> impl Fn(
    &iced::Theme,
    iced::widget::button::Status,
) -> iced::widget::button::Style
+ Copy
+ 'static {
    let bg = file_count_highlight(count, max_count);
    move |_theme: &iced::Theme,
          _status: iced::widget::button::Status|
          -> iced::widget::button::Style {
        iced::widget::button::Style {
            background: Some(iced::Background::Color(bg)),
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
            text_color: iced::Color::WHITE,
        }
    }
}

///  Recursively renders a file tree node (directory or file) with indentation
///  based on depth, including context menus for directory and file actions.
pub(crate) fn render_file_node(
    node: &FileNode,
    depth: usize,
    directory_row_size: u16,
    file_row_size: u16,
    sort_mode: LeftPanelSortMode,
    flat_button_style: impl Fn(
        &iced::Theme,
        iced::widget::button::Status,
    ) -> iced::widget::button::Style
    + Copy
    + 'static,
    max_count: usize,
) -> Element<'_, Message> {
    let indent = "  ".repeat(depth);

    let mut content = column![];

    match node.node_type {
        NodeType::Directory => {
            let expand_symbol = if node.is_expanded { "▼" } else { "▶" };
            let dir_path = node.path.clone();

            let label = format!(
                "{}{} 📁 {}  ({})",
                indent, expand_symbol, node.name, node.file_count,
            );
            let dir_label = container(text(label).size(directory_row_size))
                .width(Length::Fill);

            let dir_row = row![dir_label];

            let ds = directory_button_style(node.file_count, max_count);

            let context_menu = ContextMenu::new(
                button(dir_row)
                    .style(ds)
                    .on_press(Message::ToggleExpansion(node.path.clone())),
                move || {
                    column![button("Add all files to right panel").on_press(
                        Message::AddDirectoryToRightPanel(dir_path.clone())
                    )]
                    .into()
                },
            );
            content = content.push(context_menu);

            if node.is_expanded {
                let mut indices: Vec<usize> =
                    (0..node.children.len()).collect();
                match sort_mode {
                    LeftPanelSortMode::Alphanumeric => {
                        indices.sort_by(|&i, &j| {
                            let a = &node.children[i];
                            let b = &node.children[j];
                            match (a.node_type.clone(), b.node_type.clone()) {
                                (NodeType::Directory, NodeType::File) => {
                                    std::cmp::Ordering::Less
                                },
                                (NodeType::File, NodeType::Directory) => {
                                    std::cmp::Ordering::Greater
                                },
                                _ => a
                                    .name
                                    .to_lowercase()
                                    .cmp(&b.name.to_lowercase()),
                            }
                        });
                    },
                    LeftPanelSortMode::FileCount => {
                        indices.sort_by(|&i, &j| {
                            let a = &node.children[i];
                            let b = &node.children[j];
                            match (a.node_type.clone(), b.node_type.clone()) {
                                (NodeType::Directory, NodeType::File) => {
                                    std::cmp::Ordering::Less
                                },
                                (NodeType::File, NodeType::Directory) => {
                                    std::cmp::Ordering::Greater
                                },
                                _ => {
                                    // Directories: sort by file_count descending
                                    // Files: both count=1, falls back to alpha
                                    let count_cmp =
                                        b.file_count.cmp(&a.file_count);
                                    count_cmp.then_with(|| {
                                        a.name
                                            .to_lowercase()
                                            .cmp(&b.name.to_lowercase())
                                    })
                                },
                            }
                        });
                    },
                    LeftPanelSortMode::ModifiedDate => {
                        indices.sort_by(|&i, &j| {
                            let a = &node.children[i];
                            let b = &node.children[j];
                            let a_time = fs::metadata(&a.path)
                                .and_then(|m| m.modified())
                                .ok();
                            let b_time = fs::metadata(&b.path)
                                .and_then(|m| m.modified())
                                .ok();
                            match (a.node_type.clone(), b.node_type.clone()) {
                                (NodeType::Directory, NodeType::File) => {
                                    std::cmp::Ordering::Less
                                },
                                (NodeType::File, NodeType::Directory) => {
                                    std::cmp::Ordering::Greater
                                },
                                _ => b_time.cmp(&a_time), // newest first
                            }
                        });
                    },
                }
                for &i in &indices {
                    let child = &node.children[i];
                    content = content.push(render_file_node(
                        child,
                        depth + 1,
                        directory_row_size,
                        file_row_size,
                        sort_mode,
                        flat_button_style,
                        max_count,
                    ));
                }
            }
        },
        NodeType::File => {
            let file_row = text(format!("{} 📄 {}", indent, node.name))
                .size(file_row_size);

            let file_path = node.path.clone();

            let context_menu =
                ContextMenu::new(
                    button(file_row).style(flat_button_style),
                    move || {
                        column![button("Add to right panel").on_press(
                            Message::AddToRightPanel(file_path.clone())
                        )]
                        .into()
                    },
                );

            content = content.push(context_menu);
        },
    }

    content.into()
}

/// Recursively renders a tag-based navigation/selection tree node for the left
/// panel UI.
///
/// Displays the given `TagTreeNode` with indentation based on `depth`, and
/// provides context menus for both category nodes (genre, artist, album) and
/// track nodes. Non-leaf nodes allow adding all contained tracks to the right
/// panel, while leaf nodes (tracks) allow adding the individual track. Handles
/// expansion/collapse of nodes and passes the navigation/selection path for
/// context menu actions.
pub(crate) fn render_tag_node(
    node: &TagTreeNode,
    depth: usize,
    path: Vec<String>,
    directory_row_size: u16,
    sort_mode: LeftPanelSortMode,
    flat_button_style: impl Fn(
        &iced::Theme,
        iced::widget::button::Status,
    ) -> iced::widget::button::Style
    + Copy
    + 'static,
    max_count: usize,
) -> Element<'_, Message> {
    let indent = "  ".repeat(depth);
    let mut content = column![];
    let mut new_path = path;
    new_path.push(node.label.clone());

    let is_leaf = node.children.is_empty();
    let expand_symbol = if !is_leaf {
        if node.is_expanded { "▼" } else { "▶" }
    } else {
        ""
    };

    let label = if is_leaf {
        format!("{}{} {}", indent, expand_symbol, node.label)
    } else {
        format!(
            "{}{} {}  ({})",
            indent, expand_symbol, node.label, node.file_count,
        )
    };

    let row = if is_leaf {
        // Track node (leaf): right-click to add this track only
        let file_path = node.file_paths.first().cloned();
        let context_menu = iced_aw::widgets::ContextMenu::new(
            button(text(label).size(directory_row_size))
                .style(flat_button_style),
            move || {
                if let Some(path) = file_path.clone() {
                    column![
                        button("Add to right panel")
                            .on_press(Message::AddToRightPanel(path))
                    ]
                    .into()
                } else {
                    column![].into()
                }
            },
        );
        iced::widget::row![context_menu]
    } else {
        // Non-leaf: context menu for "Add all files"
        let ds = directory_button_style(node.file_count, max_count);
        let context_menu = iced_aw::widgets::ContextMenu::new(
            button(text(label).size(directory_row_size))
                .style(ds)
                .on_press(Message::ToggleTagExpansion(new_path.clone())),
            {
                let path = new_path.clone();
                move || {
                    column![button("Add all files to right panel").on_press(
                        Message::AddTagNodeToRightPanel(path.clone())
                    )]
                    .into()
                }
            },
        );
        iced::widget::row![context_menu]
    };

    content = content.push(row);

    if node.is_expanded {
        let mut indices: Vec<usize> = (0..node.children.len()).collect();
        match sort_mode {
            LeftPanelSortMode::Alphanumeric => {
                indices.sort_by(|&i, &j| {
                    node.children[i]
                        .label
                        .to_lowercase()
                        .cmp(&node.children[j].label.to_lowercase())
                });
            },
            LeftPanelSortMode::ModifiedDate => {
                indices.sort_by(|&i, &j| {
                    let a_time = node.children[i]
                        .file_paths
                        .first()
                        .and_then(|p| std::fs::metadata(p).ok())
                        .and_then(|m| m.modified().ok());
                    let b_time = node.children[j]
                        .file_paths
                        .first()
                        .and_then(|p| std::fs::metadata(p).ok())
                        .and_then(|m| m.modified().ok());
                    b_time.cmp(&a_time) // newest first
                });
            },
            LeftPanelSortMode::FileCount => {
                indices.sort_by(|&i, &j| {
                    let count_cmp = node.children[j]
                        .file_count
                        .cmp(&node.children[i].file_count);
                    count_cmp.then_with(|| {
                        node.children[i]
                            .label
                            .to_lowercase()
                            .cmp(&node.children[j].label.to_lowercase())
                    })
                });
            },
        }
        for &i in &indices {
            content = content.push(render_tag_node(
                &node.children[i],
                depth + 1,
                new_path.clone(),
                directory_row_size,
                sort_mode,
                flat_button_style,
                max_count,
            ));
        }
    }
    content.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlight_zero() {
        // count of 0 returns the light baseline
        let c = file_count_highlight(0, 42);
        let light = Color::from_rgb(0.20, 0.30, 0.60);
        assert_eq!(c, light);
    }

    #[test]
    fn highlight_min() {
        // minimum non-zero count
        let c = file_count_highlight(1, 42);
        // ln(1) = 0, so t = 0, and the colour should be the light baseline
        let light = Color::from_rgb(0.20, 0.30, 0.60);
        assert_eq!(c, light, "count=1 with ln(1)=0 should give light baseline");
    }

    #[test]
    fn highlight_max() {
        // max boundary returns darkest colour
        let c = file_count_highlight(42, 42);
        let dark = Color::from_rgb(0.05, 0.12, 0.35);
        assert_eq!(c, dark);
    }

    #[test]
    fn highlight_zero_max() {
        // When max_count is 0, returns baseline
        let c = file_count_highlight(5, 0);
        let light = Color::from_rgb(0.20, 0.30, 0.60);
        assert_eq!(c, light);
    }

    #[test]
    fn test_render_file_node_sorted_by_file_count() {
        // Create a directory with children in non-optimal order
        // and verify that render_file_node with FileCount mode does not panic.
        use crate::fs::file_tree::FileNode;
        use std::path::PathBuf;

        // Children in reverse-count order to expose sorting
        let small_dir = FileNode::new_directory(
            "small_dir".to_string(),
            PathBuf::from("/root/small_dir"),
            vec![
                FileNode::new_file(
                    "a.mp3".to_string(),
                    PathBuf::from("/root/small_dir/a.mp3"),
                ),
                FileNode::new_file(
                    "b.mp3".to_string(),
                    PathBuf::from("/root/small_dir/b.mp3"),
                ),
            ],
        );
        let big_dir = FileNode::new_directory(
            "big_dir".to_string(),
            PathBuf::from("/root/big_dir"),
            vec![
                FileNode::new_file(
                    "c.mp3".to_string(),
                    PathBuf::from("/root/big_dir/c.mp3"),
                ),
                FileNode::new_file(
                    "d.mp3".to_string(),
                    PathBuf::from("/root/big_dir/d.mp3"),
                ),
                FileNode::new_file(
                    "e.mp3".to_string(),
                    PathBuf::from("/root/big_dir/e.mp3"),
                ),
            ],
        );
        let file_z = FileNode::new_file(
            "z_file.txt".to_string(),
            PathBuf::from("/root/z_file.txt"),
        );
        let file_a = FileNode::new_file(
            "a_file.txt".to_string(),
            PathBuf::from("/root/a_file.txt"),
        );

        // Insert in worst-case order: small dir first, big dir last,
        // z before a
        let root = FileNode::new_directory(
            "root".to_string(),
            PathBuf::from("/root"),
            vec![small_dir, file_z, file_a, big_dir],
        );

        let flat_button_style =
            |_theme: &iced::Theme, _status: iced::widget::button::Status| {
                iced::widget::button::Style {
                    background: None,
                    border: iced::Border::default(),
                    shadow: iced::Shadow::default(),
                    text_color: iced::Color::WHITE,
                }
            };

        // This should not panic — FileCount sort orders children correctly
        let _element = render_file_node(
            &root,
            0,
            12,
            12,
            LeftPanelSortMode::FileCount,
            flat_button_style,
            10,
        );
    }

    #[test]
    fn test_render_tag_node_sorted_by_file_count() {
        // Create a tag tree where children are in worst-case order
        // and verify that render_tag_node with FileCount mode does not panic.
        use std::path::PathBuf;

        let big_genre = TagTreeNode {
            label: "big genre".to_string(),
            children: vec![
                TagTreeNode {
                    label: "track1".to_string(),
                    children: vec![],
                    file_paths: vec![PathBuf::from("/big/track1.mp3")],
                    is_expanded: false,
                    file_count: 1,
                },
                TagTreeNode {
                    label: "track2".to_string(),
                    children: vec![],
                    file_paths: vec![PathBuf::from("/big/track2.mp3")],
                    is_expanded: false,
                    file_count: 1,
                },
                TagTreeNode {
                    label: "track3".to_string(),
                    children: vec![],
                    file_paths: vec![PathBuf::from("/big/track3.mp3")],
                    is_expanded: false,
                    file_count: 1,
                },
            ],
            file_paths: vec![
                PathBuf::from("/big/track1.mp3"),
                PathBuf::from("/big/track2.mp3"),
                PathBuf::from("/big/track3.mp3"),
            ],
            is_expanded: true,
            file_count: 3,
        };
        let small_genre = TagTreeNode {
            label: "small genre".to_string(),
            children: vec![TagTreeNode {
                label: "track_a".to_string(),
                children: vec![],
                file_paths: vec![PathBuf::from("/small/track_a.mp3")],
                is_expanded: false,
                file_count: 1,
            }],
            file_paths: vec![PathBuf::from("/small/track_a.mp3")],
            is_expanded: true,
            file_count: 1,
        };
        let medium_genre = TagTreeNode {
            label: "medium genre".to_string(),
            children: vec![
                TagTreeNode {
                    label: "track_x".to_string(),
                    children: vec![],
                    file_paths: vec![PathBuf::from("/medium/track_x.mp3")],
                    is_expanded: false,
                    file_count: 1,
                },
                TagTreeNode {
                    label: "track_y".to_string(),
                    children: vec![],
                    file_paths: vec![PathBuf::from("/medium/track_y.mp3")],
                    is_expanded: false,
                    file_count: 1,
                },
            ],
            file_paths: vec![
                PathBuf::from("/medium/track_x.mp3"),
                PathBuf::from("/medium/track_y.mp3"),
            ],
            is_expanded: true,
            file_count: 2,
        };

        // Root with children in worst-case order: small, medium, big
        let root = TagTreeNode {
            label: "root".to_string(),
            children: vec![small_genre, medium_genre, big_genre],
            file_paths: vec![],
            is_expanded: true,
            file_count: 6,
        };

        let flat_button_style =
            |_theme: &iced::Theme, _status: iced::widget::button::Status| {
                iced::widget::button::Style {
                    background: None,
                    border: iced::Border::default(),
                    shadow: iced::Shadow::default(),
                    text_color: iced::Color::WHITE,
                }
            };

        // This should not panic — FileCount sort orders children correctly
        let _element = render_tag_node(
            &root,
            0,
            vec![],
            12,
            LeftPanelSortMode::FileCount,
            flat_button_style,
            10,
        );

        // Also verify Alphanumeric sort still works
        let _element = render_tag_node(
            &root,
            0,
            vec![],
            12,
            LeftPanelSortMode::Alphanumeric,
            flat_button_style,
            10,
        );
    }

    #[test]
    fn test_render_tag_node_unsorted_children_sorted_alphabetically_now() {
        // Regression test: tag tree children previously rendered in
        // BTreeMap insertion order (unsorted). Now they should be sorted
        // alphabetically in Alphanumeric mode.
        use std::path::PathBuf;

        // Children in reverse alphabetical order
        let node = TagTreeNode {
            label: "root".to_string(),
            children: vec![
                TagTreeNode {
                    label: "z_track".to_string(),
                    children: vec![],
                    file_paths: vec![PathBuf::from("/z.mp3")],
                    is_expanded: false,
                    file_count: 1,
                },
                TagTreeNode {
                    label: "m_track".to_string(),
                    children: vec![],
                    file_paths: vec![PathBuf::from("/m.mp3")],
                    is_expanded: false,
                    file_count: 1,
                },
                TagTreeNode {
                    label: "a_track".to_string(),
                    children: vec![],
                    file_paths: vec![PathBuf::from("/a.mp3")],
                    is_expanded: false,
                    file_count: 1,
                },
            ],
            file_paths: vec![],
            is_expanded: true,
            file_count: 3,
        };

        let flat_button_style =
            |_theme: &iced::Theme, _status: iced::widget::button::Status| {
                iced::widget::button::Style {
                    background: None,
                    border: iced::Border::default(),
                    shadow: iced::Shadow::default(),
                    text_color: iced::Color::WHITE,
                }
            };

        // This should not panic — Alphanumeric sort orders children
        // alphabetically, unlike the previous unsorted behaviour
        let _element = render_tag_node(
            &node,
            0,
            vec![],
            12,
            LeftPanelSortMode::Alphanumeric,
            flat_button_style,
            10,
        );
    }
}
