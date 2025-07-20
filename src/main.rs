use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum NodeType {
    File,
    Directory,
}

#[derive(Debug, Clone)]
pub struct FileNode {
    pub name: String,
    pub path: PathBuf,
    pub node_type: NodeType,
    pub children: Vec<FileNode>,
    pub is_expanded: bool,
}

impl FileNode {
    pub fn new_file(name: String, path: PathBuf) -> Self {
        FileNode {
            name,
            path,
            node_type: NodeType::File,
            children: Vec::new(),
            is_expanded: false,
        }
    }

    pub fn new_directory(name: String, path: PathBuf, children: Vec<FileNode>) -> Self {
        FileNode {
            name,
            path,
            node_type: NodeType::Directory,
            children,
            is_expanded: false,
        }
    }
}

pub fn scan_directory(
    dir: &Path,
    allowed_extensions: &[&str],
) -> Option<FileNode> {
    let allowed: Vec<String> = allowed_extensions.iter().map(|e| e.to_lowercase()).collect();
    let mut children = Vec::new();

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if allowed.contains(&ext.to_lowercase()) {
                        children.push(FileNode::new_file(name, path));
                    }
                }
            } else if path.is_dir() {
                if let Some(child_node) = scan_directory(&path, allowed_extensions) {
                    if !child_node.children.is_empty() {
                        children.push(child_node);
                    }
                }
            }
        }
    }

    if !children.is_empty() {
        Some(FileNode::new_directory(
            dir.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| dir.display().to_string()),
            dir.to_path_buf(),
            children,
        ))
    } else {
        None
    }
}

use iced::{
    widget::{button, column, container, row, scrollable, text},
    Element, Length, Task,
};

#[derive(Debug, Clone)]
pub enum Message {
    ToggleExpansion(PathBuf),
}

#[derive(Debug, Clone)]
pub struct FileTreeApp {
    root_node: Option<FileNode>,
}

impl FileTreeApp {
    fn new(root_node: Option<FileNode>) -> Self {
        FileTreeApp { root_node }
    }
}

fn update(app: &mut FileTreeApp, message: Message) -> Task<Message> {
    match message {
        Message::ToggleExpansion(path) => {
            if let Some(ref mut root) = app.root_node {
                toggle_expansion_recursive(root, &path);
            }
            Task::none()
        }
    }
}

fn view(app: &FileTreeApp) -> Element<Message> {
    let content = if let Some(ref root) = app.root_node {
        render_node(root, 0)
    } else {
        column![text("No files found")].into()
    };

    container(scrollable(content))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(10)
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

fn main() -> iced::Result {
    let home_dir = std::env::var("HOME").expect("HOME environment variable not set");
    let dir = Path::new(&home_dir).join("Documents").join("ma_timing");
    let allowed = ["txt", "rs", "md"];
    
    let root_node = scan_directory(&dir, &allowed);
    
    iced::application("File Tree Viewer", update, view)
        .run_with(|| (FileTreeApp::new(root_node), Task::none()))
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
