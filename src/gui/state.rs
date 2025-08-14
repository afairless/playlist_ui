use crate::db::sled_store::SledStore;
use crate::fs::file_tree::{FileNode, scan_directory};
use crate::gui::update::restore_expansion_state;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

const TOP_DIRS_FILE: &str = ".playlist_ui_top_dirs.json";

fn get_persist_path() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(TOP_DIRS_FILE)
}

#[derive(Debug, Clone)]
pub enum Message {
    ToggleLeftPanelSelectMode,
    ToggleLeftPanel,
    ToggleTagExpansion(Vec<String>),
    AddTagNodeToRightPanel(Vec<String>),
    ToggleExpansion(PathBuf),
    ToggleExtension(String),
    ToggleExtensionsMenu,
    ToggleLeftPanelSortMode,
    RemoveTopDir(PathBuf),
    AddDirectory,
    DirectoryAdded(Option<std::path::PathBuf>),
    AddToRightPanel(PathBuf),
    AddDirectoryToRightPanel(PathBuf),
    RemoveFromRightPanel(PathBuf),
    RemoveDirectoryFromRightPanel(PathBuf),
    SortRightPanelByDirectory,
    SortRightPanelByFile,
    SortRightPanelByCreator,
    SortRightPanelByAlbum,
    SortRightPanelByTitle,
    SortRightPanelByGenre,
    SortRightPanelByDuration,
    ShuffleRightPanel,
    ExportRightPanelAsXspf,
    ExportRightPanelAsXspfTo(PathBuf),
    ExportAndPlayRightPanelAsXspf,
    OpenRightPanelFile(PathBuf),
}

#[derive(Default, Debug, Clone, PartialEq)]
pub enum LeftPanelSelectMode {
    #[default]
    Directory,
    GenreTag,
    CreatorTag,
}

#[derive(
    Debug,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    bincode::Encode,
    bincode::Decode,
    PartialEq,
)]
pub struct TagTreeNode {
    pub label: String,
    pub children: Vec<TagTreeNode>,
    pub file_paths: Vec<std::path::PathBuf>,
    pub is_expanded: bool,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftPanelSortMode {
    #[default]
    Alphanumeric,
    ModifiedDate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SortColumn {
    Directory,
    File,
    Creator,
    Album,
    Title,
    Genre,
    Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SortOrder {
    Asc,
    Desc,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RightPanelFile {
    pub path: PathBuf,
    pub creator: Option<String>,
    pub album: Option<String>,
    pub title: Option<String>,
    pub genre: Option<String>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTreeApp {
    #[serde(skip)]
    pub sled_store: Option<SledStore>,
    #[serde(skip)]
    pub left_panel_selection_mode: LeftPanelSelectMode,
    #[serde(skip)]
    pub tag_tree_roots: Vec<TagTreeNode>,
    #[serde(skip)]
    pub left_panel_expanded: bool,
    #[serde(skip)]
    pub left_panel_sort_mode: LeftPanelSortMode,
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
    pub right_panel_files: Vec<RightPanelFile>,
    pub right_panel_sort_column: SortColumn,
    pub right_panel_sort_order: SortOrder,
    #[serde(skip)]
    pub right_panel_shuffled: bool,
}

impl FileTreeApp {
    /// Creates a new `FileTreeApp` instance with the given top-level
    /// directories, file extensions, audio extensions, and persistence path.
    /// Initializes the file tree, expansion state, and right panel state.
    pub(crate) fn new(
        top_dirs: Vec<PathBuf>,
        all_extensions: &[&str],
        persist_path: PathBuf,
        sled_store: Option<SledStore>,
    ) -> Self {
        let all_extensions_vec: Vec<String> =
            all_extensions.iter().map(|s| s.to_string()).collect();

        let mut root_nodes: Vec<Option<FileNode>> = top_dirs
            .iter()
            .map(|dir| {
                scan_directory(
                    dir,
                    &all_extensions_vec
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        let mut expanded_dirs = HashSet::new();
        if top_dirs.len() == 1 {
            if let Some(Some(node)) = root_nodes.first() {
                expanded_dirs.insert(node.path.clone());
            }
        }
        for root in root_nodes.iter_mut().flatten() {
            restore_expansion_state(root, &expanded_dirs);
        }
        FileTreeApp {
            sled_store,
            left_panel_selection_mode: LeftPanelSelectMode::Directory,
            tag_tree_roots: Vec::new(),
            left_panel_expanded: true,
            left_panel_sort_mode: LeftPanelSortMode::Alphanumeric,
            root_nodes,
            top_dirs,
            persist_path,
            selected_extensions: all_extensions_vec.clone(),
            all_extensions: all_extensions_vec,
            extensions_menu_expanded: false,
            expanded_dirs,
            right_panel_files: Vec::new(),
            right_panel_sort_column: SortColumn::Directory,
            right_panel_sort_order: SortOrder::Asc,
            right_panel_shuffled: false,
        }
    }

    /// Loads a `FileTreeApp` instance from persisted state, restoring top-level
    /// directories from disk if available, and initializing with the provided
    /// file and audio extensions.
    pub(crate) fn load(
        all_extensions: &[&str],
        persist_path: Option<PathBuf>,
        sled_store: Option<SledStore>,
    ) -> Self {
        let persist_path = persist_path.unwrap_or_else(get_persist_path);
        let top_dirs = if persist_path.exists() {
            std::fs::read_to_string(&persist_path)
                .ok()
                .and_then(|s| serde_json::from_str::<Vec<PathBuf>>(&s).ok())
                .unwrap_or_default()
                .into_iter()
                .filter(|p| p.exists() && p.is_dir())
                .collect()
        } else {
            Vec::new()
        };
        FileTreeApp::new(top_dirs, all_extensions, persist_path, sled_store)
    }

    /// Persists the current list of top-level directories to disk as JSON,
    ///     using the application's configured persistence path.
    pub(crate) fn persist_top_dirs(
        &self,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string(&self.top_dirs)?;
        fs::write(&self.persist_path, json)?;
        Ok(())
    }

    /// Returns a sorted vector of files currently in the right panel, using the
    ///     configured sort column and order, unless the panel is marked as
    ///     shuffled.
    pub(crate) fn sorted_right_panel_files(&self) -> Vec<RightPanelFile> {
        let mut files = self.right_panel_files.clone();
        if !self.right_panel_shuffled {
            files.sort_by(|a, b| match self.right_panel_sort_column {
                SortColumn::Directory => {
                    let a_dir = a
                        .path
                        .parent()
                        .and_then(|p| p.file_name())
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_ascii_lowercase();
                    let b_dir = b
                        .path
                        .parent()
                        .and_then(|p| p.file_name())
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_ascii_lowercase();
                    if self.right_panel_sort_order == SortOrder::Asc {
                        a_dir.cmp(&b_dir)
                    } else {
                        b_dir.cmp(&a_dir)
                    }
                },
                SortColumn::File => {
                    let a_file = a
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_ascii_lowercase();
                    let b_file = b
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_ascii_lowercase();
                    if self.right_panel_sort_order == SortOrder::Asc {
                        a_file.cmp(&b_file)
                    } else {
                        b_file.cmp(&a_file)
                    }
                },
                SortColumn::Creator => {
                    let a_creator = a
                        .creator
                        .as_deref()
                        .unwrap_or_default()
                        .to_ascii_lowercase();
                    let b_creator = b
                        .creator
                        .as_deref()
                        .unwrap_or_default()
                        .to_ascii_lowercase();
                    if self.right_panel_sort_order == SortOrder::Asc {
                        a_creator.cmp(&b_creator)
                    } else {
                        b_creator.cmp(&a_creator)
                    }
                },
                SortColumn::Album => {
                    let a_album = a
                        .album
                        .as_deref()
                        .unwrap_or_default()
                        .to_ascii_lowercase();
                    let b_album = b
                        .album
                        .as_deref()
                        .unwrap_or_default()
                        .to_ascii_lowercase();
                    if self.right_panel_sort_order == SortOrder::Asc {
                        a_album.cmp(&b_album)
                    } else {
                        b_album.cmp(&a_album)
                    }
                },
                SortColumn::Title => {
                    let a_title = a
                        .title
                        .as_deref()
                        .unwrap_or_default()
                        .to_ascii_lowercase();
                    let b_title = b
                        .title
                        .as_deref()
                        .unwrap_or_default()
                        .to_ascii_lowercase();
                    if self.right_panel_sort_order == SortOrder::Asc {
                        a_title.cmp(&b_title)
                    } else {
                        b_title.cmp(&a_title)
                    }
                },
                SortColumn::Genre => {
                    let a_genre = a
                        .genre
                        .as_deref()
                        .unwrap_or_default()
                        .to_ascii_lowercase();
                    let b_genre = b
                        .genre
                        .as_deref()
                        .unwrap_or_default()
                        .to_ascii_lowercase();
                    if self.right_panel_sort_order == SortOrder::Asc {
                        a_genre.cmp(&b_genre)
                    } else {
                        b_genre.cmp(&a_genre)
                    }
                },
                SortColumn::Duration => {
                    let a_dur = a.duration_ms.unwrap_or(0);
                    let b_dur = b.duration_ms.unwrap_or(0);
                    if self.right_panel_sort_order == SortOrder::Asc {
                        a_dur.cmp(&b_dur)
                    } else {
                        b_dur.cmp(&a_dur)
                    }
                },
            });
        }
        files
    }
}
