use iced::{Element, widget::{button, column, container, row, Scrollable, scrollable, text, Space}, Length};
use iced_aw::widgets::ContextMenu;
use crate::fs::file_tree::{FileNode, NodeType};
use crate::gui::{FileTreeApp, Message, SortColumn, SortOrder, RightPanelFile};


struct AudioColumnToggles {
    show_musician: bool,
    show_album: bool,
    show_title: bool,
    show_genre: bool,
}


#[derive(Debug, Clone, Copy)]
struct MenuStyle {
    text_size: u16,
    spacing: u16,
    text_color: [f32; 4],
}


fn create_extension_menu(app: &FileTreeApp, menu_size: u16, menu_text_color: [f32; 4]) -> Element<Message> {
    // Creates the file extension filter menu for the left panel, including a styled header button
    //     that toggles the menu and a list of extension toggle buttons. The menu appearance is controlled
    //     by the given text size and color.

    let header = button(
        text(if app.extensions_menu_expanded { "▼ File Extensions" } else { "▶ File Extensions" })
            .size(menu_size)
            .style(move |_theme| iced::widget::text::Style { color: Some(menu_text_color.into()) })
    )
    .on_press(Message::ToggleExtensionsMenu);

    if app.extensions_menu_expanded {
        let mut menu = column![];
        for ext in &app.all_extensions {
            let checked = app.selected_extensions.contains(ext);
            let label = if checked { format!("[x] .{ext}") } else { format!("[ ] .{ext}") };
            menu = menu.push(
                button(text(label))
                    .on_press(Message::ToggleExtension(ext.clone()))
            );
        }
        column![header, menu].into()
    } else {
        column![header].into()
    }
}


fn create_left_panel_menu_row(app: &FileTreeApp, menu_style: MenuStyle) -> Element<Message> {
    // Constructs the left panel's menu row containing the "Add Directory" button and the file extension menu,
    //     applying the specified text size, spacing, and color styling to both buttons.

    let directory_button = iced::widget::button::<Message, iced::Theme, iced::Renderer>(
        iced::widget::text("Add Directory")
            .size(menu_style.text_size)
            .style(move |_theme| iced::widget::text::Style { color: Some(menu_style.text_color.into()) })
    )
    .on_press(Message::AddDirectory);

    let extension_menu = create_extension_menu(app, menu_style.text_size, menu_style.text_color);

    iced::widget::row![directory_button, extension_menu].spacing(menu_style.spacing).into()
}


fn render_node(
    node: &FileNode, depth: usize, directory_row_size: u16, file_row_size: u16, 
    flat_button_style: impl Fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style + Copy + 'static,
    ) -> Element<Message> {
    //  Recursively renders a file tree node (directory or file) with indentation based on depth,  
    //      including context menus for directory and file actions.

    let indent = "  ".repeat(depth);

    let mut content = column![];

    match node.node_type {
        NodeType::Directory => {
            let expand_symbol = if node.is_expanded { "▼" } else { "▶" };
            let dir_path = node.path.clone();

            let dir_label = container(
                text(format!("{}{} 📁 {}", indent, expand_symbol, node.name)).size(directory_row_size)
            ).width(Length::Fill);

            let dir_row = row![dir_label];


            let context_menu = ContextMenu::new(
                button(dir_row)
                    .on_press(Message::ToggleExpansion(node.path.clone())),
                move || {
                    column![
                        button("Add all files to right panel")
                            .on_press(Message::AddDirectoryToRightPanel(dir_path.clone()))
                    ]
                    .into()
                },
            );
            content = content.push(context_menu);

            if node.is_expanded {
                let mut indices: Vec<usize> = (0..node.children.len()).collect();
                indices.sort_by(|&i, &j| {
                    let a = &node.children[i];
                    let b = &node.children[j];
                    match (a.node_type.clone(), b.node_type.clone()) {
                        (NodeType::Directory, NodeType::File) => std::cmp::Ordering::Less,
                        (NodeType::File, NodeType::Directory) => std::cmp::Ordering::Greater,
                        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                    }
                });
                for &i in &indices {
                    let child = &node.children[i];
                    content = content.push(render_node(child, depth + 1, directory_row_size, file_row_size, flat_button_style));
                }
            }
        }
        NodeType::File => {
            let file_row = text(format!("{} 📄 {}", indent, node.name)).size(file_row_size);

            let file_path = node.path.clone();

            let context_menu = ContextMenu::new(
                button(file_row).style(flat_button_style),
                move || {
                    column![
                        button("Add to right panel")
                            .on_press(Message::AddToRightPanel(file_path.clone()))
                    ]
                    .into()
                },
            );

            content = content.push(context_menu);
        }
    }

    content.into()
}


fn create_left_panel_file_trees(app: &FileTreeApp, tree_row_height: u16, remove_button_width: u16, directory_row_size: u16, file_row_size: u16) -> iced::widget::Column<'_, Message> {
    // Builds the column of directory trees for the left panel, including directory headers and file trees,
    //     with configurable spacing between rows and directory name text size.

    let flat_button_style = |_theme: &iced::Theme, _status: iced::widget::button::Status| iced::widget::button::Style {
        background: None,
        border: iced::Border::default(),
        shadow: iced::Shadow::default(),
        text_color: iced::Color::WHITE,
    };

    let gap_width = remove_button_width / 4;

    let mut trees = column![];
    for (i, node_opt) in app.root_nodes.iter().enumerate() {

        let dir_path = app.top_dirs.get(i).cloned().unwrap_or_default();

        // Remove button (narrow column)
        let remove_button = button(
                text("X").size(directory_row_size)
            )
            .width(remove_button_width - gap_width)
            .on_press(Message::RemoveTopDir(dir_path.clone()));

        let content = if let Some(node) = node_opt {
            // Directory tree
            render_node(node, 0, directory_row_size, file_row_size, flat_button_style)
        } else {
            text("No files found").into()
        };

        // Row: [X][directory tree]
        let row = row![
            content,
            Space::with_width(gap_width),
            remove_button,
        ]
        .align_y(iced::Alignment::Start);

        trees = trees.push(row);
        trees = trees.push(Space::with_height(tree_row_height));
    }
    trees
}


fn create_right_panel_menu_row(menu_style: MenuStyle) -> Element<'static, Message> {
    //  Creates the right panel's menu row with "Shuffle", "Export to XSPF", and "Play in VLC" buttons,
    //      applying the specified text size, spacing, and color styling to each button.

    let shuffle_btn = iced::widget::button(
        iced::widget::text("Shuffle")
            .width(Length::Shrink)
            .size(menu_style.text_size)
            .style(move |_theme| iced::widget::text::Style { color: Some(menu_style.text_color.into()) })
    )
    .on_press(Message::ShuffleRightPanel)
    .width(Length::Shrink);

    let export_btn = iced::widget::button(
        iced::widget::text("Export to XSPF")
            .width(Length::Shrink)
            .size(menu_style.text_size)
            .style(move |_theme| iced::widget::text::Style { color: Some(menu_style.text_color.into()) })
    )
    .on_press(Message::ExportRightPanelAsXspf)
    .width(Length::Shrink);

    let play_btn = iced::widget::button(
        iced::widget::text("Play")
            .width(Length::Shrink)
            .size(menu_style.text_size)
            .style(move |_theme| iced::widget::text::Style { color: Some(menu_style.text_color.into()) })
    )
    .on_press(Message::ExportAndPlayRightPanelAsXspf)
    .width(Length::Shrink);

    iced::widget::Row::new()
        .push(shuffle_btn)
        .push(export_btn)
        .push(play_btn)
        .spacing(menu_style.spacing).into()
}


fn right_panel_header_row(app: &FileTreeApp, audio_column_toggles: AudioColumnToggles, column_spacing: u16, header_text_size: u16, header_text_color: [f32; 4]) -> iced::widget::Row<'static, Message> {
    //  Builds the header row for the right panel table, including sortable column buttons for  
    //      directory, file, and optionally musician, album, title, and genre. Column spacing and  
    //      text size are configurable via parameters.

    // Sorting arrows
    let dir_arrow = if app.right_panel_sort_column == SortColumn::Directory {
        match app.right_panel_sort_order {
            SortOrder::Desc => " ↑",
            SortOrder::Asc => " ↓",
        }
    } else { "" };
    let file_arrow = if app.right_panel_sort_column == SortColumn::File {
        match app.right_panel_sort_order {
            SortOrder::Desc => " ↑",
            SortOrder::Asc => " ↓",
        }
    } else { "" };
    let musician_arrow = if audio_column_toggles.show_musician && app.right_panel_sort_column == SortColumn::Musician {
        match app.right_panel_sort_order {
            SortOrder::Desc => " ↑",
            SortOrder::Asc => " ↓",
        }
    } else { "" };
    let album_arrow = if audio_column_toggles.show_album && app.right_panel_sort_column == SortColumn::Album {
        match app.right_panel_sort_order {
            SortOrder::Desc => " ↑",
            SortOrder::Asc => " ↓",
        }
    } else { "" };
    let title_arrow = if audio_column_toggles.show_title && app.right_panel_sort_column == SortColumn::Title {
        match app.right_panel_sort_order {
            SortOrder::Desc => " ↑",
            SortOrder::Asc => " ↓",
        }
    } else { "" };
    let genre_arrow = if audio_column_toggles.show_genre && app.right_panel_sort_column == SortColumn::Genre {
        match app.right_panel_sort_order {
            SortOrder::Desc => " ↑",
            SortOrder::Asc => " ↓",
        }
    } else { "" };

    let mut header_row = iced::widget::Row::new()
        .push(
            iced::widget::button(
                iced::widget::text(format!("Directory{dir_arrow}"))
                    .width(Length::FillPortion(1))
                    .size(header_text_size)
                    .style(move |_theme| iced::widget::text::Style { color: Some(header_text_color.into()) })
            )
            .on_press(Message::SortRightPanelByDirectory)
            .width(Length::FillPortion(1))
        )
        .push(
            iced::widget::button(
                iced::widget::text(format!("File{file_arrow}"))
                    .width(Length::FillPortion(1))
                    .size(header_text_size)
                    .style(move |_theme| iced::widget::text::Style { color: Some(header_text_color.into()) })
            )
            .on_press(Message::SortRightPanelByFile)
            .width(Length::FillPortion(1))
        );

    if audio_column_toggles.show_musician {
        header_row = header_row.push(
            iced::widget::button(
                iced::widget::text(format!("Musician{musician_arrow}"))
                    .width(Length::FillPortion(1))
                    .size(header_text_size)
                    .style(move |_theme| iced::widget::text::Style { color: Some(header_text_color.into()) })
            )
            .on_press(Message::SortRightPanelByMusician)
            .width(Length::FillPortion(1))
        );
    }
    if audio_column_toggles.show_album {
        header_row = header_row.push(
            iced::widget::button(
                iced::widget::text(format!("Album{album_arrow}"))
                    .width(Length::FillPortion(1))
                    .size(header_text_size)
                    .style(move |_theme| iced::widget::text::Style { color: Some(header_text_color.into()) })
            )
            .on_press(Message::SortRightPanelByAlbum)
            .width(Length::FillPortion(1))
        );
    }
    if audio_column_toggles.show_title {
        header_row = header_row.push(
            iced::widget::button(
                iced::widget::text(format!("Title{title_arrow}"))
                    .width(Length::FillPortion(1))
                    .size(header_text_size)
                    .style(move |_theme| iced::widget::text::Style { color: Some(header_text_color.into()) })
            )
            .on_press(Message::SortRightPanelByTitle)
            .width(Length::FillPortion(1))
        );
    }
    if audio_column_toggles.show_genre {
        header_row = header_row.push(
            iced::widget::button(
                iced::widget::text(format!("Genre{genre_arrow}"))
                    .width(Length::FillPortion(1))
                    .size(header_text_size)
                    .style(move |_theme| iced::widget::text::Style { color: Some(header_text_color.into()) })
            )
            .on_press(Message::SortRightPanelByGenre)
            .width(Length::FillPortion(1))
        );
    }

    header_row = header_row.spacing(column_spacing);
    header_row
}


fn create_right_panel_dir_widget(file: &RightPanelFile, row_text_size: u16) -> Element<'static, Message> {
    //  Creates the directory cell widget for a right panel row, displaying the parent directory  
    //      name with the specified text size and providing a context menu for directory actions.

    let dirname = file.path.parent()
        .and_then(|p| p.file_name())
        .map(|d| d.to_string_lossy().to_string())
        .unwrap_or_default();
    let dir_path = file.path.parent().map(|p| p.to_path_buf());
    let dir_widget = if let Some(path) = dir_path {
        let dir_path = path.clone();
        iced_aw::widgets::ContextMenu::new(
            iced::widget::text(dirname.clone()).width(Length::FillPortion(1)).size(row_text_size),
            Box::new(move || {
                iced::widget::column![
                    iced::widget::button("Delete All in Directory")
                        .on_press(Message::RemoveDirectoryFromRightPanel(dir_path.clone()))
                ].into()
            }) as Box<dyn Fn() -> iced::Element<'static, Message>>
        )
    } else {
        iced_aw::widgets::ContextMenu::new(
            iced::widget::text(dirname.clone()).width(Length::FillPortion(1)).size(row_text_size),
            Box::new(|| iced::widget::column![].into()) as Box<dyn Fn() -> iced::Element<'static, Message>>
        )
    };
    dir_widget.into()
}


fn create_right_panel_file_context_menu(file: &RightPanelFile, row_text_size: u16) -> Element<'static, Message> {
    //  Creates the file cell widget for a right panel row, displaying the file name with the  
    //      specified text size and providing a context menu for file-specific actions.

    let filename = file.path.file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();
    let file_context_menu = iced_aw::widgets::ContextMenu::new(
        iced::widget::text(filename.clone()).width(Length::FillPortion(1)).size(row_text_size),
        {
            let file_path = file.path.clone();
            Box::new(move || {
                iced::widget::column![
                    iced::widget::button("Delete")
                        .on_press(Message::RemoveFromRightPanel(file_path.clone()))
                ].into()
            }) as Box<dyn Fn() -> iced::Element<'static, Message>>
        }
    );
    file_context_menu.into()
}


fn create_right_panel(app: &FileTreeApp, menu_style: MenuStyle, column_row_spacing: u16, column_height_spacing: u16, row_text_size: u16, header_text_color: [f32; 4]) -> Element<Message> {
    //  Assembles the entire right panel, including the menu row, header row, and all file rows,  
    //      applying the specified menu size, spacing, and text color to controls and table content.

    let displayed_files = app.sorted_right_panel_files();

    // Determine which columns to show
    let show_musician = displayed_files.iter().any(|f| f.musician.as_ref().map(|s| !s.is_empty()).unwrap_or(false));
    let show_album    = displayed_files.iter().any(|f| f.album.as_ref().map(|s| !s.is_empty()).unwrap_or(false));
    let show_title    = displayed_files.iter().any(|f| f.title.as_ref().map(|s| !s.is_empty()).unwrap_or(false));
    let show_genre    = displayed_files.iter().any(|f| f.genre.as_ref().map(|s| !s.is_empty()).unwrap_or(false));

    let audio_column_toggles = AudioColumnToggles {
        show_musician,
        show_album,
        show_title,
        show_genre,
    };

    let header_text_size = row_text_size + 4;
    let menu_row = create_right_panel_menu_row(menu_style);

    let header_row = right_panel_header_row(app, audio_column_toggles, column_row_spacing, header_text_size, header_text_color);

    let mut rows = Vec::new();
    for (i, file_ref) in displayed_files.iter().enumerate() {
        let file = file_ref.clone();

        let dir_widget = create_right_panel_dir_widget(&file, row_text_size);
        let file_context_menu = create_right_panel_file_context_menu(&file, row_text_size);

        let mut row = iced::widget::Row::new()
            .push(dir_widget)
            .push(file_context_menu);

        if show_musician {
            row = row.push(iced::widget::text(file.musician.clone().unwrap_or_default()).width(Length::FillPortion(1)).size(row_text_size));
        }
        if show_album {
            row = row.push(iced::widget::text(file.album.clone().unwrap_or_default()).width(Length::FillPortion(1)).size(row_text_size));
        }
        if show_title {
            row = row.push(iced::widget::text(file.title.clone().unwrap_or_default()).width(Length::FillPortion(1)).size(row_text_size));
        }
        if show_genre {
            row = row.push(iced::widget::text(file.genre.clone().unwrap_or_default()).width(Length::FillPortion(1)).size(row_text_size));
        }
        row = row.spacing(column_row_spacing);

        // Shade alternating pairs of rows
        let pair = (i / 2) % 2;
        let bg_color = if pair == 0 {
            iced::Color::from_rgb(0.13, 0.13, 0.13) // darker
        } else {
            iced::Color::from_rgb(0.18, 0.18, 0.18) // lighter
        };

        let clickable_row = iced::widget::button(row)
            .on_press(Message::OpenRightPanelFile(file.path.clone()))
            .style(move |_theme, _style| iced::widget::button::Style {
                background: Some(iced::Background::Color(bg_color)),
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                text_color: iced::Color::WHITE,
            });

        rows.push(clickable_row.into());
    }

    let col = iced::widget::Column::new()
        .push(Space::with_height(column_height_spacing))
        .push(menu_row)
        .push(Space::with_height(column_height_spacing))
        .push(header_row)
        .push(Scrollable::new(iced::widget::column(rows)));

    col.into()
}


pub fn view(app: &FileTreeApp) -> Element<Message> {
    //  Composes the entire application UI, including the left and right panels, menus, and file trees,  
    //      using the current application state to determine layout and content.

    let menu_style = MenuStyle {
        text_size: 20,
        spacing: 10,
        text_color: [0.0, 1.0, 1.0, 1.0],
    };

    // toggle appearance of left panel
    let toggle_left_panel_btn = button(
        text(if app.left_panel_expanded { "←" } else { "→" }).size(20)
    )
    .on_press(Message::ToggleLeftPanel);

    let left_panel_menu_row = create_left_panel_menu_row(app, menu_style);

    let tree_row_height = 10;
    let remove_button_width = 40;
    let directory_row_size = 16;
    let file_row_size = 14;
    let trees = create_left_panel_file_trees(app, tree_row_height, remove_button_width, directory_row_size, file_row_size);

    let left_content = if app.left_panel_expanded {
        column![
            toggle_left_panel_btn,
            Space::with_height(10),
            left_panel_menu_row,
            Space::with_height(10),
            trees
        ]
    } else {
        column![toggle_left_panel_btn]
    };

    let left_panel: Element<Message> = container::<Message, iced::Theme, iced::Renderer>(
            scrollable(left_content)
        )
        .width(Length::FillPortion(1))
        .padding(10)
        .into();

    let column_row_spacing = 14;
    let column_height_spacing = 10;
    let row_text_size = 14;
    let header_text_color = [1.0, 1.0, 0.0, 1.0];
    let right_panel_width = if app.left_panel_expanded {3} else {20};
    let right_panel: Element<Message> = container::<Message, iced::Theme, iced::Renderer>(
            create_right_panel(app, menu_style, column_row_spacing, column_height_spacing, row_text_size, header_text_color)
        )
        .width(Length::FillPortion(right_panel_width))
        .into();

    let split_row = row![
        left_panel,
        right_panel
    ]
    .width(Length::Fill)
    .height(Length::Fill);

    container::<Message, iced::Theme, iced::Renderer>(split_row)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgb(0.15, 0.15, 0.15))),
            text_color: None,
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
        })
        .into()
}

#[cfg(test)]
mod iced_tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::{tempdir, NamedTempFile};
    use std::fs::File;
    use crate::{update, view, FileTreeApp};
    use crate::gui::RightPanelFile;

    // Helper function to create a test file tree
    fn create_test_tree() -> FileNode {
        let mut root = FileNode::new_directory(
            "root".to_string(),
            PathBuf::from("/test/root"),
            vec![]
        );
        
        let mut subdir = FileNode::new_directory(
            "subdir".to_string(),
            PathBuf::from("/test/root/subdir"),
            vec![]
        );
        
        subdir.children.push(FileNode::new_file(
            "file1.txt".to_string(),
            PathBuf::from("/test/root/subdir/file1.txt")
        ));
        
        root.children.push(subdir);
        root.children.push(FileNode::new_file(
            "file2.md".to_string(),
            PathBuf::from("/test/root/file2.md")
        ));
        
        root
    }

    mod state_tests {
        // State-related tests here
        use super::*;
        use crate::gui::update::restore_expansion_state;

        #[test]
        fn test_file_tree_app_new() {

            let root_node = create_test_tree();
            let dir = root_node.path.clone();
            let file_extensions: Vec<String> = vec!["txt", "md"].into_iter().map(|s| s.to_string()).collect();

            let temp_file = NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();

            let mut app = FileTreeApp::new(vec![dir], file_extensions, Vec::new(), persist_path);
            app.root_nodes[0] = Some(root_node); // manually set the test tree

            assert!(app.root_nodes[0].is_some());
            assert_eq!(app.root_nodes[0].as_ref().unwrap().name, "root");
        }

        #[test]
        fn test_file_tree_app_new_empty() {
            let dir = PathBuf::from("/dummy");
            let file_extensions = vec![
                "txt", "md"
            ].into_iter().map(|s| s.to_string()).collect();
            let temp_file = NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let app = FileTreeApp::new(vec![dir], file_extensions, Vec::new(), persist_path);
            assert!(app.root_nodes[0].is_none());
        }

        #[test]
        fn test_file_tree_app_top_dirs_persistence() {
            use tempfile::tempdir;
            let temp_dir = tempdir().unwrap();
            let dir = temp_dir.path().to_path_buf();
            let file_extensions = vec!["txt".to_string(), "md".to_string()];
            let persist_path = tempfile::NamedTempFile::new().unwrap().path().to_path_buf();

            let app = FileTreeApp::new(vec![dir.clone()], file_extensions.clone(), Vec::new(), persist_path.clone());
            app.persist_top_dirs();

            let app2 = FileTreeApp::load(file_extensions.clone(), Some(Vec::new()), Some(persist_path));
            assert!(app2.top_dirs.contains(&dir));
        }

        #[test]
        fn test_file_tree_app_load_corrupted_top_dirs() {
            use std::fs::OpenOptions;
            use std::io::Write;
            use tempfile::NamedTempFile;
            let file_extensions = vec!["txt".to_string()];
            let temp_file = NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();

            // Write corrupted data to the persistence file
            let mut file = OpenOptions::new().write(true).open(&persist_path).unwrap();
            writeln!(file, "corrupted data").unwrap();

            // Attempt to load top_dirs (should fallback to empty)
            let app = FileTreeApp::new(vec![], file_extensions.clone(), Vec::new(), persist_path.clone());
            app.persist_top_dirs(); // Ensure file exists for load

            let loaded_app = FileTreeApp::load(file_extensions.clone(), Some(Vec::new()), Some(persist_path));
            assert!(loaded_app.top_dirs.is_empty(), "Should handle corrupted top_dirs state gracefully");
        }

        #[test]
        fn test_toggle_extension_with_empty_selected_extensions() {
            let dir = PathBuf::from("/dummy");
            let file_extensions = vec!["txt".to_string()];
            let temp_file = NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![dir], file_extensions.clone(), Vec::new(), persist_path);

            // Remove all selected extensions
            app.selected_extensions.clear();
            // Toggle "txt" on
            let msg = Message::ToggleExtension("txt".to_string());
            let _ = update(&mut app, msg);
            assert!(app.selected_extensions.contains(&"txt".to_string()));
        }

        #[test]
        fn test_toggle_extension_not_in_file_extensions() {
            let dir = PathBuf::from("/dummy");
            let file_extensions = vec!["txt".to_string()];
            let temp_file = NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![dir], file_extensions.clone(), Vec::new(), persist_path);

            // Try toggling an extension not in file_extensions
            let msg = Message::ToggleExtension("md".to_string());
            let _ = update(&mut app, msg);
            assert!(!app.selected_extensions.contains(&"md".to_string()));
            assert!(!app.all_extensions.contains(&"md".to_string()));
        }

        #[test]
        fn test_initial_top_leve_directory_expansion_behavior_one_directory() {

            let dir1 = PathBuf::from("/dir1");
            let file_extensions = vec!["txt".to_string()];
            let temp_file = NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![dir1.clone()], file_extensions.clone(), Vec::new(), persist_path.clone());
            // Simulate a scanned directory node
            let node1 = FileNode::new_directory("dir1".to_string(), dir1.clone(), vec![]);
            app.root_nodes[0] = Some(node1);
            app.expanded_dirs.clear();
            app.expanded_dirs.insert(dir1.clone());
            restore_expansion_state(app.root_nodes[0].as_mut().unwrap(), &app.expanded_dirs);
            assert!(app.root_nodes[0].as_ref().unwrap().is_expanded, "Single top-level directory should be expanded");
        }

        #[test]
        fn test_initial_top_leve_directory_expansion_behavior_two_directories() {

            let file_extensions = vec!["txt".to_string()];
            let temp_file = NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();

            // Two top-level directories
            let dir1 = PathBuf::from("/dir1");
            let dir2 = PathBuf::from("/dir2");
            let mut app2 = FileTreeApp::new(vec![dir1.clone(), dir2.clone()], file_extensions.clone(), Vec::new(), persist_path.clone());
            let node1 = FileNode::new_directory("dir1".to_string(), dir1.clone(), vec![]);
            let node2 = FileNode::new_directory("dir2".to_string(), dir2.clone(), vec![]);
            app2.root_nodes[0] = Some(node1);
            app2.root_nodes[1] = Some(node2);
            app2.expanded_dirs.clear();
            restore_expansion_state(app2.root_nodes[0].as_mut().unwrap(), &app2.expanded_dirs);
            restore_expansion_state(app2.root_nodes[1].as_mut().unwrap(), &app2.expanded_dirs);
            assert!(!app2.root_nodes[0].as_ref().unwrap().is_expanded, "Multiple top-level directories should be collapsed");
            assert!(!app2.root_nodes[1].as_ref().unwrap().is_expanded, "Multiple top-level directories should be collapsed");
        }
    }

    mod update_tests {
        // Update-related tests here
        use super::*;

        #[test]
        fn test_update_toggle_expansion() {
            let root_node = create_test_tree();
            let dir = root_node.path.clone(); // Use the root node's path
            let file_extensions: Vec<String> = vec!["txt", "md"].into_iter().map(|s| s.to_string()).collect();

            // Initially not expanded
            assert!(!root_node.children[0].is_expanded);

            let temp_file = NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![dir], file_extensions, Vec::new(), persist_path);
            app.root_nodes[0] = Some(root_node); // manually set the test tree

            let subdir_path = PathBuf::from("/test/root/subdir");
            let message = Message::ToggleExpansion(subdir_path.clone());

            let _task = update(&mut app, message);

            // Should be expanded now
            assert!(app.root_nodes[0].as_ref().unwrap().children[0].is_expanded);
        }

        #[test]
        fn test_update_toggle_expansion_twice() {
            let root_node = create_test_tree();
            let dir = root_node.path.clone(); // Use the root node's path
            let file_extensions: Vec<String> = vec!["txt", "md"].into_iter().map(|s| s.to_string()).collect();

            let temp_file = NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![dir], file_extensions, Vec::new(), persist_path);
            app.root_nodes[0] = Some(root_node); // manually set the test tree

            let subdir_path = PathBuf::from("/test/root/subdir");

            // Toggle once - should expand
            let message = Message::ToggleExpansion(subdir_path.clone());
            let _ = update(&mut app, message);
            assert!(app.root_nodes[0].as_ref().unwrap().children[0].is_expanded);

            // Toggle again - should collapse
            let message = Message::ToggleExpansion(subdir_path);
            let _ = update(&mut app, message);
            assert!(!app.root_nodes[0].as_ref().unwrap().children[0].is_expanded);
        }

        #[test]
        fn test_update_with_no_root_node() {
            let dir = PathBuf::from("/dummy");
            let file_extensions = vec!["txt", "md"].into_iter().map(|s| s.to_string()).collect();
            let temp_file = NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![dir], file_extensions, Vec::new(), persist_path);
            let message = Message::ToggleExpansion(PathBuf::from("/nonexistent"));
            
            let _task = update(&mut app, message);
            
            // Should not panic and app state should remain unchanged
            assert!(app.root_nodes[0].is_none());
        }

        #[test]
        fn test_update_with_invalid_message() {
            use crate::gui::{update, FileTreeApp, Message};
            use std::path::PathBuf;
            use tempfile::NamedTempFile;

            // Setup app with minimal state
            let dir = PathBuf::from("/dummy");
            let file_extensions = vec!["txt".to_string()];
            let temp_file = NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![dir], file_extensions.clone(), Vec::new(), persist_path);

            // Clone state before update
            let prev_state = app.clone();

            // Call update with a message that should have no effect
            let _ = update(&mut app, Message::ToggleExtension("invalid_ext".to_string()));

            // Assert that state is unchanged
            assert_eq!(app.selected_extensions, prev_state.selected_extensions);
            assert_eq!(app.right_panel_shuffled, prev_state.right_panel_shuffled);
        }

        #[test]
        fn test_toggle_extension_duplicate_handling() {
            use crate::gui::{update, FileTreeApp, Message};
            use std::path::PathBuf;
            use tempfile::NamedTempFile;

            let dir = PathBuf::from("/dummy");
            let file_extensions = vec!["txt".to_string()];
            let temp_file = NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![dir], file_extensions.clone(), Vec::new(), persist_path);

            // Initially, "txt" is selected
            assert_eq!(app.selected_extensions, vec!["txt".to_string()]);

            // Toggle "txt" extension once (should remove)
            let msg = Message::ToggleExtension("txt".to_string());
            let _ = update(&mut app, msg.clone());
            assert_eq!(app.selected_extensions.len(), 0);

            // Toggle "txt" again (should add)
            let _ = update(&mut app, msg.clone());
            assert_eq!(app.selected_extensions.len(), 1);

            // Toggle "txt" twice in a row (should remove then add, no duplicates)
            let _ = update(&mut app, msg.clone());
            let _ = update(&mut app, msg.clone());
            assert_eq!(app.selected_extensions.len(), 1);

            // Toggle "txt" three times (should remove, add, remove)
            let _ = update(&mut app, msg.clone());
            let _ = update(&mut app, msg.clone());
            let _ = update(&mut app, msg.clone());
            assert_eq!(app.selected_extensions.len(), 0);
        }

        #[test]
        fn test_update_performance_with_large_number_of_extensions() {

            let dir = PathBuf::from("/dummy");
            let num_ext = 1000;
            let file_extensions: Vec<String> = (0..num_ext).map(|i| format!("ext{i}")).collect();
            let temp_file = NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![dir], file_extensions.clone(), Vec::new(), persist_path);

            // Toggle all extensions off
            for ext in &file_extensions {
                let msg = Message::ToggleExtension(ext.clone());
                let _ = update(&mut app, msg);
            }
            assert!(app.selected_extensions.is_empty());

            // Toggle all extensions on
            for ext in &file_extensions {
                let msg = Message::ToggleExtension(ext.clone());
                let _ = update(&mut app, msg);
            }
            assert_eq!(app.selected_extensions.len(), num_ext);

            // Ensure no duplicates
            let unique: std::collections::HashSet<_> = app.selected_extensions.iter().collect();
            assert_eq!(unique.len(), num_ext);
        }

        #[test]
        fn test_toggle_extension() {
            let dir = PathBuf::from("/dummy");
            let file_extensions = vec!["txt".to_string(), "md".to_string()];
            let temp_file = NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![dir], file_extensions.clone(), Vec::new(), persist_path);

            let msg = Message::ToggleExtension("md".to_string());
            let _ = update(&mut app, msg);
            assert!(!app.selected_extensions.contains(&"md".to_string()));

            let msg = Message::ToggleExtension("md".to_string());
            let _ = update(&mut app, msg);
            assert!(app.selected_extensions.contains(&"md".to_string()));
        }

        #[test]
        fn test_toggle_extensions_menu() {
            let dir = PathBuf::from("/dummy");
            let file_extensions = vec!["txt".to_string()];
            let temp_file = NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![dir], file_extensions, Vec::new(), persist_path);

            let msg = Message::ToggleExtensionsMenu;
            let _ = update(&mut app, msg);
            assert!(app.extensions_menu_expanded);

            let _ = update(&mut app, Message::ToggleExtensionsMenu);
            assert!(!app.extensions_menu_expanded);
        }

        #[test]
        fn test_add_to_right_panel() {
            let file_path = PathBuf::from("/file.txt");
            let mut app = FileTreeApp::new(vec![], vec!["txt".to_string()], Vec::new(), PathBuf::from("/tmp"));
            let msg = Message::AddToRightPanel(file_path.clone());
            let _ = update(&mut app, msg);
            assert!(app.right_panel_files.iter().any(|f| f.path == file_path));
        }

        #[test]
        fn test_add_directory_to_right_panel() {
            let dir_path = PathBuf::from("/dir");
            let file1 = PathBuf::from("/dir/file1.txt");
            let file2 = PathBuf::from("/dir/file2.txt");
            let mut dir_node = FileNode::new_directory("dir".to_string(), dir_path.clone(), vec![]);
            dir_node.children.push(FileNode::new_file("file1.txt".to_string(), file1.clone()));
            dir_node.children.push(FileNode::new_file("file2.txt".to_string(), file2.clone()));

            let mut app = FileTreeApp::new(vec![dir_path.clone()], vec!["txt".to_string()], Vec::new(), PathBuf::from("/tmp"));
            app.root_nodes[0] = Some(dir_node);

            let msg = Message::AddDirectoryToRightPanel(dir_path.clone());
            let _ = update(&mut app, msg);
            assert!(app.right_panel_files.iter().any(|f| f.path == file1));
            assert!(app.right_panel_files.iter().any(|f| f.path == file2));
        }

        #[test]
        fn test_remove_from_right_panel() {
            let file_path = PathBuf::from("/file.txt");
            let mut app = FileTreeApp::new(vec![], vec!["txt".to_string()], Vec::new(), PathBuf::from("/tmp"));
            app.right_panel_files.push(RightPanelFile {
                path: file_path.clone(),
                musician: None,
                album: None,
                title: None,
                genre: None,
            });
            let msg = Message::RemoveFromRightPanel(file_path.clone());
            let _ = update(&mut app, msg);
            assert!(!app.right_panel_files.iter().any(|f| f.path == file_path));
        }

        #[test]
        fn test_remove_directory_from_right_panel() {
            let dir_path = PathBuf::from("/dir");
            let mut app = FileTreeApp::new(vec![], vec!["txt".to_string()], Vec::new(), PathBuf::from("/tmp"));
            let right_panel_file1 = RightPanelFile {
                path: PathBuf::from("/dir/file1.txt"),
                musician: None,
                album: None,
                title: None,
                genre: None,
            };
            let right_panel_file2 = RightPanelFile {
                path: PathBuf::from("/dir/file2.txt"),
                musician: None,
                album: None,
                title: None,
                genre: None,
            };
            let right_panel_file3 = RightPanelFile {
                path: PathBuf::from("/other/file3.txt"),
                musician: None,
                album: None,
                title: None,
                genre: None,
            };
            app.right_panel_files = vec![right_panel_file1.clone(), right_panel_file2.clone(), right_panel_file3.clone()];
            let msg = Message::RemoveDirectoryFromRightPanel(dir_path.clone());
            let _ = update(&mut app, msg);
            assert!(!app.right_panel_files.iter().any(|f| f.path == right_panel_file1.path));
            assert!(!app.right_panel_files.iter().any(|f| f.path == right_panel_file2.path));
            assert!(app.right_panel_files.iter().any(|f| f.path == right_panel_file3.path));
        }

        #[test]
        fn test_sort_right_panel_by_directory_and_file() {
            let file_a = PathBuf::from("/dir_a/file.txt");
            let file_b = PathBuf::from("/dir_b/file.txt");
            let mut app = FileTreeApp::new(vec![], vec!["txt".to_string()], Vec::new(), PathBuf::from("/tmp"));
            let right_panel_file_a = RightPanelFile {
                path: file_a.clone(),
                musician: None,
                album: None,
                title: None,
                genre: None,
            };
            let right_panel_file_b = RightPanelFile {
                path: file_b.clone(),
                musician: None,
                album: None,
                title: None,
                genre: None,
            };
            app.right_panel_files = vec![right_panel_file_b.clone(), right_panel_file_a.clone()];

            let msg = Message::SortRightPanelByDirectory;
            let _ = update(&mut app, msg);
            assert_eq!(app.right_panel_sort_column, SortColumn::Directory);
            assert_eq!(app.right_panel_sort_order, SortOrder::Desc);

            let _ = update(&mut app, Message::SortRightPanelByDirectory);
            assert_eq!(app.right_panel_sort_order, SortOrder::Asc);

            let msg = Message::SortRightPanelByFile;
            let _ = update(&mut app, msg);
            assert_eq!(app.right_panel_sort_column, SortColumn::File);
            assert_eq!(app.right_panel_sort_order, SortOrder::Asc);
        }

        #[test]
        fn test_shuffle_right_panel() {
            let file1 = PathBuf::from("/dir/file1.txt");
            let file2 = PathBuf::from("/dir/file2.txt");
            let mut app = FileTreeApp::new(vec![], vec!["txt".to_string()], Vec::new(), PathBuf::from("/tmp"));
            let right_panel_file1 = RightPanelFile {
                path: file1.clone(),
                musician: None,
                album: None,
                title: None,
                genre: None,
            };
            let right_panel_file2 = RightPanelFile {
                path: file2.clone(),
                musician: None,
                album: None,
                title: None,
                genre: None,
            };
            app.right_panel_files = vec![right_panel_file1.clone(), right_panel_file2.clone()];
            let msg = Message::ShuffleRightPanel;
            let _ = update(&mut app, msg);
            assert!(app.right_panel_shuffled);
        }

        #[test]
        fn test_add_duplicate_to_right_panel() {
            let file_path = PathBuf::from("/file.txt");
            let mut app = FileTreeApp::new(vec![], vec!["txt".to_string()], Vec::new(), PathBuf::from("/tmp"));
            let right_panel_file = RightPanelFile {
                path: file_path.clone(),
                musician: None,
                album: None,
                title: None,
                genre: None,
            };
            app.right_panel_files.push(right_panel_file.clone());
            let msg = Message::AddToRightPanel(file_path.clone());
            let _ = update(&mut app, msg);
            // Should not add duplicate
            assert_eq!(app.right_panel_files.iter().filter(|p| **p == right_panel_file).count(), 1);
        }

        #[test]
        fn test_remove_nonexistent_from_right_panel() {
            let file_path = PathBuf::from("/file.txt");
            let mut app = FileTreeApp::new(vec![], vec!["txt".to_string()], Vec::new(), PathBuf::from("/tmp"));
            // Try to remove a file that's not present
            let msg = Message::RemoveFromRightPanel(file_path.clone());
            let _ = update(&mut app, msg);
            // Should not panic and list remains empty
            assert!(app.right_panel_files.is_empty());
        }

        #[test]
        fn test_remove_nonexistent_directory_from_right_panel() {
            let dir_path = PathBuf::from("/dir");
            let mut app = FileTreeApp::new(vec![], vec!["txt".to_string()], Vec::new(), PathBuf::from("/tmp"));
            let right_panel_file = RightPanelFile {
                path: PathBuf::from("/other/file.txt"),
                musician: None,
                album: None,
                title: None,
                genre: None,
            };
            app.right_panel_files = vec![right_panel_file];
            let msg = Message::RemoveDirectoryFromRightPanel(dir_path.clone());
            let _ = update(&mut app, msg);
            // Should not remove unrelated files
            assert_eq!(app.right_panel_files.len(), 1);
        }

        #[test]
        fn test_sort_right_panel_empty_and_single() {
            let mut app = FileTreeApp::new(vec![], vec!["txt".to_string()], Vec::new(), PathBuf::from("/tmp"));
            // Empty list
            let _ = update(&mut app, Message::SortRightPanelByDirectory);
            assert!(app.right_panel_files.is_empty());

            // Single item
            let right_panel_file = RightPanelFile {
                path: PathBuf::from("/dir/file.txt"),
                musician: None,
                album: None,
                title: None,
                genre: None,
            };
            app.right_panel_files.push(right_panel_file.clone());
            let _ = update(&mut app, Message::SortRightPanelByFile);
            assert_eq!(app.right_panel_files.len(), 1);
            assert_eq!(app.right_panel_files[0], right_panel_file);
        }

        #[test]
        fn test_shuffle_right_panel_empty_and_single() {
            let mut app = FileTreeApp::new(vec![], vec!["txt".to_string()], Vec::new(), PathBuf::from("/tmp"));
            // Empty list
            let _ = update(&mut app, Message::ShuffleRightPanel);
            assert!(app.right_panel_shuffled);

            // Single item
            let right_panel_file = RightPanelFile {
                path: PathBuf::from("/dir/file.txt"),
                musician: None,
                album: None,
                title: None,
                genre: None,
            };
            app.right_panel_files.push(right_panel_file.clone());
            let _ = update(&mut app, Message::ShuffleRightPanel);
            assert!(app.right_panel_shuffled);
            assert_eq!(app.right_panel_files.len(), 1);
            assert_eq!(app.right_panel_files[0], right_panel_file);
        }

        #[test]
        fn test_sort_then_shuffle_then_sort_right_panel() {
            let right_panel_file1 = RightPanelFile {
                path: PathBuf::from("/dir_a/file1.txt"),
                musician: None,
                album: None,
                title: None,
                genre: None,
            };
            let right_panel_file2 = RightPanelFile {
                path: PathBuf::from("/dir_a/file2.txt"),
                musician: None,
                album: None,
                title: None,
                genre: None,
            };
            let mut app = FileTreeApp::new(vec![], vec!["txt".to_string()], Vec::new(), PathBuf::from("/tmp"));
            app.right_panel_files = vec![right_panel_file1.clone(), right_panel_file2.clone()];

            // Sort
            let _ = update(&mut app, Message::SortRightPanelByDirectory);
            assert!(!app.right_panel_shuffled);

            // Shuffle
            let _ = update(&mut app, Message::ShuffleRightPanel);
            assert!(app.right_panel_shuffled);

            // Sort again
            let _ = update(&mut app, Message::SortRightPanelByFile);
            assert!(!app.right_panel_shuffled);
        }

        #[test]
        fn test_toggle_extension_with_empty_file_extensions() {
            let dir = PathBuf::from("/dummy");
            let file_extensions: Vec<String> = vec![];
            let temp_file = NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![dir], file_extensions, Vec::new(), persist_path);

            // Try toggling a non-existent extension
            let msg = Message::ToggleExtension("md".to_string());
            let _ = update(&mut app, msg);
            // Should not add "md" to selected_extensions
            assert!(!app.selected_extensions.contains(&"md".to_string()));
        }

        #[test]
        fn test_toggle_extensions_menu_with_empty_extensions() {
            let dir = PathBuf::from("/dummy");
            let file_extensions: Vec<String> = vec![];
            let temp_file = NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![dir], file_extensions, Vec::new(), persist_path);

            // Toggle menu open and closed
            let msg = Message::ToggleExtensionsMenu;
            let _ = update(&mut app, msg);
            assert!(app.extensions_menu_expanded);

            let _ = update(&mut app, Message::ToggleExtensionsMenu);
            assert!(!app.extensions_menu_expanded);
        }

        #[test]
        fn test_toggle_extension_with_empty_string() {
            use crate::gui::{update, FileTreeApp, Message};
            use std::path::PathBuf;
            use tempfile::NamedTempFile;

            let dir = PathBuf::from("/dummy");
            let file_extensions = vec!["txt".to_string()];
            let temp_file = NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![dir], file_extensions.clone(), Vec::new(), persist_path);

            let msg = Message::ToggleExtension("".to_string());
            let _ = update(&mut app, msg);
            assert!(!app.selected_extensions.contains(&"".to_string()));
        }

        #[test]
        fn test_toggle_extension_with_nonexistent_extension() {
            use crate::gui::{update, FileTreeApp, Message};
            use std::path::PathBuf;
            use tempfile::NamedTempFile;

            let dir = PathBuf::from("/dummy");
            let file_extensions = vec!["txt".to_string()];
            let temp_file = NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![dir], file_extensions.clone(), Vec::new(), persist_path);

            let msg = Message::ToggleExtension("nonexistent".to_string());
            let _ = update(&mut app, msg);
            assert!(!app.selected_extensions.contains(&"nonexistent".to_string()));
        }

        #[test]
        fn test_toggle_extension_with_special_characters() {
            use crate::gui::{update, FileTreeApp, Message};
            use std::path::PathBuf;
            use tempfile::NamedTempFile;

            let dir = PathBuf::from("/dummy");
            let file_extensions = vec!["txt".to_string()];
            let temp_file = NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![dir], file_extensions.clone(), Vec::new(), persist_path);

            let msg = Message::ToggleExtension("💥".to_string());
            let _ = update(&mut app, msg);
            assert!(!app.selected_extensions.contains(&"💥".to_string()));
        }

        #[test]
        fn test_directory_added_with_file_path() {
            use std::fs::File;
            use tempfile::tempdir;

            let temp_dir = tempdir().unwrap();
            let file_path = temp_dir.path().join("testfile.txt");
            File::create(&file_path).unwrap();

            let file_extensions = vec!["txt".to_string()];
            let temp_file = NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![], file_extensions, Vec::new(), persist_path);

            // Simulate adding a file path
            let message = Message::DirectoryAdded(Some(file_path.clone()));
            let _ = update(&mut app, message);

            // The parent directory should be added, not the file itself
            assert!(app.top_dirs.contains(&temp_dir.path().to_path_buf()));
            assert!(!app.top_dirs.contains(&file_path));
        }

        #[test]
        fn test_deeply_nested_expansion() {
            let dir = PathBuf::from("/dummy");
            let mut root = FileNode::new_directory("root".to_string(), PathBuf::from("/root"), vec![]);
            let mut level1 = FileNode::new_directory("level1".to_string(), PathBuf::from("/root/level1"), vec![]);
            let mut level2 = FileNode::new_directory("level2".to_string(), PathBuf::from("/root/level1/level2"), vec![]);
            let file_extensions: Vec<String> = vec!["txt", "md"].into_iter().map(|s| s.to_string()).collect();

            level2.children.push(FileNode::new_file("deep.txt".to_string(), PathBuf::from("/root/level1/level2/deep.txt")));
            level1.children.push(level2);
            root.children.push(level1);

            let temp_file = NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![dir], file_extensions, Vec::new(), persist_path);
            app.root_nodes[0] = Some(root); // manually set the test tree

            // Test expanding deeply nested directory
            let deep_path = PathBuf::from("/root/level1/level2");
            let message = Message::ToggleExpansion(deep_path);
            let _task = update(&mut app, message);

            // Verify the deep directory was expanded
            let level2_node = &app.root_nodes[0].as_ref().unwrap().children[0].children[0];
            assert!(level2_node.is_expanded);
        }

    }

    mod view_tests {
        // View/rendering/UI feedback tests here
        use super::*;
        use crate::fs::file_tree::scan_directory;

        #[test]
        fn test_view_with_root_node() {
            let dir = PathBuf::from("/dummy");
            let file_extensions = vec!["txt", "md"].into_iter().map(|s| s.to_string()).collect();
            let temp_file = NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let app = FileTreeApp::new(vec![dir], file_extensions, Vec::new(), persist_path);
            
            let _element = view(&app);
            
            // Test passes if view() doesn't panic
            // We can't easily inspect Element content without custom renderer
        }

        #[test]
        fn test_view_with_no_root_node() {
            let dir = PathBuf::from("/dummy");
            let file_extensions = vec!["txt", "md"].into_iter().map(|s| s.to_string()).collect();
            let temp_file = NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let app = FileTreeApp::new(vec![dir], file_extensions, Vec::new(), persist_path);
            
            let _element = view(&app);
            
            // Test passes if view() doesn't panic when rendering empty state
        }

        #[test]
        fn test_view_renders_with_empty_state() {
            let app = FileTreeApp::new(vec![], vec![], Vec::new(), PathBuf::from("/tmp"));
            let _element = view(&app);
            // Test passes if view() does not panic
        }

        #[test]
        fn test_view_renders_with_many_extensions() {
            let num_ext = 1000;
            let file_extensions: Vec<String> = (0..num_ext).map(|i| format!("ext{i}")).collect();
            let temp_file = NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let app = FileTreeApp::new(vec![PathBuf::from("/dummy")], file_extensions, Vec::new(), persist_path);

            let _element = view(&app);
            // Test passes if view() does not panic
        }

        #[test]
        fn test_extension_menu_labels_with_many_extensions() {
            let num_ext = 1000;
            let file_extensions: Vec<String> = (0..num_ext).map(|i| format!("ext{i}")).collect();
            let temp_file = tempfile::NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![std::path::PathBuf::from("/dummy")], file_extensions.clone(), Vec::new(), persist_path);

            // Expand the menu to render all extensions
            app.extensions_menu_expanded = true;

            // Generate expected labels
            let expected_labels: Vec<String> = file_extensions.iter().map(|ext| format!("[x] .{ext}")).collect();

            // Collect actual labels from extension_menu
            let mut actual_labels = Vec::new();
            for ext in &app.all_extensions {
                let checked = app.selected_extensions.contains(ext);
                let label = if checked { format!("[x] .{ext}") } else { format!("[ ] .{ext}") };
                actual_labels.push(label);
            }

            for expected in expected_labels {
                assert!(
                    actual_labels.contains(&expected),
                    "Extension label '{expected}' not found in menu",
                );
            }
        }

        #[test]
        fn test_extension_menu_label_feedback_on_toggle() {
            let file_extensions = vec!["foo".to_string(), "bar".to_string()];
            let temp_file = tempfile::NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![std::path::PathBuf::from("/dummy")], file_extensions.clone(), Vec::new(), persist_path);

            app.extensions_menu_expanded = true;

            // Initial labels (all checked)
            let labels_before: Vec<String> = file_extensions
                .iter()
                .map(|ext| format!("[x] .{ext}"))
                .collect();

            for ext in &file_extensions {
                let checked = app.selected_extensions.contains(ext);
                let label = if checked { format!("[x] .{ext}") } else { format!("[ ] .{ext}") };
                assert!(labels_before.contains(&label));
            }

            // Toggle "foo" off
            let msg = Message::ToggleExtension("foo".to_string());
            let _ = update(&mut app, msg);

            // Labels after toggle
            let labels_after: Vec<String> = file_extensions
                .iter()
                .map(|ext| {
                    let checked = app.selected_extensions.contains(ext);
                    if checked { format!("[x] .{ext}") } else { format!("[ ] .{ext}") }
                })
                .collect();

            assert!(labels_after.contains(&"[ ] .foo".to_string()));
            assert!(labels_after.contains(&"[x] .bar".to_string()));

            // Toggle "foo" on again
            let msg = Message::ToggleExtension("foo".to_string());
            let _ = update(&mut app, msg);

            // Labels after second toggle
            let labels_final: Vec<String> = file_extensions
                .iter()
                .map(|ext| {
                    let checked = app.selected_extensions.contains(ext);
                    if checked { format!("[x] .{ext}") } else { format!("[ ] .{ext}") }
                })
                .collect();

            assert!(labels_final.contains(&"[x] .foo".to_string()));
            assert!(labels_final.contains(&"[x] .bar".to_string()));
        }

        #[test]
        fn test_directory_expansion_ui_feedback() {
            let dir_path = std::path::PathBuf::from("/root");
            let mut dir_node = FileNode::new_directory("root".to_string(), dir_path.clone(), vec![]);
            dir_node.is_expanded = false;
            let file_extensions = vec!["txt".to_string()];
            let temp_file = tempfile::NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![dir_path.clone()], file_extensions, Vec::new(), persist_path);
            app.root_nodes[0] = Some(dir_node);

            // Helper to get expansion symbol from rendered label
            fn get_expansion_symbol(app: &FileTreeApp) -> String {
                let node = app.root_nodes[0].as_ref().unwrap();
                let indent = "  ".repeat(0);
                let expand_symbol = if node.is_expanded { "▼" } else { "▶" };
                format!("{}{} 📁 {}", indent, expand_symbol, node.name)
            }

            // Initially collapsed
            let label_before = get_expansion_symbol(&app);
            assert!(label_before.contains("▶"), "Expected collapsed symbol");

            // Toggle expansion
            let msg = Message::ToggleExpansion(dir_path.clone());
            let _ = update(&mut app, msg);

            // Should be expanded now
            let label_after = get_expansion_symbol(&app);
            assert!(label_after.contains("▼"), "Expected expanded symbol");

            // Toggle again to collapse
            let msg = Message::ToggleExpansion(dir_path.clone());
            let _ = update(&mut app, msg);

            let label_final = get_expansion_symbol(&app);
            assert!(label_final.contains("▶"), "Expected collapsed symbol again");
        }

        #[test]
        fn test_right_panel_file_selection_ui_feedback() {
            let file_path = std::path::PathBuf::from("/file.txt");
            let file_extensions = vec!["txt".to_string()];
            let temp_file = tempfile::NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![], file_extensions, Vec::new(), persist_path);

            // Add file to right panel
            let msg_add = Message::AddToRightPanel(file_path.clone());
            let _ = update(&mut app, msg_add);
            assert!(app.right_panel_files.iter().any(|f| f.path == file_path), "File should be in right panel after adding");

            // Remove file from right panel
            let msg_remove = Message::RemoveFromRightPanel(file_path.clone());
            let _ = update(&mut app, msg_remove);
            assert!(!app.right_panel_files.iter().any(|f| f.path == file_path), "File should not be in right panel after removing");
        }

        #[test]
        fn test_right_panel_remove_directory_ui_feedback() {
            let dir_path = std::path::PathBuf::from("/dir");
            let right_panel_file1 = RightPanelFile {
                path: dir_path.join("file1.txt"),
                musician: None,
                album: None,
                title: None,
                genre: None,
            };
            let right_panel_file2 = RightPanelFile {
                path: dir_path.join("file2.txt"),
                musician: None,
                album: None,
                title: None,
                genre: None,
            };
            let right_panel_file3 = RightPanelFile {
                path: std::path::PathBuf::from("/other/file3.txt"),
                musician: None,
                album: None,
                title: None,
                genre: None,
            };
            let file_extensions = vec!["txt".to_string()];
            let temp_file = tempfile::NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![], file_extensions, Vec::new(), persist_path);

            app.right_panel_files = vec![right_panel_file1.clone(), right_panel_file2.clone(), right_panel_file3.clone()];

            // Remove all files in /dir
            let msg_remove_dir = Message::RemoveDirectoryFromRightPanel(dir_path.clone());
            let _ = update(&mut app, msg_remove_dir);

            assert!(!app.right_panel_files.iter().any(|f| f.path == right_panel_file1.path), "file1 should be removed from right panel");
            assert!(!app.right_panel_files.iter().any(|f| f.path == right_panel_file2.path), "file2 should be removed from right panel");
            assert!(app.right_panel_files.iter().any(|f| f.path == right_panel_file3.path), "file3 should be removed from right panel");
        }

        #[test]
        fn test_right_panel_shuffle_and_sort_ui_feedback() {
            let right_panel_file1 = RightPanelFile {
                path: std::path::PathBuf::from("/dir_a/file1.txt"),
                musician: None,
                album: None,
                title: None,
                genre: None,
            };
            let right_panel_file2 = RightPanelFile {
                path: std::path::PathBuf::from("/dir_a/file2.txt"),
                musician: None,
                album: None,
                title: None,
                genre: None,
            };
            let file_extensions = vec!["txt".to_string()];
            let temp_file = tempfile::NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![], file_extensions, Vec::new(), persist_path);

            app.right_panel_files = vec![right_panel_file1.clone(), right_panel_file2.clone()];

            // Shuffle right panel
            let msg_shuffle = Message::ShuffleRightPanel;
            let _ = update(&mut app, msg_shuffle);
            assert!(app.right_panel_shuffled, "Right panel should be marked as shuffled");

            // Sort right panel by directory
            let msg_sort = Message::SortRightPanelByDirectory;
            let _ = update(&mut app, msg_sort);
            assert!(!app.right_panel_shuffled, "Right panel should not be marked as shuffled after sorting");
            assert_eq!(app.right_panel_sort_column, SortColumn::Directory, "Sort column should be Directory");
        }

        #[test]
        fn test_add_directory_to_right_panel_ui_feedback() {
            let dir_path = std::path::PathBuf::from("/dir");
            let file1 = dir_path.join("file1.txt");
            let file2 = dir_path.join("file2.txt");
            let mut dir_node = FileNode::new_directory("dir".to_string(), dir_path.clone(), vec![]);
            dir_node.children.push(FileNode::new_file("file1.txt".to_string(), file1.clone()));
            dir_node.children.push(FileNode::new_file("file2.txt".to_string(), file2.clone()));
            let file_extensions = vec!["txt".to_string()];
            let temp_file = tempfile::NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![dir_path.clone()], file_extensions, Vec::new(), persist_path);
            app.root_nodes[0] = Some(dir_node);

            let msg = Message::AddDirectoryToRightPanel(dir_path.clone());
            let _ = update(&mut app, msg);

            assert!(app.right_panel_files.iter().any(|f| f.path == file1), "file1 should be removed from right panel");
            assert!(app.right_panel_files.iter().any(|f| f.path == file2), "file2 should be removed from right panel");
        }

        #[test]
        fn test_remove_top_dir_ui_feedback() {
            let dir_path = std::path::PathBuf::from("/dir");
            let file_extensions = vec!["txt".to_string()];
            let temp_file = tempfile::NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![dir_path.clone()], file_extensions, Vec::new(), persist_path);

            assert!(app.top_dirs.contains(&dir_path), "Top dir should be present before removal");

            let msg = Message::RemoveTopDir(dir_path.clone());
            let _ = update(&mut app, msg);

            assert!(!app.top_dirs.contains(&dir_path), "Top dir should be removed after action");
        }

        #[test]
        fn test_toggle_extensions_menu_ui_feedback() {
            let dir_path = std::path::PathBuf::from("/dummy");
            let file_extensions = vec!["txt".to_string()];
            let temp_file = tempfile::NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![dir_path.clone()], file_extensions, Vec::new(), persist_path);

            assert!(!app.extensions_menu_expanded, "Menu should be collapsed initially");

            let msg = Message::ToggleExtensionsMenu;
            let _ = update(&mut app, msg);
            assert!(app.extensions_menu_expanded, "Menu should be expanded after toggle");

            let _ = update(&mut app, Message::ToggleExtensionsMenu);
            assert!(!app.extensions_menu_expanded, "Menu should be collapsed after second toggle");
        }

        #[test]
        fn test_remove_nonexistent_file_from_right_panel_ui_feedback() {
            let file_path = std::path::PathBuf::from("/not_present.txt");
            let file_extensions = vec!["txt".to_string()];
            let temp_file = tempfile::NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![], file_extensions, Vec::new(), persist_path);

            // Try to remove a file that's not present
            let msg = Message::RemoveFromRightPanel(file_path.clone());
            let _ = update(&mut app, msg);

            // Should not panic and list remains empty
            assert!(app.right_panel_files.is_empty(), "Right panel should remain empty");
        }

        #[test]
        fn test_remove_nonexistent_directory_from_right_panel_ui_feedback() {
            let dir_path = std::path::PathBuf::from("/not_present_dir");
            let right_panel_file = RightPanelFile {
                path: std::path::PathBuf::from("/other/file.txt"),
                musician: None,
                album: None,
                title: None,
                genre: None,
            };
            let file_extensions = vec!["txt".to_string()];
            let temp_file = tempfile::NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![], file_extensions, Vec::new(), persist_path);

            app.right_panel_files = vec![right_panel_file.clone()];

            let msg = Message::RemoveDirectoryFromRightPanel(dir_path.clone());
            let _ = update(&mut app, msg);

            // Should not remove unrelated files
            assert_eq!(app.right_panel_files.len(), 1, "Unrelated file should remain");
            assert!(app.right_panel_files.iter().any(|f| f.path == right_panel_file.path), "Unrelated file should remain");
        }

        #[test]
        fn test_toggle_nonexistent_extension_ui_feedback() {
            let dir_path = std::path::PathBuf::from("/dummy");
            let file_extensions = vec!["txt".to_string()];
            let temp_file = tempfile::NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![dir_path.clone()], file_extensions.clone(), Vec::new(), persist_path);

            let prev_selected = app.selected_extensions.clone();

            // Try toggling an extension not in file_extensions
            let msg = Message::ToggleExtension("md".to_string());
            let _ = update(&mut app, msg);

            // Should not add "md" to selected_extensions
            assert_eq!(app.selected_extensions, prev_selected, "Selected extensions should not change");
        }

        #[test]
        fn test_extension_menu_visual_consistency_after_toggle() {
            let file_extensions = vec!["foo".to_string(), "bar".to_string()];
            let temp_file = tempfile::NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![std::path::PathBuf::from("/dummy")], file_extensions.clone(), Vec::new(), persist_path);
            app.extensions_menu_expanded = true;

            // Initial: all checked
            let labels = file_extensions.iter().map(|ext| format!("[x] .{ext}")).collect::<Vec<_>>();
            for ext in &file_extensions {
                let checked = app.selected_extensions.contains(ext);
                let label = if checked { format!("[x] .{ext}") } else { format!("[ ] .{ext}") };
                assert!(labels.contains(&label));
            }

            // Toggle "foo" off, then "bar" off
            let _ = update(&mut app, Message::ToggleExtension("foo".to_string()));
            let _ = update(&mut app, Message::ToggleExtension("bar".to_string()));
            let labels = file_extensions.iter().map(|ext| {
                let checked = app.selected_extensions.contains(ext);
                if checked { format!("[x] .{ext}") } else { format!("[ ] .{ext}") }
            }).collect::<Vec<_>>();
            assert!(labels.contains(&"[ ] .foo".to_string()));
            assert!(labels.contains(&"[ ] .bar".to_string()));

            // Toggle both on again
            let _ = update(&mut app, Message::ToggleExtension("foo".to_string()));
            let _ = update(&mut app, Message::ToggleExtension("bar".to_string()));
            let labels = file_extensions.iter().map(|ext| {
                let checked = app.selected_extensions.contains(ext);
                if checked { format!("[x] .{ext}") } else { format!("[ ] .{ext}") }
            }).collect::<Vec<_>>();
            assert!(labels.contains(&"[x] .foo".to_string()));
            assert!(labels.contains(&"[x] .bar".to_string()));
        }

        #[test]
        fn test_directory_expansion_visual_consistency_after_toggle() {
            let dir_path = std::path::PathBuf::from("/root");
            let mut dir_node = FileNode::new_directory("root".to_string(), dir_path.clone(), vec![]);
            dir_node.is_expanded = false;
            let file_extensions = vec!["txt".to_string()];
            let temp_file = tempfile::NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![dir_path.clone()], file_extensions, Vec::new(), persist_path);
            app.root_nodes[0] = Some(dir_node);

            fn get_expansion_symbol(app: &FileTreeApp) -> String {
                let node = app.root_nodes[0].as_ref().unwrap();
                let expand_symbol = if node.is_expanded { "▼" } else { "▶" };
                expand_symbol.to_string()
                // format!("{expand_symbol}")
            }

            // Initial: collapsed
            assert_eq!(get_expansion_symbol(&app), "▶");

            // Expand
            let _ = update(&mut app, Message::ToggleExpansion(dir_path.clone()));
            assert_eq!(get_expansion_symbol(&app), "▼");

            // Collapse
            let _ = update(&mut app, Message::ToggleExpansion(dir_path.clone()));
            assert_eq!(get_expansion_symbol(&app), "▶");
        }

        #[test]
        fn test_right_panel_visual_consistency_after_add_remove() {
            let file_path = std::path::PathBuf::from("/file.txt");
            let file_extensions = vec!["txt".to_string()];
            let temp_file = tempfile::NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![], file_extensions, Vec::new(), persist_path);

            // Add file
            let _ = update(&mut app, Message::AddToRightPanel(file_path.clone()));
            assert!(app.right_panel_files.iter().any(|f| f.path == file_path));

            // Remove file
            let _ = update(&mut app, Message::RemoveFromRightPanel(file_path.clone()));
            assert!(!app.right_panel_files.iter().any(|f| f.path == file_path));

            // Add again
            let _ = update(&mut app, Message::AddToRightPanel(file_path.clone()));
            assert!(app.right_panel_files.iter().any(|f| f.path == file_path));
        }

        #[test]
        fn test_deeply_nested_directory_visual_consistency() {
            let root_path = std::path::PathBuf::from("/root");
            let level1_path = root_path.join("level1");
            let level2_path = level1_path.join("level2");
            let file_path = level2_path.join("deep.txt");

            let mut level2 = FileNode::new_directory("level2".to_string(), level2_path.clone(), vec![]);
            level2.children.push(FileNode::new_file("deep.txt".to_string(), file_path.clone()));
            let level1 = FileNode::new_directory("level1".to_string(), level1_path.clone(), vec![level2]);
            let root = FileNode::new_directory("root".to_string(), root_path.clone(), vec![level1]);

            let file_extensions = vec!["txt".to_string()];
            let temp_file = tempfile::NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![root_path.clone()], file_extensions, Vec::new(), persist_path);
            app.root_nodes[0] = Some(root);

            // Helper to get expansion symbol for a given depth
            fn get_expansion_symbol(node: &FileNode) -> &str {
                if node.is_expanded { "▼" } else { "▶" }
            }

            // Initial: all collapsed
            let root_node = app.root_nodes[0].as_ref().unwrap();
            assert_eq!(get_expansion_symbol(root_node), "▶");
            assert_eq!(get_expansion_symbol(&root_node.children[0]), "▶");
            assert_eq!(get_expansion_symbol(&root_node.children[0].children[0]), "▶");

            // Expand root
            let _ = update(&mut app, Message::ToggleExpansion(root_path.clone()));
            let root_node = app.root_nodes[0].as_ref().unwrap();
            assert_eq!(get_expansion_symbol(root_node), "▼");

            // Expand level1
            let _ = update(&mut app, Message::ToggleExpansion(level1_path.clone()));
            let root_node = app.root_nodes[0].as_ref().unwrap();
            assert_eq!(get_expansion_symbol(&root_node.children[0]), "▼");

            // Expand level2
            let _ = update(&mut app, Message::ToggleExpansion(level2_path.clone()));
            let root_node = app.root_nodes[0].as_ref().unwrap();
            assert_eq!(get_expansion_symbol(&root_node.children[0].children[0]), "▼");

            // Collapse level2
            let _ = update(&mut app, Message::ToggleExpansion(level2_path.clone()));
            let root_node = app.root_nodes[0].as_ref().unwrap();
            assert_eq!(get_expansion_symbol(&root_node.children[0].children[0]), "▶");
        }

        #[test]
        fn test_multiple_state_changes_visual_consistency() {
            let file1 = std::path::PathBuf::from("/dir/file1.txt");
            let file2 = std::path::PathBuf::from("/dir/file2.txt");
            let file_extensions = vec!["txt".to_string(), "md".to_string()];
            let temp_file = tempfile::NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![], file_extensions.clone(), Vec::new(), persist_path);

            // Add both files
            let _ = update(&mut app, Message::AddToRightPanel(file1.clone()));
            let _ = update(&mut app, Message::AddToRightPanel(file2.clone()));
            assert!(app.right_panel_files.iter().any(|f| f.path == file1));
            assert!(app.right_panel_files.iter().any(|f| f.path == file2));

            // Toggle extensions off
            let _ = update(&mut app, Message::ToggleExtension("txt".to_string()));
            let _ = update(&mut app, Message::ToggleExtension("md".to_string()));
            assert!(!app.selected_extensions.contains(&"txt".to_string()));
            assert!(!app.selected_extensions.contains(&"md".to_string()));

            // Remove one file
            let _ = update(&mut app, Message::RemoveFromRightPanel(file1.clone()));
            assert!(!app.right_panel_files.iter().any(|f| f.path == file1));
            assert!(app.right_panel_files.iter().any(|f| f.path == file2));

            // Toggle extensions on again
            let _ = update(&mut app, Message::ToggleExtension("txt".to_string()));
            let _ = update(&mut app, Message::ToggleExtension("md".to_string()));
            assert!(app.selected_extensions.contains(&"txt".to_string()));
            assert!(app.selected_extensions.contains(&"md".to_string()));
        }

        #[test]
        fn test_render_node_file() {

            let flat_button_style = |_theme: &iced::Theme, _status: iced::widget::button::Status| iced::widget::button::Style {
                background: None,
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                text_color: iced::Color::WHITE,
            };

            let file_node = FileNode::new_file(
                "test.txt".to_string(),
                PathBuf::from("/test.txt")
            );
            
            let row_size = 12;
            let _element = render_node(&file_node, 0, row_size, row_size, flat_button_style);
            // Test passes if render_node() doesn't panic
        }

        #[test]
        fn test_render_node_directory() {

            let flat_button_style = |_theme: &iced::Theme, _status: iced::widget::button::Status| iced::widget::button::Style {
                background: None,
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                text_color: iced::Color::WHITE,
            };

            let dir_node = FileNode::new_directory(
                "testdir".to_string(),
                PathBuf::from("/testdir"),
                vec![]
            );
            
            let row_size = 12;
            let _element = render_node(&dir_node, 1, row_size, row_size, flat_button_style);
            // Test passes if render_node() doesn't panic
        }

        #[test]
        fn test_integration_with_real_directory() {
            let dir = PathBuf::from("/dummy");
            let file_extensions = vec!["txt", "md"].into_iter().map(|s| s.to_string()).collect();
            // Create a temporary directory structure for testing
            let temp_dir = tempdir().unwrap();
            let root = temp_dir.path();
            
            // Create test files
            File::create(root.join("test.txt")).unwrap();
            File::create(root.join("test.rs")).unwrap();
            File::create(root.join("ignored.doc")).unwrap();
            
            // Create subdirectory with files
            let subdir = root.join("subdir");
            std::fs::create_dir(&subdir).unwrap();
            File::create(subdir.join("nested.txt")).unwrap();
            
            let allowed = ["txt", "rs"];
            let root_node = scan_directory(root, &allowed);
            
            assert!(root_node.is_some());
            
            // Test that the app can be created and updated
            let temp_file = NamedTempFile::new().unwrap();
            let persist_path = temp_file.path().to_path_buf();
            let mut app = FileTreeApp::new(vec![dir], file_extensions, Vec::new(), persist_path);
            
            // Test expanding the subdirectory
            let subdir_path = subdir.to_path_buf();
            let message = Message::ToggleExpansion(subdir_path);
            let _task = update(&mut app, message);
            
            // Test view rendering doesn't panic
            let _element = view(&app);
            
            // Test passes if all operations complete without panicking
        }

    }
}
