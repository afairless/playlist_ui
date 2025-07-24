use std::fs;
use std::path::PathBuf;
use std::collections::HashSet;
use serde::{Serialize, Deserialize};
use crate::file_tree::{FileNode, NodeType, scan_directory};
use iced::{
    widget::{button, column, container, row, scrollable, text, Space},
    Element, Length, Task,
};
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

    let mut right_trees = column![];
    let mut has_nodes = false;
    for root in app.root_nodes.iter().flatten() {
        right_trees = right_trees.push(render_node(root, 0));
        right_trees = right_trees.push(Space::with_height(10));
        has_nodes = true;
    }
    let right_content: Element<Message> = if has_nodes {
        right_trees.into()
    } else {
        column![text("No files found")].into()
    };

    let left_panel: Element<Message> = container::<Message, iced::Theme, iced::Renderer>(
            scrollable(left_content)
        )
        .width(Length::FillPortion(1))
        .padding(10)
        .into();

    let right_panel: Element<Message> = container::<Message, iced::Theme, iced::Renderer>(
            scrollable(right_content)
        )
        .width(Length::FillPortion(1))
        .padding(10)
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
        .into()
}

fn render_node(node: &FileNode, depth: usize) -> Element<Message> {
    let indent = "  ".repeat(depth);
    
    let mut content = column![];
    
    match node.node_type {
        NodeType::Directory => {
            let expand_symbol = if node.is_expanded { "▼" } else { "▶" };
            let dir_row = row![
                text(format!("{}{} 📁 {}", indent, expand_symbol, node.name))
                    .size(14)
            ];
            
            let dir_button = button(dir_row)
                .on_press(Message::ToggleExpansion(node.path.clone()));
            
            content = content.push(dir_button);
            
            if node.is_expanded {
                for child in &node.children {
                    content = content.push(render_node(child, depth + 1));
                }
            }
        }
        NodeType::File => {
            let file_row = text(format!("{} 📄 {}", indent, node.name))
                .size(14);
            content = content.push(file_row);
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
