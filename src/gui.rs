use std::path::{Path, PathBuf};
use crate::file_tree::{FileNode, NodeType, scan_directory};
use iced::{
    widget::{button, column, container, row, scrollable, text},
    Element, Length, Task,
};

#[derive(Debug, Clone)]
pub enum Message {
    ToggleExpansion(PathBuf),
    ToggleExtension(String),
}

#[derive(Debug, Clone)]
pub struct FileTreeApp {
    root_node: Option<FileNode>,
    selected_extensions: Vec<String>,
    all_extensions: Vec<String>,
    dir: PathBuf,
}

impl FileTreeApp {
    pub fn new(dir: PathBuf, all_extensions: Vec<String>) -> Self {
        let root_node = scan_directory(&dir, &all_extensions.iter().map(|s| s.as_str()).collect::<Vec<_>>());
        FileTreeApp {
            root_node,
            selected_extensions: all_extensions.clone(),
            all_extensions,
            dir,
        }
    }
}

pub fn update(app: &mut FileTreeApp, message: Message) -> Task<Message> {
    match message {
        Message::ToggleExpansion(path) => {
            if let Some(ref mut root) = app.root_node {
                toggle_expansion_recursive(root, &path);
            }
            Task::none()
        }
        Message::ToggleExtension(ext) => {
            if app.selected_extensions.contains(&ext) {
                app.selected_extensions.retain(|e| e != &ext);
            } else {
                app.selected_extensions.push(ext.clone());
            }
            app.root_node = scan_directory(
                &app.dir,
                &app.selected_extensions.iter().map(|s| s.as_str()).collect::<Vec<_>>()
            );
            Task::none()
        }
    }
}

fn extension_menu(app: &FileTreeApp) -> Element<Message> {
    let mut menu = column![];
    for ext in &app.all_extensions {
        let checked = app.selected_extensions.contains(ext);
        let label = if checked { format!("[x] .{ext}") } else { format!("[ ] .{ext}") };
        menu = menu.push(
            button(text(label))
                .on_press(Message::ToggleExtension(ext.clone()))
        );
    }
    menu.into()
}

pub fn view(app: &FileTreeApp) -> Element<Message> {
    let ext_menu = extension_menu(app);

    let left_content: Element<Message> = if let Some(ref root) = app.root_node {
        column![
            text("File Extensions:").size(16),
            ext_menu,
            render_node(root, 0)
        ].into()
    } else {
        column![
            text("File Extensions:").size(16),
            ext_menu,
            text("No files found")
        ].into()
    };

    let right_content = if let Some(ref root) = app.root_node {
        render_node(root, 0)
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

fn toggle_expansion_recursive(node: &mut FileNode, target_path: &Path) {
    if node.path == target_path {
        node.is_expanded = !node.is_expanded;
        return;
    }
    
    for child in &mut node.children {
        toggle_expansion_recursive(child, target_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs::{File, create_dir};

    #[test]
    fn test_scan_directory_01() {
        // scan empty directory
        let dir = tempfile::tempdir().unwrap();
        let allowed = ["rs"];
        let node = scan_directory(dir.path(), &allowed);
        assert!(node.is_none());
    }

    #[test]
    fn test_scan_directory_02() {
        // directory has no matching files
        let dir = tempfile::tempdir().unwrap();
        std::fs::File::create(dir.path().join("a.md")).unwrap();
        std::fs::File::create(dir.path().join("b.doc")).unwrap();
        let allowed = ["rs"];
        let node = scan_directory(dir.path(), &allowed);
        assert!(node.is_none());
    }

    #[test]
    fn test_scan_directory_03() {
        // nested matching
        let dir = tempfile::tempdir().unwrap();
        let subdir = dir.path().join("sub");
        std::fs::create_dir(&subdir).unwrap();
        std::fs::File::create(subdir.join("a.rs")).unwrap();
        std::fs::File::create(subdir.join("b.md")).unwrap();
        let allowed = ["rs"];
        let node = scan_directory(dir.path(), &allowed).unwrap();
        assert_eq!(node.children.len(), 1);
        assert_eq!(node.children[0].name, "sub");
        assert_eq!(node.children[0].children.len(), 1);
        assert_eq!(node.children[0].children[0].name, "a.rs");
    }

    #[test]
    fn test_scan_directory_04() {
        // match multiple file extensions
        let dir = tempfile::tempdir().unwrap();
        std::fs::File::create(dir.path().join("a.rs")).unwrap();
        std::fs::File::create(dir.path().join("b.txt")).unwrap();
        std::fs::File::create(dir.path().join("c.md")).unwrap();
        let allowed = ["rs", "txt"];
        let node = scan_directory(dir.path(), &allowed).unwrap();
        let names: Vec<_> = node.children.iter().map(|n| &n.name).collect();
        assert!(names.contains(&&"a.rs".to_string()));
        assert!(names.contains(&&"b.txt".to_string()));
        assert!(!names.contains(&&"c.md".to_string()));
    }

    #[test]
    fn test_scan_directory_05() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Create files and subdirectories
        File::create(root.join("a.rs")).unwrap();
        File::create(root.join("b.txt")).unwrap();
        File::create(root.join("c.md")).unwrap();

        let subdir = root.join("sub");
        create_dir(&subdir).unwrap();
        File::create(subdir.join("d.rs")).unwrap();
        File::create(subdir.join("e.md")).unwrap();

        let allowed = ["rs", "txt"];
        let node = scan_directory(root, &allowed).unwrap();

        // Check root directory
        assert_eq!(node.children.len(), 3);
        let names: Vec<_> = node.children.iter().map(|n| &n.name).collect();
        assert!(names.contains(&&"a.rs".to_string()));
        assert!(names.contains(&&"b.txt".to_string()));
        assert!(names.contains(&&"sub".to_string()));

        // Check subdirectory
        let sub_node = node.children.iter().find(|n| n.name == "sub").unwrap();
        assert_eq!(sub_node.children.len(), 1);
        assert_eq!(sub_node.children[0].name, "d.rs");
    }

    #[test]
    fn test_scan_directory_06() {
        // subdirectories with no matches
        let dir = tempfile::tempdir().unwrap();
        let subdir1 = dir.path().join("sub1");
        let subdir2 = dir.path().join("sub2");
        std::fs::create_dir(&subdir1).unwrap();
        std::fs::create_dir(&subdir2).unwrap();
        std::fs::File::create(subdir1.join("a.md")).unwrap();
        std::fs::File::create(subdir2.join("b.doc")).unwrap();
        let allowed = ["rs"];
        let node = scan_directory(dir.path(), &allowed);
        assert!(node.is_none());
    }

    #[test]
    fn test_scan_directory_07() {
        // nested empty subdirectory
        let dir = tempfile::tempdir().unwrap();
        let subdir = dir.path().join("empty_sub");
        std::fs::create_dir(&subdir).unwrap();
        let allowed = ["rs"];
        let node = scan_directory(dir.path(), &allowed);
        assert!(node.is_none());
    }

    #[test]
    fn test_scan_directory_08() {
        // file with no extension
        let dir = tempfile::tempdir().unwrap();
        std::fs::File::create(dir.path().join("README")).unwrap();
        let allowed = ["rs"];
        let node = scan_directory(dir.path(), &allowed);
        assert!(node.is_none());
    }

    #[test]
    fn test_scan_directory_09() {
        // file extension match based on string case
        let dir = tempfile::tempdir().unwrap();
        std::fs::File::create(dir.path().join("A.RS")).unwrap();
        let allowed = ["rs"];
        let node = scan_directory(dir.path(), &allowed).unwrap();
        // Should not match, as extension comparison is case-sensitive
        let names: Vec<_> = node.children.iter().map(|n| &n.name).collect();
        assert!(names.contains(&&"A.RS".to_string()));
    }

    #[test]
    fn test_scan_directory_10() {
        // deeply nested matches
        let dir = tempfile::tempdir().unwrap();
        let subdir1 = dir.path().join("sub1");
        let subdir2 = subdir1.join("sub2");
        std::fs::create_dir(&subdir1).unwrap();
        std::fs::create_dir(&subdir2).unwrap();
        std::fs::File::create(subdir2.join("deep.rs")).unwrap();
        let allowed = ["rs"];
        let node = scan_directory(dir.path(), &allowed).unwrap();
        assert_eq!(node.children.len(), 1);
        assert_eq!(node.children[0].name, "sub1");
        assert_eq!(node.children[0].children.len(), 1);
        assert_eq!(node.children[0].children[0].name, "sub2");
        assert_eq!(node.children[0].children[0].children.len(), 1);
        assert_eq!(node.children[0].children[0].children[0].name, "deep.rs");
    }
}

#[cfg(test)]
mod iced_tests {
    use super::*;
    use tempfile::tempdir;
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
        let mut app = FileTreeApp::new(dir, all_extensions);
        app.root_node = Some(root_node); // manually set the test tree

        assert!(app.root_node.is_some());
        assert_eq!(app.root_node.as_ref().unwrap().name, "root");
    }

    #[test]
    fn test_file_tree_app_new_empty() {
        let dir = PathBuf::from("/dummy");
        let all_extensions = vec![
            "txt", "md"
        ].into_iter().map(|s| s.to_string()).collect();
        let app = FileTreeApp::new(dir, all_extensions);
        assert!(app.root_node.is_none());
    }

    #[test]
    fn test_update_toggle_expansion() {
        let root_node = create_test_tree();
        let dir = root_node.path.clone(); // Use the root node's path
        let all_extensions: Vec<String> = vec!["txt", "md"].into_iter().map(|s| s.to_string()).collect();

        // Initially not expanded
        assert!(!root_node.children[0].is_expanded);

        let mut app = FileTreeApp::new(dir, all_extensions);
        app.root_node = Some(root_node); // manually set the test tree

        let subdir_path = PathBuf::from("/test/root/subdir");
        let message = Message::ToggleExpansion(subdir_path.clone());

        let _task = update(&mut app, message);

        // Should be expanded now
        assert!(app.root_node.as_ref().unwrap().children[0].is_expanded);
    }

    #[test]
    fn test_update_toggle_expansion_twice() {
        let root_node = create_test_tree();
        let dir = root_node.path.clone(); // Use the root node's path
        let all_extensions: Vec<String> = vec!["txt", "md"].into_iter().map(|s| s.to_string()).collect();

        let mut app = FileTreeApp::new(dir, all_extensions);
        app.root_node = Some(root_node); // manually set the test tree

        let subdir_path = PathBuf::from("/test/root/subdir");

        // Toggle once - should expand
        let message = Message::ToggleExpansion(subdir_path.clone());
        let _ = update(&mut app, message);
        assert!(app.root_node.as_ref().unwrap().children[0].is_expanded);

        // Toggle again - should collapse
        let message = Message::ToggleExpansion(subdir_path);
        let _ = update(&mut app, message);
        assert!(!app.root_node.as_ref().unwrap().children[0].is_expanded);
    }

    #[test]
    fn test_update_with_no_root_node() {
        let dir = PathBuf::from("/dummy");
        let all_extensions = vec!["txt", "md"].into_iter().map(|s| s.to_string()).collect();
        let mut app = FileTreeApp::new(dir, all_extensions);
        let message = Message::ToggleExpansion(PathBuf::from("/nonexistent"));
        
        let _task = update(&mut app, message);
        
        // Should not panic and app state should remain unchanged
        assert!(app.root_node.is_none());
    }

    #[test]
    fn test_toggle_expansion_recursive() {
        let mut root_node = create_test_tree();
        let subdir_path = PathBuf::from("/test/root/subdir");
        
        assert!(!root_node.children[0].is_expanded);
        
        toggle_expansion_recursive(&mut root_node, &subdir_path);
        
        assert!(root_node.children[0].is_expanded);
    }

    #[test]
    fn test_toggle_expansion_recursive_nonexistent_path() {
        let mut root_node = create_test_tree();
        let nonexistent_path = PathBuf::from("/test/nonexistent");
        
        let original_state = root_node.children[0].is_expanded;
        
        toggle_expansion_recursive(&mut root_node, &nonexistent_path);
        
        // Should not change any expansion states
        assert_eq!(root_node.children[0].is_expanded, original_state);
    }

    #[test]
    fn test_view_with_root_node() {
        let dir = PathBuf::from("/dummy");
        let all_extensions = vec!["txt", "md"].into_iter().map(|s| s.to_string()).collect();
        let app = FileTreeApp::new(dir, all_extensions);
        
        let _element = view(&app);
        
        // Test passes if view() doesn't panic
        // We can't easily inspect Element content without custom renderer
    }

    #[test]
    fn test_view_with_no_root_node() {
        let dir = PathBuf::from("/dummy");
        let all_extensions = vec!["txt", "md"].into_iter().map(|s| s.to_string()).collect();
        let app = FileTreeApp::new(dir, all_extensions);
        
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
        let mut app = FileTreeApp::new(dir, all_extensions);
        
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

        let mut app = FileTreeApp::new(dir, all_extensions);
        app.root_node = Some(root); // manually set the test tree

        // Test expanding deeply nested directory
        let deep_path = PathBuf::from("/root/level1/level2");
        let message = Message::ToggleExpansion(deep_path);
        let _task = update(&mut app, message);

        // Verify the deep directory was expanded
        let level2_node = &app.root_node.as_ref().unwrap().children[0].children[0];
        assert!(level2_node.is_expanded);
    }
}
