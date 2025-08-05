use std::fs;
use std::path::PathBuf;
use std::collections::HashSet;
use serde::{Serialize, Deserialize};
use crate::fs::file_tree::{FileNode, scan_directory};
use crate::gui::update::restore_expansion_state;

const TOP_DIRS_FILE: &str = ".playlist_ui_top_dirs.json";

fn get_persist_path() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(TOP_DIRS_FILE)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTreeApp {
    #[serde(skip)]
    pub audio_extensions: Vec<String>,
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
    pub fn new(top_dirs: Vec<PathBuf>, all_extensions: Vec<String>, audio_extensions: Vec<String>, persist_path: PathBuf) -> Self {
        let mut root_nodes: Vec<Option<FileNode>> = top_dirs.iter()
            .map(|dir| scan_directory(dir, &all_extensions.iter().map(|s| s.as_str()).collect::<Vec<_>>()))
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
            audio_extensions,
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

    pub fn load(all_extensions: Vec<String>, audio_extensions: Option<Vec<String>>, persist_path: Option<PathBuf>) -> Self {
        let persist_path = persist_path.unwrap_or_else(get_persist_path);
        let audio_extensions = audio_extensions.unwrap_or_default();
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
        FileTreeApp::new(top_dirs, all_extensions, audio_extensions, persist_path)
    }

    pub fn persist_top_dirs(&self) {
        if let Ok(json) = serde_json::to_string(&self.top_dirs) {
            let _ = fs::write(&self.persist_path, json);
        }
    }

    pub fn sorted_right_panel_files(&self) -> Vec<RightPanelFile> {
        let mut files = self.right_panel_files.clone();
        if !self.right_panel_shuffled {
            files.sort_by(|a, b| {
                match self.right_panel_sort_column {
                    SortColumn::Directory => {
                        let a_dir = a.path.parent().and_then(|p| p.file_name()).unwrap_or_default().to_string_lossy().to_ascii_lowercase();
                        let b_dir = b.path.parent().and_then(|p| p.file_name()).unwrap_or_default().to_string_lossy().to_ascii_lowercase();
                        if self.right_panel_sort_order == SortOrder::Asc {
                            a_dir.cmp(&b_dir)
                        } else {
                            b_dir.cmp(&a_dir)
                        }
                    }
                    SortColumn::File => {
                        let a_file = a.path.file_name().unwrap_or_default().to_string_lossy().to_ascii_lowercase();
                        let b_file = b.path.file_name().unwrap_or_default().to_string_lossy().to_ascii_lowercase();
                        if self.right_panel_sort_order == SortOrder::Asc {
                            a_file.cmp(&b_file)
                        } else {
                            b_file.cmp(&a_file)
                        }
                    }
                    SortColumn::Musician => {
                        let a_musician = a.musician.as_deref().unwrap_or_default().to_ascii_lowercase();
                        let b_musician = b.musician.as_deref().unwrap_or_default().to_ascii_lowercase();
                        if self.right_panel_sort_order == SortOrder::Asc {
                            a_musician.cmp(&b_musician)
                        } else {
                            b_musician.cmp(&a_musician)
                        }
                    }
                    SortColumn::Album => {
                        let a_album = a.album.as_deref().unwrap_or_default().to_ascii_lowercase();
                        let b_album = b.album.as_deref().unwrap_or_default().to_ascii_lowercase();
                        if self.right_panel_sort_order == SortOrder::Asc {
                            a_album.cmp(&b_album)
                        } else {
                            b_album.cmp(&a_album)
                        }
                    }
                    SortColumn::Title => {
                        let a_title = a.title.as_deref().unwrap_or_default().to_ascii_lowercase();
                        let b_title = b.title.as_deref().unwrap_or_default().to_ascii_lowercase();
                        if self.right_panel_sort_order == SortOrder::Asc {
                            a_title.cmp(&b_title)
                        } else {
                            b_title.cmp(&a_title)
                        }
                    }
                    SortColumn::Genre => {
                        let a_genre = a.genre.as_deref().unwrap_or_default().to_ascii_lowercase();
                        let b_genre = b.genre.as_deref().unwrap_or_default().to_ascii_lowercase();
                        if self.right_panel_sort_order == SortOrder::Asc {
                            a_genre.cmp(&b_genre)
                        } else {
                            b_genre.cmp(&a_genre)
                        }
                    }
                }
            });
        }
        files
    }
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
    SortRightPanelByMusician,
    SortRightPanelByAlbum,
    SortRightPanelByTitle,
    SortRightPanelByGenre,
    ShuffleRightPanel,
    ExportRightPanelAsXspf,
    ExportRightPanelAsXspfTo(PathBuf),
    ExportAndPlayRightPanelAsXspf,
    OpenRightPanelFile(PathBuf),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SortColumn {
    Directory,
    File,
    Musician,
    Album,
    Title,
    Genre,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SortOrder {
    Asc,
    Desc,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RightPanelFile {
    pub path: PathBuf,
    pub musician: Option<String>,
    pub album: Option<String>,
    pub title: Option<String>,
    pub genre: Option<String>,
}

