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
    let mut children = Vec::new();

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if allowed_extensions.contains(&ext) {
                        children.push(FileNode::new_file(name, path));
                    }
                }
            } else if path.is_dir() {
                if let Some(child_node) = scan_directory(&path, allowed_extensions) {
                    // Only add directory if it has matching children
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


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs::{File, create_dir};

    #[test]
    fn test_scan_directory_01() {
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
}
