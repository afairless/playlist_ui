//! File-tree construction for the Playlist UI.
//!
//! Recursively scans directories for audio files matching a set of
//! allowed extensions, building a `FileNode` tree that mirrors the
//! filesystem layout. Each node carries a pre-computed file count for
//! display and highlight purposes.
//!
//! Public API:
//!     FileNode        — tree node (file or directory)
//!     NodeType        — file / directory discriminant
//!     scan_directory  — build a FileNode tree from a directory path

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum NodeType {
    File,
    Directory,
}

#[derive(Debug, Clone)]
pub(crate) struct FileNode {
    pub name: String,
    pub path: PathBuf,
    pub node_type: NodeType,
    pub children: Vec<FileNode>,
    pub is_expanded: bool,
    pub file_count: usize,
}

impl FileNode {
    /// Creates a new `FileNode` representing a file with the given name and
    /// path.
    /// The node type is set to `File`, and it has no children. The node is not
    /// expanded by default.
    pub(crate) fn new_file(name: String, path: PathBuf) -> Self {
        FileNode {
            name,
            path,
            node_type: NodeType::File,
            children: Vec::new(),
            is_expanded: false,
            file_count: 1,
        }
    }
    /// Creates a new `FileNode` representing a directory with the given name,
    /// path, and children. The node type is set to `Directory`. The node is not
    /// expanded by default.
    pub(crate) fn new_directory(
        name: String,
        path: PathBuf,
        children: Vec<FileNode>,
    ) -> Self {
        let file_count = children.iter().map(|c| c.file_count).sum();
        FileNode {
            name,
            path,
            node_type: NodeType::Directory,
            children,
            is_expanded: false,
            file_count,
        }
    }
}

/// Recursively scans a directory for files matching the allowed extensions,
/// building a tree of `FileNode` objects. Only files whose extensions are
/// present in `allowed_extensions` are included. The root node is marked as
/// expanded by default.
pub(crate) fn scan_directory(
    dir: &Path,
    allowed_extensions: &[&str],
) -> Option<FileNode> {
    scan_directory_with_expansion(dir, allowed_extensions, true)
}

/// Recursively scans a directory for files matching the allowed extensions,
/// building a tree of `FileNode` objects and marking the root node as expanded
/// if specified.
fn scan_directory_with_expansion(
    dir: &Path,
    allowed_extensions: &[&str],
    is_root: bool,
) -> Option<FileNode> {
    let allowed: Vec<String> =
        allowed_extensions.iter().map(|e| e.to_lowercase()).collect();
    let mut children = Vec::new();

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str())
                    && allowed.contains(&ext.to_lowercase())
                {
                    children.push(FileNode::new_file(name, path));
                }
            } else if path.is_dir()
                && let Some(child_node) = scan_directory_with_expansion(
                    &path,
                    allowed_extensions,
                    false,
                )
                && !child_node.children.is_empty()
            {
                children.push(child_node);
            }
        }
    }

    if !children.is_empty() {
        let mut node = FileNode::new_directory(
            dir.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| dir.display().to_string()),
            dir.to_path_buf(),
            children,
        );

        // Only expand the root directory by default
        node.is_expanded = is_root;

        Some(node)
    } else {
        None
    }
}

#[cfg(test)]
mod file_tree_tests {
    use super::*;
    use std::fs::{File, create_dir};
    use tempfile::tempdir;

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

    #[test]
    fn file_count_none() {
        // Empty directory with no children should report 0
        let root = FileNode::new_directory(
            "empty".to_string(),
            PathBuf::from("/empty"),
            vec![],
        );
        assert_eq!(root.file_count, 0);
    }

    #[test]
    fn file_count_one() {
        // Single-file directory should report 1
        let file =
            FileNode::new_file("a.rs".to_string(), PathBuf::from("/dir/a.rs"));
        assert_eq!(file.file_count, 1);
        let root = FileNode::new_directory(
            "dir".to_string(),
            PathBuf::from("/dir"),
            vec![file],
        );
        assert_eq!(root.file_count, 1);
    }

    #[test]
    fn file_count_many() {
        // Multi-level nested directories with multiple leaf files
        // Structure: root/ (3 files + sub/ with 2 files + empty/ with 0 files)
        let file1 =
            FileNode::new_file("a.rs".to_string(), PathBuf::from("/root/a.rs"));
        let file2 =
            FileNode::new_file("b.rs".to_string(), PathBuf::from("/root/b.rs"));
        let file3 =
            FileNode::new_file("c.rs".to_string(), PathBuf::from("/root/c.rs"));
        let file4 = FileNode::new_file(
            "d.rs".to_string(),
            PathBuf::from("/root/sub/d.rs"),
        );
        let file5 = FileNode::new_file(
            "e.rs".to_string(),
            PathBuf::from("/root/sub/e.rs"),
        );

        let sub = FileNode::new_directory(
            "sub".to_string(),
            PathBuf::from("/root/sub"),
            vec![file4, file5],
        );
        let empty = FileNode::new_directory(
            "empty".to_string(),
            PathBuf::from("/root/empty"),
            vec![],
        );

        let root = FileNode::new_directory(
            "root".to_string(),
            PathBuf::from("/root"),
            vec![file1, file2, file3, sub, empty],
        );
        // Verify counts by walking the tree
        assert_eq!(root.file_count, 5); // 3 direct + 2 in sub + 0 in empty
        // Find the "sub" child
        let sub_node = root.children.iter().find(|c| c.name == "sub").unwrap();
        assert_eq!(sub_node.file_count, 2);
        let empty_node =
            root.children.iter().find(|c| c.name == "empty").unwrap();
        assert_eq!(empty_node.file_count, 0);
    }
}
