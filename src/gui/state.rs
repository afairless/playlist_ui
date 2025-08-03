use std::fs;
use std::path::PathBuf;
use std::collections::HashSet;
use serde::{Serialize, Deserialize};
use crate::file_tree::{FileNode, scan_directory};

const TOP_DIRS_FILE: &str = ".playlist_ui_top_dirs.json";

fn get_persist_path() -> PathBuf {
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
    SortRightPanelByDirectory,
    SortRightPanelByFile,
    ShuffleRightPanel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SortColumn {
    Directory,
    File,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SortOrder {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTreeApp {
    #[serde(skip)]
    pub root_nodes: Vec<Option<FileNode>>,
    pub top_dirs: Vec<PathBuf>,
    #[serde(skip)]
    persist_path: PathBuf,
    #[serde(skip)]
    pub selected_extensions: Vec<String>,
    #[serde(skip)]
    pub all_extensions: Vec<String>,
    #[serde(skip)]
    pub extensions_menu_expanded: bool,
    #[serde(skip)]
    pub expanded_dirs: HashSet<PathBuf>,
    #[serde(skip)]
    pub right_panel_files: Vec<PathBuf>,
    pub right_panel_sort_column: SortColumn,
    pub right_panel_sort_order: SortOrder,
    #[serde(skip)]
    pub right_panel_shuffled: bool,
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
            right_panel_sort_column: SortColumn::Directory,
            right_panel_sort_order: SortOrder::Asc,
            right_panel_shuffled: false,
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

    pub fn persist_top_dirs(&self) {
        if let Ok(json) = serde_json::to_string(&self.top_dirs) {
            let _ = fs::write(&self.persist_path, json);
        }
    }
}

