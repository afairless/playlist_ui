use std::fs;
use std::path::PathBuf;
use std::collections::HashSet;
use serde::{Serialize, Deserialize};
use crate::file_tree::{FileNode, NodeType, scan_directory};
use iced::{
    widget::{button, column, container, row, scrollable, text, Space},
    Element, Length, Task,
};
use iced_aw::widgets::ContextMenu;
use rfd::FileDialog;

const TOP_DIRS_FILE: &str = ".playlist_ui_top_dirs.json";

pub fn get_persist_path() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(TOP_DIRS_FILE)
}

#[derive(Debug, Clone)]
pub enum Message {
    ToggleExpansion(PathBuf),
    ToggleExtension(String),
    ToggleExtensionsMenu,
    RemoveTopDir(PathBuf),
    AddDirectory,
    DirectoryAdded(Option<std::path::PathBuf>),
    AddToRightPanel(PathBuf),
    AddDirectoryToRightPanel(PathBuf),
    RemoveFromRightPanel(PathBuf),
    RemoveDirectoryFromRightPanel(PathBuf),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTreeApp {
    #[serde(skip)]
    root_nodes: Vec<Option<FileNode>>,
    top_dirs: Vec<PathBuf>,
    #[serde(skip)]
    persist_path: PathBuf,
    #[serde(skip)]
    selected_extensions: Vec<String>,
    #[serde(skip)]
    all_extensions: Vec<String>,
    #[serde(skip)]
    extensions_menu_expanded: bool,
    #[serde(skip)]
    expanded_dirs: HashSet<PathBuf>,
    #[serde(skip)]
    right_panel_files: Vec<PathBuf>,
}

impl FileTreeApp {
    pub fn new(top_dirs: Vec<PathBuf>, all_extensions: Vec<String>, persist_path: PathBuf) -> Self {
        let root_nodes: Vec<Option<FileNode>> = top_dirs.iter()
            .map(|dir| scan_directory(dir, &all_extensions.iter().map(|s| s.as_str()).collect::<Vec<_>>()))
            .collect();
        let mut expanded_dirs = HashSet::new();
        for n in root_nodes.iter().flatten() {
            expanded_dirs.insert(n.path.clone());
        }
        FileTreeApp {
            root_nodes,
            top_dirs,
            persist_path,
            selected_extensions: all_extensions.clone(),
            all_extensions,
            extensions_menu_expanded: false,
            expanded_dirs,
            right_panel_files: Vec::new(),
        }
    }
    pub fn load(all_extensions: Vec<String>) -> Self {
        let persist_path = get_persist_path();
        let top_dirs = if persist_path.exists() {
            fs::read_to_string(&persist_path)
                .ok()
                .and_then(|s| serde_json::from_str::<Vec<PathBuf>>(&s).ok())
                .unwrap_or_default()
                .into_iter()
                .filter(|p| p.exists() && p.is_dir())
                .collect()
        } else {
            Vec::new()
        };
        FileTreeApp::new(top_dirs, all_extensions, persist_path)
    }

    fn persist_top_dirs(&self) {
        if let Ok(json) = serde_json::to_string(&self.top_dirs) {
            let _ = fs::write(&self.persist_path, json);
        }
    }
}

fn restore_expansion_state(node: &mut FileNode, expanded_dirs: &HashSet<PathBuf>) {
    node.is_expanded = expanded_dirs.contains(&node.path);
    for child in &mut node.children {
        restore_expansion_state(child, expanded_dirs);
    }
}

fn collect_files_recursively(node: &FileNode, files: &mut Vec<PathBuf>) {
    match node.node_type {
        NodeType::File => files.push(node.path.clone()),
        NodeType::Directory => {
            for child in &node.children {
                collect_files_recursively(child, files);
            }
        }
    }
}

fn find_node_by_path<'a>(node: &'a FileNode, path: &PathBuf) -> Option<&'a FileNode> {
    if &node.path == path {
        return Some(node);
    }
    for child in &node.children {
        if let Some(found) = find_node_by_path(child, path) {
            return Some(found);
        }
    }
    None
}

pub fn update(app: &mut FileTreeApp, message: Message) -> Task<Message> {
    match message {
        Message::ToggleExpansion(path) => {
            if app.expanded_dirs.contains(&path) {
                app.expanded_dirs.remove(&path);
            } else {
                app.expanded_dirs.insert(path.clone());
            }
            for root in app.root_nodes.iter_mut().flatten() {
                restore_expansion_state(root, &app.expanded_dirs);
            }
            Task::none()
        }
        Message::ToggleExtension(ext) => {
            if app.selected_extensions.contains(&ext) {
                app.selected_extensions.retain(|e| e != &ext);
            } else {
                app.selected_extensions.push(ext.clone());
            }
            app.root_nodes = app.top_dirs.iter()
                .map(|dir| scan_directory(
                    dir,
                    &app.selected_extensions.iter().map(|s| s.as_str()).collect::<Vec<_>>()
                ))
                .collect();

            for root in app.root_nodes.iter_mut().flatten() {
                restore_expansion_state(root, &app.expanded_dirs);
            }
            Task::none()
        }
        Message::ToggleExtensionsMenu => {
            app.extensions_menu_expanded = !app.extensions_menu_expanded;
            Task::none()
        }
        Message::RemoveTopDir(dir) => {
            if let Some(idx) = app.top_dirs.iter().position(|d| d == &dir) {
                app.top_dirs.remove(idx);
                app.root_nodes.remove(idx);
                app.persist_top_dirs();
            }
            Task::none()
        }
        Message::AddDirectory => {
            Task::perform(
                async move { FileDialog::new().pick_folder() },
                Message::DirectoryAdded,
            )
        }
        Message::DirectoryAdded(Some(mut path)) => {
            // If the added path is a file, use its parent directory
            if path.is_file() {
                if let Some(parent) = path.parent() {
                    path = parent.to_path_buf();
                }
            }
            if !app.top_dirs.contains(&path) && path.exists() && path.is_dir() {
                app.top_dirs.push(path.clone());
                app.root_nodes.push(scan_directory(
                    &path,
                    &app.selected_extensions.iter().map(|s| s.as_str()).collect::<Vec<_>>()
                ));
                app.persist_top_dirs();
            }
            Task::none()
        }
        Message::DirectoryAdded(None) => Task::none(),
        Message::AddToRightPanel(path) => {
            if !app.right_panel_files.contains(&path) {
                app.right_panel_files.push(path);
            }
            Task::none()
        }
        Message::AddDirectoryToRightPanel(dir_path) => {
            for root in app.root_nodes.iter().flatten() {
                if let Some(node) = find_node_by_path(root, &dir_path) {
                    let mut files = Vec::new();
                    collect_files_recursively(node, &mut files);
                    for file in files {
                        if !app.right_panel_files.contains(&file) {
                            app.right_panel_files.push(file);
                        }
                    }
                }
            }
            Task::none()
        }
        Message::RemoveFromRightPanel(path) => {
            app.right_panel_files.retain(|p| p != &path);
            Task::none()
        }
        Message::RemoveDirectoryFromRightPanel(dir_path) => {
            app.right_panel_files.retain(|file| {
                // Remove if file is not in dir_path or its subdirectories
                !file.starts_with(&dir_path)
            });
            Task::none()
        }
    }
}

fn extension_menu(app: &FileTreeApp) -> Element<Message> {
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

fn right_panel(app: &FileTreeApp) -> iced::Element<Message> {
    let mut col = iced::widget::Column::new();

    // Header row
    let header_row = iced::widget::Row::new()
        .push(
            iced::widget::text("Directory")
                .width(Length::FillPortion(1))
                .size(24)
                .style(|_theme| iced::widget::text::Style {
                    color: Some([0.5, 0.5, 0.5, 1.0].into()),
                })
        )
        .push(
            iced::widget::text("File")
                .width(Length::FillPortion(1))
                .size(24)
                .style(|_theme| iced::widget::text::Style {
                    color: Some([0.5, 0.5, 0.5, 1.0].into()),
                })
        );
    col = col.push(header_row);

    for file in &app.right_panel_files {
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

fn render_node(node: &FileNode, depth: usize) -> Element<Message> {
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
    use tempfile::{tempdir, NamedTempFile};
    use std::fs::File;

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
}
