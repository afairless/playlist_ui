use crate::fs::file_tree::{FileNode, NodeType};
use crate::gui::{LeftPanelSortMode, Message, TagTreeNode};
use iced::{
    Element, Length,
    widget::{button, column, container, row, text},
};
use iced_aw::widgets::ContextMenu;
use std::fs;

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
) -> Element<Message> {
    let indent = "  ".repeat(depth);

    let mut content = column![];

    match node.node_type {
        NodeType::Directory => {
            let expand_symbol = if node.is_expanded { "▼" } else { "▶" };
            let dir_path = node.path.clone();

            let dir_label = container(
                text(format!("{}{} 📁 {}", indent, expand_symbol, node.name))
                    .size(directory_row_size),
            )
            .width(Length::Fill);

            let dir_row = row![dir_label];

            let context_menu = ContextMenu::new(
                button(dir_row)
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

/// Recursively renders a tag-based navigation tree node for the left panel UI.
///
/// Displays the given `TagTreeNode` with indentation based on `depth`, and
/// provides context menus for both category nodes (genre, artist, album) and
/// track nodes. Non-leaf nodes allow adding all contained tracks to the right
/// panel, while leaf nodes (tracks) allow adding the individual track. Handles
/// expansion/collapse of nodes and passes the navigation path for context menu
/// actions.
pub(crate) fn render_tag_node(
    node: &TagTreeNode,
    depth: usize,
    path: Vec<String>,
    directory_row_size: u16,
    flat_button_style: impl Fn(
        &iced::Theme,
        iced::widget::button::Status,
    ) -> iced::widget::button::Style
    + Copy
    + 'static,
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

    let label = format!("{}{} {}", indent, expand_symbol, node.label);

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
        let context_menu = iced_aw::widgets::ContextMenu::new(
            button(text(label).size(directory_row_size))
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
        for child in &node.children {
            content = content.push(render_tag_node(
                child,
                depth + 1,
                new_path.clone(),
                directory_row_size,
                flat_button_style,
            ));
        }
    }
    content.into()
}
