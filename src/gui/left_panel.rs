use crate::gui::render_node::{render_file_node, render_tag_node};
use crate::gui::view::{MenuStyle, TreeBrowserStyle};
use crate::gui::{FileTreeApp, LeftPanelNavMode, LeftPanelSortMode, Message};
use iced::{
    Element,
    widget::{Space, button, column, row, text},
};

/// Creates the toggle button for the left panel, displaying either a left or
/// right arrow depending on the current expansion state. The button uses the
/// specified menu style for text size and triggers the `ToggleLeftPanel`
/// message when pressed.
fn create_toggle_left_panel_button(
    app: &FileTreeApp,
    menu_style: MenuStyle,
) -> iced::widget::Button<Message> {
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
) -> Element<Message> {
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

/// Builds the left panel's tag-based navigation tree UI.
///
/// Iterates over the root nodes of the tag tree in the application state and
/// recursively renders each node using `render_tag_node`. Applies the specified
/// tree browser style for row sizing and spacing. This function is used when
/// the left panel is in tag navigation mode to display the genre → artist →
/// album → track hierarchy.
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
    let mut trees = column![];
    for node in &app.tag_tree_roots {
        trees = trees.push(render_tag_node(
            node,
            0,
            vec![],
            tree_browser_style.directory_row_size,
            flat_button_style,
        ));
        trees =
            trees.push(Space::with_height(tree_browser_style.tree_row_height));
    }
    trees
}

/// Constructs the left panel UI for the application, including the menu row,
/// file extension filter menu, and either the directory or tag tree browser
/// depending on the current navigation mode. The panel's appearance and
/// behavior are controlled by the provided style parameters and button style
/// function. Returns an `Element<Message>` representing the left panel's
/// content, which adapts to the expansion state and navigation mode.
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
) -> Element<Message> {
    //
    // left_panel_menu_row_1
    // --------------------------------------------------

    let left_panel_menu_row_1 = create_left_panel_menu_row(app, menu_style);

    let nav_mode_label = match app.left_panel_nav_mode {
        LeftPanelNavMode::Directory => "Select by: Directory",
        LeftPanelNavMode::Tag => "Select by: Genre",
        LeftPanelNavMode::Musician => "Select by: Creator",
    };
    let nav_mode_button =
        iced::widget::button::<Message, iced::Theme, iced::Renderer>(
            iced::widget::text(nav_mode_label)
                .size(menu_style.text_size)
                .style(move |_theme| iced::widget::text::Style {
                    color: Some(menu_style.text_color.into()),
                }),
        )
        .on_press(Message::ToggleLeftPanelNavMode);

    //
    // left_panel_menu_row_2
    // --------------------------------------------------

    let extension_menu =
        create_extension_menu(app, menu_style.text_size, menu_style.text_color);
    let left_panel_menu_row_2 =
        iced::widget::row![nav_mode_button, extension_menu]
            .spacing(menu_style.spacing);

    //
    // tree_browser
    // --------------------------------------------------

    let tree_browser = match app.left_panel_nav_mode {
        LeftPanelNavMode::Directory => create_left_panel_file_tree_browser(
            app,
            tree_browser_style,
            flat_button_style,
        ),
        LeftPanelNavMode::Tag | LeftPanelNavMode::Musician => {
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
            tree_browser,
        ]
    } else {
        column![create_toggle_left_panel_button(app, menu_style)]
    };
    left_content.into()
}
