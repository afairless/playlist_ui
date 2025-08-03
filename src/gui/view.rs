use iced::{Element, widget::{button, column, container, row, scrollable, text, Space}, Length};
use iced_aw::widgets::ContextMenu;
use crate::gui::{FileTreeApp, Message, SortColumn, SortOrder};
use crate::file_tree::{FileNode, NodeType};

pub fn extension_menu(app: &FileTreeApp) -> Element<Message> {
    let header = button(
        text(if app.extensions_menu_expanded { "▼ File Extensions" } else { "▶ File Extensions" }).size(16)
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

pub fn right_panel(app: &FileTreeApp) -> iced::Element<Message> {
    let mut col = iced::widget::Column::new();

    let dir_arrow = if app.right_panel_sort_column == SortColumn::Directory {
        match app.right_panel_sort_order {
            SortOrder::Desc => " ↑",
            SortOrder::Asc => " ↓",
        }
    } else {
        ""
    };
    let file_arrow = if app.right_panel_sort_column == SortColumn::File {
        match app.right_panel_sort_order {
            SortOrder::Desc => " ↑",
            SortOrder::Asc => " ↓",
        }
    } else {
        ""
    };
    let shuffle_btn = iced::widget::button(
        iced::widget::text("Shuffle")
            .width(Length::Shrink)
            .size(20)
    )
    .on_press(Message::ShuffleRightPanel)
    .width(Length::Shrink);

    let header_row = iced::widget::Row::new()
        .push(
            iced::widget::button(
                iced::widget::text(format!("Directory{dir_arrow}"))
                    .width(Length::FillPortion(1))
                    .size(24)
                    .style(|_theme| iced::widget::text::Style {
                        color: Some([0.5, 0.5, 0.5, 1.0].into()),
                    })
            )
            .on_press(Message::SortRightPanelByDirectory)
            .width(Length::FillPortion(1))
        )
        .push(
            iced::widget::button(
                iced::widget::text(format!("File{file_arrow}"))
                    .width(Length::FillPortion(1))
                    .size(24)
                    .style(|_theme| iced::widget::text::Style {
                        color: Some([0.5, 0.5, 0.5, 1.0].into()),
                    })
            )
            .on_press(Message::SortRightPanelByFile)
            .width(Length::FillPortion(1))
        )
        .push(shuffle_btn);
    col = col.push(header_row);

    let mut displayed_files = app.right_panel_files.clone();
    if !app.right_panel_shuffled {
        displayed_files.sort_by(|a, b| {
            match app.right_panel_sort_column {
                SortColumn::Directory => {
                    let a_dir = a.parent().and_then(|p| p.file_name()).unwrap_or_default().to_string_lossy().to_ascii_lowercase();
                    let b_dir = b.parent().and_then(|p| p.file_name()).unwrap_or_default().to_string_lossy().to_ascii_lowercase();
                    if app.right_panel_sort_order == SortOrder::Asc {
                        a_dir.cmp(&b_dir)
                    } else {
                        b_dir.cmp(&a_dir)
                    }
                }
                SortColumn::File => {
                    let a_file = a.file_name().unwrap_or_default().to_string_lossy().to_ascii_lowercase();
                    let b_file = b.file_name().unwrap_or_default().to_string_lossy().to_ascii_lowercase();
                    if app.right_panel_sort_order == SortOrder::Asc {
                        a_file.cmp(&b_file)
                    } else {
                        b_file.cmp(&a_file)
                    }
                }
            }
        });
    }

    for file in &displayed_files {
        let dirname = file.parent()
            .and_then(|p| p.file_name())
            .map(|d| d.to_string_lossy().to_string())
            .unwrap_or_default();
        let filename = file.file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();

        let dir_path = file.parent().map(|p| p.to_path_buf());

        let dir_widget = if let Some(path) = dir_path {
            let dir_path = path.clone();
            iced_aw::widgets::ContextMenu::new(
                iced::widget::text(dirname.clone()).width(Length::FillPortion(1)),
                Box::new(move || {
                    iced::widget::column![
                        iced::widget::button("Delete All in Directory")
                            .on_press(Message::RemoveDirectoryFromRightPanel(dir_path.clone()))
                    ].into()
                }) as Box<dyn Fn() -> iced::Element<'static, Message>>
            )
        } else {
            iced_aw::widgets::ContextMenu::new(
                iced::widget::text(dirname.clone()).width(Length::FillPortion(1)),
                Box::new(|| iced::widget::column![].into()) as Box<dyn Fn() -> iced::Element<'static, Message>>
            )
        };

        let file_context_menu = iced_aw::widgets::ContextMenu::new(
            iced::widget::text(filename.clone()).width(Length::FillPortion(1)),
            {
                let file_path = file.clone();
                Box::new(move || {
                    iced::widget::column![
                        iced::widget::button("Delete")
                            .on_press(Message::RemoveFromRightPanel(file_path.clone()))
                    ].into()
                }) as Box<dyn Fn() -> iced::Element<'static, Message>>
            }
        );

        let row = iced::widget::Row::new()
            .push(dir_widget)
            .push(file_context_menu);

        col = col.push(row);
    }
    col.into()
}

pub fn view(app: &FileTreeApp) -> Element<Message> {

    let add_dir_btn = iced::widget::button::<Message, iced::Theme, iced::Renderer>(
        iced::widget::text("Add Directory")
    )
    .on_press(Message::AddDirectory);

    let add_dir_row = iced::widget::row![add_dir_btn];

    let ext_menu = extension_menu(app);

    let mut trees = column![];
    for (i, node_opt) in app.root_nodes.iter().enumerate() {
        let dir_name = if let Some(p) = app.top_dirs.get(i) {
            if let Some(name) = p.file_name().and_then(|os_str| os_str.to_str()) {
                name.to_string()
            } else {
                p.display().to_string()
            }
        } else {
            String::new()
        };
        let dir_label = text(format!("Top-level directory: {dir_name}")).size(16);

        let remove_btn = button(text("Remove"))
            .on_press(Message::RemoveTopDir(app.top_dirs[i].clone()));

        let header_row = row![dir_label, remove_btn];

        if let Some(node) = node_opt {
            trees = trees.push(column![header_row, render_node(node, 0)]);
        } else {
            trees = trees.push(column![header_row, text("No files found")]);
        }
        trees = trees.push(Space::with_height(10));
    }

    let left_content = column![
        add_dir_row,
        Space::with_height(10),
        ext_menu,
        Space::with_height(10),
        trees
    ];

    let left_panel: Element<Message> = container::<Message, iced::Theme, iced::Renderer>(
            scrollable(left_content)
        )
        .width(Length::FillPortion(1))
        .padding(10)
        .into();

    let right_panel = right_panel(app);

    let split_row = row![
        left_panel,
        right_panel
    ]
    .width(Length::Fill)
    .height(Length::Fill);

    container::<Message, iced::Theme, iced::Renderer>(split_row)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

pub fn render_node(node: &FileNode, depth: usize) -> Element<Message> {
    let indent = "  ".repeat(depth);

    let mut content = column![];

    match node.node_type {
        NodeType::Directory => {
            let expand_symbol = if node.is_expanded { "▼" } else { "▶" };
            let dir_path = node.path.clone();

            let dir_row = row![
                text(format!("{}{} 📁 {}", indent, expand_symbol, node.name))
                    .size(14)
            ];

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
                for child in &node.children {
                    content = content.push(render_node(child, depth + 1));
                }
            }
        }
        NodeType::File => {
            let file_row = text(format!("{} 📄 {}", indent, node.name)).size(14);

            // Wrap the file row in a context menu for right-click
            let file_path = node.path.clone();
            let context_menu = ContextMenu::new(
                button(file_row),
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

 
#[cfg(test)]
mod iced_tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::{tempdir, NamedTempFile};
    use std::fs::File;
    use crate::file_tree::scan_directory;
    use crate::update;

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

    #[test]
    fn test_file_tree_app_new() {

        let root_node = create_test_tree();
        let dir = root_node.path.clone();
        let all_extensions: Vec<String> = vec!["txt", "md"].into_iter().map(|s| s.to_string()).collect();

        let temp_file = NamedTempFile::new().unwrap();
        let persist_path = temp_file.path().to_path_buf();

        let mut app = FileTreeApp::new(vec![dir], all_extensions, persist_path);
        app.root_nodes[0] = Some(root_node); // manually set the test tree

        assert!(app.root_nodes[0].is_some());
        assert_eq!(app.root_nodes[0].as_ref().unwrap().name, "root");
    }

    #[test]
    fn test_file_tree_app_new_empty() {
        let dir = PathBuf::from("/dummy");
        let all_extensions = vec![
            "txt", "md"
        ].into_iter().map(|s| s.to_string()).collect();
        let temp_file = NamedTempFile::new().unwrap();
        let persist_path = temp_file.path().to_path_buf();
        let app = FileTreeApp::new(vec![dir], all_extensions, persist_path);
        assert!(app.root_nodes[0].is_none());
    }

    #[test]
    fn test_update_toggle_expansion() {
        let root_node = create_test_tree();
        let dir = root_node.path.clone(); // Use the root node's path
        let all_extensions: Vec<String> = vec!["txt", "md"].into_iter().map(|s| s.to_string()).collect();

        // Initially not expanded
        assert!(!root_node.children[0].is_expanded);

        let temp_file = NamedTempFile::new().unwrap();
        let persist_path = temp_file.path().to_path_buf();
        let mut app = FileTreeApp::new(vec![dir], all_extensions, persist_path);
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
        let all_extensions: Vec<String> = vec!["txt", "md"].into_iter().map(|s| s.to_string()).collect();

        let temp_file = NamedTempFile::new().unwrap();
        let persist_path = temp_file.path().to_path_buf();
        let mut app = FileTreeApp::new(vec![dir], all_extensions, persist_path);
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
        let all_extensions = vec!["txt", "md"].into_iter().map(|s| s.to_string()).collect();
        let temp_file = NamedTempFile::new().unwrap();
        let persist_path = temp_file.path().to_path_buf();
        let mut app = FileTreeApp::new(vec![dir], all_extensions, persist_path);
        let message = Message::ToggleExpansion(PathBuf::from("/nonexistent"));
        
        let _task = update(&mut app, message);
        
        // Should not panic and app state should remain unchanged
        assert!(app.root_nodes[0].is_none());
    }

    #[test]
    fn test_view_with_root_node() {
        let dir = PathBuf::from("/dummy");
        let all_extensions = vec!["txt", "md"].into_iter().map(|s| s.to_string()).collect();
        let temp_file = NamedTempFile::new().unwrap();
        let persist_path = temp_file.path().to_path_buf();
        let app = FileTreeApp::new(vec![dir], all_extensions, persist_path);
        
        let _element = view(&app);
        
        // Test passes if view() doesn't panic
        // We can't easily inspect Element content without custom renderer
    }

    #[test]
    fn test_view_with_no_root_node() {
        let dir = PathBuf::from("/dummy");
        let all_extensions = vec!["txt", "md"].into_iter().map(|s| s.to_string()).collect();
        let temp_file = NamedTempFile::new().unwrap();
        let persist_path = temp_file.path().to_path_buf();
        let app = FileTreeApp::new(vec![dir], all_extensions, persist_path);
        
        let _element = view(&app);
        
        // Test passes if view() doesn't panic when rendering empty state
    }

    #[test]
    fn test_render_node_file() {
        let file_node = FileNode::new_file(
            "test.txt".to_string(),
            PathBuf::from("/test.txt")
        );
        
        let _element = render_node(&file_node, 0);
        
        // Test passes if render_node() doesn't panic
    }

    #[test]
    fn test_render_node_directory() {
        let dir_node = FileNode::new_directory(
            "testdir".to_string(),
            PathBuf::from("/testdir"),
            vec![]
        );
        
        let _element = render_node(&dir_node, 1);
        
        // Test passes if render_node() doesn't panic
    }

    #[test]
    fn test_integration_with_real_directory() {
        let dir = PathBuf::from("/dummy");
        let all_extensions = vec!["txt", "md"].into_iter().map(|s| s.to_string()).collect();
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
        let mut app = FileTreeApp::new(vec![dir], all_extensions, persist_path);
        
        // Test expanding the subdirectory
        let subdir_path = subdir.to_path_buf();
        let message = Message::ToggleExpansion(subdir_path);
        let _task = update(&mut app, message);
        
        // Test view rendering doesn't panic
        let _element = view(&app);
        
        // Test passes if all operations complete without panicking
    }

    #[test]
    fn test_deeply_nested_expansion() {
        let dir = PathBuf::from("/dummy");
        let mut root = FileNode::new_directory("root".to_string(), PathBuf::from("/root"), vec![]);
        let mut level1 = FileNode::new_directory("level1".to_string(), PathBuf::from("/root/level1"), vec![]);
        let mut level2 = FileNode::new_directory("level2".to_string(), PathBuf::from("/root/level1/level2"), vec![]);
        let all_extensions: Vec<String> = vec!["txt", "md"].into_iter().map(|s| s.to_string()).collect();

        level2.children.push(FileNode::new_file("deep.txt".to_string(), PathBuf::from("/root/level1/level2/deep.txt")));
        level1.children.push(level2);
        root.children.push(level1);

        let temp_file = NamedTempFile::new().unwrap();
        let persist_path = temp_file.path().to_path_buf();
        let mut app = FileTreeApp::new(vec![dir], all_extensions, persist_path);
        app.root_nodes[0] = Some(root); // manually set the test tree

        // Test expanding deeply nested directory
        let deep_path = PathBuf::from("/root/level1/level2");
        let message = Message::ToggleExpansion(deep_path);
        let _task = update(&mut app, message);

        // Verify the deep directory was expanded
        let level2_node = &app.root_nodes[0].as_ref().unwrap().children[0].children[0];
        assert!(level2_node.is_expanded);
    }

    #[test]
    fn test_directory_added_with_file_path() {
        use std::fs::File;
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("testfile.txt");
        File::create(&file_path).unwrap();

        let all_extensions = vec!["txt".to_string()];
        let temp_file = NamedTempFile::new().unwrap();
        let persist_path = temp_file.path().to_path_buf();
        let mut app = FileTreeApp::new(vec![], all_extensions, persist_path);

        // Simulate adding a file path
        let message = Message::DirectoryAdded(Some(file_path.clone()));
        let _ = update(&mut app, message);

        // The parent directory should be added, not the file itself
        assert!(app.top_dirs.contains(&temp_dir.path().to_path_buf()));
        assert!(!app.top_dirs.contains(&file_path));
    }

    #[test]
    fn test_toggle_extension() {
        let dir = PathBuf::from("/dummy");
        let all_extensions = vec!["txt".to_string(), "md".to_string()];
        let temp_file = NamedTempFile::new().unwrap();
        let persist_path = temp_file.path().to_path_buf();
        let mut app = FileTreeApp::new(vec![dir], all_extensions.clone(), persist_path);

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
        let all_extensions = vec!["txt".to_string()];
        let temp_file = NamedTempFile::new().unwrap();
        let persist_path = temp_file.path().to_path_buf();
        let mut app = FileTreeApp::new(vec![dir], all_extensions, persist_path);

        let msg = Message::ToggleExtensionsMenu;
        let _ = update(&mut app, msg);
        assert!(app.extensions_menu_expanded);

        let _ = update(&mut app, Message::ToggleExtensionsMenu);
        assert!(!app.extensions_menu_expanded);
    }

    #[test]
    fn test_add_to_right_panel() {
        let file_path = PathBuf::from("/file.txt");
        let mut app = FileTreeApp::new(vec![], vec!["txt".to_string()], PathBuf::from("/tmp"));
        let msg = Message::AddToRightPanel(file_path.clone());
        let _ = update(&mut app, msg);
        assert!(app.right_panel_files.contains(&file_path));
    }

    #[test]
    fn test_add_directory_to_right_panel() {
        let dir_path = PathBuf::from("/dir");
        let file1 = PathBuf::from("/dir/file1.txt");
        let file2 = PathBuf::from("/dir/file2.txt");
        let mut dir_node = FileNode::new_directory("dir".to_string(), dir_path.clone(), vec![]);
        dir_node.children.push(FileNode::new_file("file1.txt".to_string(), file1.clone()));
        dir_node.children.push(FileNode::new_file("file2.txt".to_string(), file2.clone()));

        let mut app = FileTreeApp::new(vec![dir_path.clone()], vec!["txt".to_string()], PathBuf::from("/tmp"));
        app.root_nodes[0] = Some(dir_node);

        let msg = Message::AddDirectoryToRightPanel(dir_path.clone());
        let _ = update(&mut app, msg);
        assert!(app.right_panel_files.contains(&file1));
        assert!(app.right_panel_files.contains(&file2));
    }

    #[test]
    fn test_remove_from_right_panel() {
        let file_path = PathBuf::from("/file.txt");
        let mut app = FileTreeApp::new(vec![], vec!["txt".to_string()], PathBuf::from("/tmp"));
        app.right_panel_files.push(file_path.clone());
        let msg = Message::RemoveFromRightPanel(file_path.clone());
        let _ = update(&mut app, msg);
        assert!(!app.right_panel_files.contains(&file_path));
    }

    #[test]
    fn test_remove_directory_from_right_panel() {
        let dir_path = PathBuf::from("/dir");
        let file1 = PathBuf::from("/dir/file1.txt");
        let file2 = PathBuf::from("/dir/file2.txt");
        let file3 = PathBuf::from("/other/file3.txt");
        let mut app = FileTreeApp::new(vec![], vec!["txt".to_string()], PathBuf::from("/tmp"));
        app.right_panel_files = vec![file1.clone(), file2.clone(), file3.clone()];
        let msg = Message::RemoveDirectoryFromRightPanel(dir_path.clone());
        let _ = update(&mut app, msg);
        assert!(!app.right_panel_files.contains(&file1));
        assert!(!app.right_panel_files.contains(&file2));
        assert!(app.right_panel_files.contains(&file3));
    }

    #[test]
    fn test_sort_right_panel_by_directory_and_file() {
        let file_a = PathBuf::from("/dir_a/file.txt");
        let file_b = PathBuf::from("/dir_b/file.txt");
        let mut app = FileTreeApp::new(vec![], vec!["txt".to_string()], PathBuf::from("/tmp"));
        app.right_panel_files = vec![file_b.clone(), file_a.clone()];

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
        let mut app = FileTreeApp::new(vec![], vec!["txt".to_string()], PathBuf::from("/tmp"));
        app.right_panel_files = vec![file1.clone(), file2.clone()];
        let msg = Message::ShuffleRightPanel;
        let _ = update(&mut app, msg);
        assert!(app.right_panel_shuffled);
    }

    #[test]
    fn test_add_duplicate_to_right_panel() {
        let file_path = PathBuf::from("/file.txt");
        let mut app = FileTreeApp::new(vec![], vec!["txt".to_string()], PathBuf::from("/tmp"));
        app.right_panel_files.push(file_path.clone());
        let msg = Message::AddToRightPanel(file_path.clone());
        let _ = update(&mut app, msg);
        // Should not add duplicate
        assert_eq!(app.right_panel_files.iter().filter(|p| **p == file_path).count(), 1);
    }

    #[test]
    fn test_remove_nonexistent_from_right_panel() {
        let file_path = PathBuf::from("/file.txt");
        let mut app = FileTreeApp::new(vec![], vec!["txt".to_string()], PathBuf::from("/tmp"));
        // Try to remove a file that's not present
        let msg = Message::RemoveFromRightPanel(file_path.clone());
        let _ = update(&mut app, msg);
        // Should not panic and list remains empty
        assert!(app.right_panel_files.is_empty());
    }

    #[test]
    fn test_remove_nonexistent_directory_from_right_panel() {
        let dir_path = PathBuf::from("/dir");
        let mut app = FileTreeApp::new(vec![], vec!["txt".to_string()], PathBuf::from("/tmp"));
        app.right_panel_files = vec![PathBuf::from("/other/file.txt")];
        let msg = Message::RemoveDirectoryFromRightPanel(dir_path.clone());
        let _ = update(&mut app, msg);
        // Should not remove unrelated files
        assert_eq!(app.right_panel_files.len(), 1);
    }

    #[test]
    fn test_sort_right_panel_empty_and_single() {
        let mut app = FileTreeApp::new(vec![], vec!["txt".to_string()], PathBuf::from("/tmp"));
        // Empty list
        let _ = update(&mut app, Message::SortRightPanelByDirectory);
        assert!(app.right_panel_files.is_empty());

        // Single item
        let file_path = PathBuf::from("/dir/file.txt");
        app.right_panel_files.push(file_path.clone());
        let _ = update(&mut app, Message::SortRightPanelByFile);
        assert_eq!(app.right_panel_files.len(), 1);
        assert_eq!(app.right_panel_files[0], file_path);
    }

    #[test]
    fn test_shuffle_right_panel_empty_and_single() {
        let mut app = FileTreeApp::new(vec![], vec!["txt".to_string()], PathBuf::from("/tmp"));
        // Empty list
        let _ = update(&mut app, Message::ShuffleRightPanel);
        assert!(app.right_panel_shuffled);

        // Single item
        let file_path = PathBuf::from("/dir/file.txt");
        app.right_panel_files.push(file_path.clone());
        let _ = update(&mut app, Message::ShuffleRightPanel);
        assert!(app.right_panel_shuffled);
        assert_eq!(app.right_panel_files.len(), 1);
        assert_eq!(app.right_panel_files[0], file_path);
    }

    #[test]
    fn test_sort_then_shuffle_then_sort_right_panel() {
        let file1 = PathBuf::from("/dir_a/file1.txt");
        let file2 = PathBuf::from("/dir_b/file2.txt");
        let mut app = FileTreeApp::new(vec![], vec!["txt".to_string()], PathBuf::from("/tmp"));
        app.right_panel_files = vec![file1.clone(), file2.clone()];

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
    fn test_toggle_extension_with_empty_all_extensions() {
        let dir = PathBuf::from("/dummy");
        let all_extensions: Vec<String> = vec![];
        let temp_file = NamedTempFile::new().unwrap();
        let persist_path = temp_file.path().to_path_buf();
        let mut app = FileTreeApp::new(vec![dir], all_extensions, persist_path);

        // Try toggling a non-existent extension
        let msg = Message::ToggleExtension("md".to_string());
        let _ = update(&mut app, msg);
        // Should not add "md" to selected_extensions
        assert!(!app.selected_extensions.contains(&"md".to_string()));
    }

    #[test]
    fn test_toggle_extensions_menu_with_empty_extensions() {
        let dir = PathBuf::from("/dummy");
        let all_extensions: Vec<String> = vec![];
        let temp_file = NamedTempFile::new().unwrap();
        let persist_path = temp_file.path().to_path_buf();
        let mut app = FileTreeApp::new(vec![dir], all_extensions, persist_path);

        // Toggle menu open and closed
        let msg = Message::ToggleExtensionsMenu;
        let _ = update(&mut app, msg);
        assert!(app.extensions_menu_expanded);

        let _ = update(&mut app, Message::ToggleExtensionsMenu);
        assert!(!app.extensions_menu_expanded);
    }

    #[test]
    fn test_toggle_extension_with_empty_selected_extensions() {
        let dir = PathBuf::from("/dummy");
        let all_extensions = vec!["txt".to_string()];
        let temp_file = NamedTempFile::new().unwrap();
        let persist_path = temp_file.path().to_path_buf();
        let mut app = FileTreeApp::new(vec![dir], all_extensions.clone(), persist_path);

        // Remove all selected extensions
        app.selected_extensions.clear();
        // Toggle "txt" on
        let msg = Message::ToggleExtension("txt".to_string());
        let _ = update(&mut app, msg);
        assert!(app.selected_extensions.contains(&"txt".to_string()));
    }

    #[test]
    fn test_toggle_extension_not_in_all_extensions() {
        let dir = PathBuf::from("/dummy");
        let all_extensions = vec!["txt".to_string()];
        let temp_file = NamedTempFile::new().unwrap();
        let persist_path = temp_file.path().to_path_buf();
        let mut app = FileTreeApp::new(vec![dir], all_extensions.clone(), persist_path);

        // Try toggling an extension not in all_extensions
        let msg = Message::ToggleExtension("md".to_string());
        let _ = update(&mut app, msg);
        assert!(!app.selected_extensions.contains(&"md".to_string()));
        assert!(!app.all_extensions.contains(&"md".to_string()));
    }

    #[test]
    fn test_update_with_invalid_message() {
        use crate::gui::{update, FileTreeApp, Message};
        use std::path::PathBuf;
        use tempfile::NamedTempFile;

        // Setup app with minimal state
        let dir = PathBuf::from("/dummy");
        let all_extensions = vec!["txt".to_string()];
        let temp_file = NamedTempFile::new().unwrap();
        let persist_path = temp_file.path().to_path_buf();
        let mut app = FileTreeApp::new(vec![dir], all_extensions.clone(), persist_path);

        // Clone state before update
        let prev_state = app.clone();

        // Call update with a message that should have no effect
        let _ = update(&mut app, Message::ToggleExtension("invalid_ext".to_string()));

        // Assert that state is unchanged
        assert_eq!(app.selected_extensions, prev_state.selected_extensions);
        assert_eq!(app.right_panel_shuffled, prev_state.right_panel_shuffled);
    }

    #[test]
    fn test_toggle_extension_with_empty_string() {
        use crate::gui::{update, FileTreeApp, Message};
        use std::path::PathBuf;
        use tempfile::NamedTempFile;

        let dir = PathBuf::from("/dummy");
        let all_extensions = vec!["txt".to_string()];
        let temp_file = NamedTempFile::new().unwrap();
        let persist_path = temp_file.path().to_path_buf();
        let mut app = FileTreeApp::new(vec![dir], all_extensions.clone(), persist_path);

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
        let all_extensions = vec!["txt".to_string()];
        let temp_file = NamedTempFile::new().unwrap();
        let persist_path = temp_file.path().to_path_buf();
        let mut app = FileTreeApp::new(vec![dir], all_extensions.clone(), persist_path);

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
        let all_extensions = vec!["txt".to_string()];
        let temp_file = NamedTempFile::new().unwrap();
        let persist_path = temp_file.path().to_path_buf();
        let mut app = FileTreeApp::new(vec![dir], all_extensions.clone(), persist_path);

        let msg = Message::ToggleExtension("💥".to_string());
        let _ = update(&mut app, msg);
        assert!(!app.selected_extensions.contains(&"💥".to_string()));
    }
}
