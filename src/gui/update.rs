//! Message-handling update logic for the Playlist UI.
//!
//! Implements the pure state-transition function required by the iced
//! Elm architecture. Each `Message` variant is handled by a corresponding
//! arm that mutates `FileTreeApp` and returns an optional `Task` for
//! side effects (file dialogs, exports, etc.).
//!
//! Public API:
//!     update — handle a message and transition the app state
//!     restore_expansion_state — walk a tree restoring expanded dirs
//!     find_tag_node_mut — locate a tag tree node by label path
//!     collect_tag_node_files — gather all file paths under a tag node

use crate::fs::file_tree::{FileNode, NodeType, scan_directory};
use crate::fs::media_metadata::{
    build_creator_tag_tree, build_genre_tag_tree, extract_media_metadata,
};
use crate::gui::left_panel::{filter_file_node, filter_tag_node};
use crate::gui::{
    FileTreeApp, LeftPanelSelectMode, LeftPanelSortMode, Message,
    RightPanelFile, SortColumn, SortOrder, TagTreeNode, TextSearchMode,
};
use crate::utils::file_field_matches;
use iced::Task;
use rfd::FileDialog;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Recomputes `filtered_root_nodes` from `app.root_nodes` using the current
/// search query and mode. Called whenever the query string or search mode
/// changes.
fn recompute_filtered_nodes(app: &FileTreeApp) -> Vec<Option<FileNode>> {
    if app.search_query.is_empty() {
        app.root_nodes.clone()
    } else {
        app.root_nodes
            .iter()
            .map(|node_opt| {
                node_opt.as_ref().and_then(|node| {
                    filter_file_node(node, &app.search_query, app.search_mode)
                })
            })
            .collect()
    }
}

fn recompute_filtered_tag_nodes(app: &FileTreeApp) -> Vec<TagTreeNode> {
    if app.search_query.is_empty() {
        app.tag_tree_roots.clone()
    } else {
        app.tag_tree_roots
            .iter()
            .filter_map(|node| {
                filter_tag_node(node, &app.search_query, app.search_mode)
            })
            .collect()
    }
}

/// Returns the files that should be displayed in the right panel,
/// sorted according to the current sort settings. When a search query
/// is active, only matching files are returned; otherwise all files
/// are returned.
fn displayed_right_panel_files(app: &FileTreeApp) -> Vec<RightPanelFile> {
    let files = if app.search_query.is_empty() {
        app.right_panel_files.clone()
    } else {
        app.filtered_right_panel_files.clone()
    };
    let mut files = files;
    if !app.right_panel_shuffled {
        files.sort_by(|a, b| {
            let filename_cmp = || {
                let a_name = a
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_ascii_lowercase();
                let b_name = b
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_ascii_lowercase();
                a_name.cmp(&b_name)
            };
            match app.right_panel_sort_column {
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
                    let primary =
                        if app.right_panel_sort_order == SortOrder::Asc {
                            a_dir.cmp(&b_dir)
                        } else {
                            b_dir.cmp(&a_dir)
                        };
                    primary.then_with(filename_cmp)
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
                    if app.right_panel_sort_order == SortOrder::Asc {
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
                    let primary =
                        if app.right_panel_sort_order == SortOrder::Asc {
                            a_creator.cmp(&b_creator)
                        } else {
                            b_creator.cmp(&a_creator)
                        };
                    primary.then_with(filename_cmp)
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
                    let primary =
                        if app.right_panel_sort_order == SortOrder::Asc {
                            a_album.cmp(&b_album)
                        } else {
                            b_album.cmp(&a_album)
                        };
                    primary.then_with(filename_cmp)
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
                    let primary =
                        if app.right_panel_sort_order == SortOrder::Asc {
                            a_title.cmp(&b_title)
                        } else {
                            b_title.cmp(&a_title)
                        };
                    primary.then_with(filename_cmp)
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
                    let primary =
                        if app.right_panel_sort_order == SortOrder::Asc {
                            a_genre.cmp(&b_genre)
                        } else {
                            b_genre.cmp(&a_genre)
                        };
                    primary.then_with(filename_cmp)
                },
                SortColumn::Duration => {
                    let a_dur = a.duration_ms.unwrap_or(0);
                    let b_dur = b.duration_ms.unwrap_or(0);
                    let primary =
                        if app.right_panel_sort_order == SortOrder::Asc {
                            a_dur.cmp(&b_dur)
                        } else {
                            b_dur.cmp(&a_dur)
                        };
                    primary.then_with(filename_cmp)
                },
            }
        });
    }
    files
}

/// Recomputes `filtered_right_panel_files` from `app.right_panel_files`
/// using the current search query and mode. When the query is empty,
/// returns an empty vector signalling the view to fall back to
/// `sorted_right_panel_files()`. When a query is active, filters files
/// according to the active search mode and returns the matching subset
/// (unsorted — sorting is applied by the view).
fn recompute_filtered_right_panel_files(
    app: &FileTreeApp,
) -> Vec<RightPanelFile> {
    if app.search_query.is_empty() {
        return Vec::new();
    }
    let query = &app.search_query;
    app.right_panel_files
        .iter()
        .filter(|f| match app.search_mode {
            TextSearchMode::All => {
                file_field_matches(&f.creator, query)
                    || file_field_matches(&f.album, query)
                    || file_field_matches(&f.title, query)
                    || file_field_matches(&f.genre, query)
            },
            TextSearchMode::Creator => file_field_matches(&f.creator, query),
            TextSearchMode::Album => file_field_matches(&f.album, query),
            TextSearchMode::Title => file_field_matches(&f.title, query),
            TextSearchMode::Genre => file_field_matches(&f.genre, query),
            TextSearchMode::DirectoryPath => f
                .path
                .to_string_lossy()
                .to_ascii_lowercase()
                .contains(&query.to_ascii_lowercase()),
            TextSearchMode::TrackFilename => f
                .path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_lowercase()
                .contains(&query.to_ascii_lowercase()),
        })
        .cloned()
        .collect()
}

/// Restores the expansion state of a file tree node and its descendants based
/// on the provided set of expanded directory paths.
pub fn restore_expansion_state(
    node: &mut FileNode,
    expanded_dirs: &HashSet<PathBuf>,
) {
    node.is_expanded = expanded_dirs.contains(&node.path);
    for child in &mut node.children {
        restore_expansion_state(child, expanded_dirs);
    }
}

/// Recursively collects all file paths from the given file tree node and its
/// descendants, appending them to the provided vector.
fn collect_files_recursively(node: &FileNode, files: &mut Vec<PathBuf>) {
    match node.node_type {
        NodeType::File => files.push(node.path.clone()),
        NodeType::Directory => {
            for child in &node.children {
                collect_files_recursively(child, files);
            }
        },
    }
}

/// Recursively searches for a node in the file tree with the specified path,
///     returning a reference to the node if found.
fn find_node_by_path<'a>(
    node: &'a FileNode,
    path: &Path,
) -> Option<&'a FileNode> {
    if node.path.as_path() == path {
        return Some(node);
    }
    for child in &node.children {
        if let Some(found) = find_node_by_path(child, path) {
            return Some(found);
        }
    }
    None
}

/// Recursively searches for a mutable reference to a tag tree node by path.
///
/// Traverses the `TagTreeNode` hierarchy using the provided sequence of labels
/// in `path`.
/// Returns a mutable reference to the node at the end of the path if found, or
/// `None` if any label in the path does not exist at the corresponding level.
pub fn find_tag_node_mut<'a>(
    nodes: &'a mut [TagTreeNode],
    path: &[String],
) -> Option<&'a mut TagTreeNode> {
    let mut current_nodes = nodes;

    for (i, label) in path.iter().enumerate() {
        let found = current_nodes.iter_mut().find(|n| &n.label == label)?;
        if i == path.len() - 1 {
            return Some(found);
        }
        current_nodes = &mut found.children;
    }
    None
}

/// Recursively collects all file paths from a tag tree node and its
/// descendants.
///
/// Traverses the given `TagTreeNode` and all of its children, appending any
/// file paths found in each node's `file_paths` field to the provided `files`
/// vector. This is used to gather all media files under a specific tag node
/// (e.g., genre, artist, album, or track).
pub fn collect_tag_node_files(node: &TagTreeNode, files: &mut Vec<PathBuf>) {
    files.extend(node.file_paths.iter().cloned());
    for child in &node.children {
        collect_tag_node_files(child, files);
    }
}

/// Handles all application state updates in response to user actions or
/// messages, modifying the `FileTreeApp` state and returning an optional
/// asynchronous task.
pub fn update(app: &mut FileTreeApp, message: Message) -> Task<Message> {
    match message {
        Message::ToggleExpansion(path) => {
            if app.expanded_dirs.contains(&path) {
                app.expanded_dirs.remove(&path);
            } else {
                app.expanded_dirs.insert(path);
            }
            for root in app.root_nodes.iter_mut().flatten() {
                restore_expansion_state(root, &app.expanded_dirs);
            }
            app.filtered_root_nodes = recompute_filtered_nodes(app);
            Task::none()
        },
        Message::ToggleExtension(ext) => {
            if app.all_extensions.contains(&ext) {
                if app.selected_extensions.contains(&ext) {
                    app.selected_extensions.retain(|e| e != &ext);
                } else {
                    app.selected_extensions.push(ext);
                }
            }
            app.root_nodes = app
                .top_dirs
                .iter()
                .map(|dir| {
                    scan_directory(
                        dir,
                        &app.selected_extensions
                            .iter()
                            .map(|s| s.as_str())
                            .collect::<Vec<_>>(),
                    )
                })
                .collect();

            for root in app.root_nodes.iter_mut().flatten() {
                restore_expansion_state(root, &app.expanded_dirs);
            }
            app.filtered_root_nodes = recompute_filtered_nodes(app);
            app.filtered_tag_tree_roots = recompute_filtered_tag_nodes(app);
            Task::none()
        },
        Message::ToggleExtensionsMenu => {
            app.extensions_menu_expanded = !app.extensions_menu_expanded;
            Task::none()
        },
        Message::RemoveTopDir(dir) => {
            if let Some(idx) = app.top_dirs.iter().position(|d| d == &dir) {
                app.top_dirs.remove(idx);
                app.root_nodes.remove(idx);
                if let Err(e) = app.persist_top_dirs() {
                    log::error!("Failed to persist top dirs: {e}");
                }
            }
            Task::none()
        },
        Message::AddDirectory => Task::perform(
            async move { FileDialog::new().pick_folder() },
            Message::DirectoryAdded,
        ),
        Message::DirectoryAdded(Some(mut path)) => {
            // If the added path is a file, use its parent directory
            if path.is_file()
                && let Some(parent) = path.parent()
            {
                path = parent.to_path_buf();
            }
            if !app.top_dirs.contains(&path) && path.exists() && path.is_dir() {
                app.top_dirs.push(path.clone());
                app.root_nodes.push(scan_directory(
                    &path,
                    &app.selected_extensions
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>(),
                ));
                if let Err(e) = app.persist_top_dirs() {
                    log::error!("Failed to persist top dirs: {e}");
                }
            }
            Task::none()
        },
        Message::DirectoryAdded(None) => Task::none(),
        Message::AddToRightPanel(path) => {
            app.right_panel_shuffled = false;
            if !app.right_panel_files.iter().any(|f| f.path == path) {
                let meta = extract_media_metadata(&path);
                app.right_panel_files.push(RightPanelFile {
                    path,
                    creator: meta.creator,
                    album: meta.album,
                    title: meta.title,
                    genre: meta.genre,
                    duration_ms: meta.duration_ms,
                });
            }
            Task::none()
        },
        Message::AddDirectoryToRightPanel(dir_path) => {
            app.right_panel_shuffled = false;
            for root in app.root_nodes.iter().flatten() {
                if let Some(node) = find_node_by_path(root, &dir_path) {
                    let mut files = Vec::new();
                    collect_files_recursively(node, &mut files);
                    for file in files {
                        if !app.right_panel_files.iter().any(|f| f.path == file)
                        {
                            let meta = extract_media_metadata(&file);
                            app.right_panel_files.push(RightPanelFile {
                                path: file,
                                creator: meta.creator,
                                album: meta.album,
                                title: meta.title,
                                genre: meta.genre,
                                duration_ms: meta.duration_ms,
                            });
                        }
                    }
                }
            }
            Task::none()
        },
        Message::RemoveFromRightPanel(path) => {
            app.right_panel_files.retain(|f| f.path != path);
            Task::none()
        },
        Message::RemoveDirectoryFromRightPanel(dir_path) => {
            app.right_panel_files.retain(|file| {
                // Remove if file is not in dir_path or its subdirectories
                !file.path.starts_with(&dir_path)
            });
            Task::none()
        },
        Message::SortRightPanelByDirectory => {
            if app.right_panel_sort_column == SortColumn::Directory {
                app.right_panel_sort_order = match app.right_panel_sort_order {
                    SortOrder::Asc => SortOrder::Desc,
                    SortOrder::Desc => SortOrder::Asc,
                };
            } else {
                app.right_panel_sort_column = SortColumn::Directory;
                app.right_panel_sort_order = SortOrder::Asc;
            }
            app.right_panel_shuffled = false;
            Task::none()
        },
        Message::SortRightPanelByFile => {
            if app.right_panel_sort_column == SortColumn::File {
                app.right_panel_sort_order = match app.right_panel_sort_order {
                    SortOrder::Asc => SortOrder::Desc,
                    SortOrder::Desc => SortOrder::Asc,
                };
            } else {
                app.right_panel_sort_column = SortColumn::File;
                app.right_panel_sort_order = SortOrder::Asc;
            }
            app.right_panel_shuffled = false;
            Task::none()
        },
        Message::SortRightPanelByCreator => {
            if app.right_panel_sort_column == SortColumn::Creator {
                app.right_panel_sort_order = match app.right_panel_sort_order {
                    SortOrder::Asc => SortOrder::Desc,
                    SortOrder::Desc => SortOrder::Asc,
                };
            } else {
                app.right_panel_sort_column = SortColumn::Creator;
                app.right_panel_sort_order = SortOrder::Asc;
            }
            app.right_panel_shuffled = false;
            Task::none()
        },
        Message::SortRightPanelByAlbum => {
            if app.right_panel_sort_column == SortColumn::Album {
                app.right_panel_sort_order = match app.right_panel_sort_order {
                    SortOrder::Asc => SortOrder::Desc,
                    SortOrder::Desc => SortOrder::Asc,
                };
            } else {
                app.right_panel_sort_column = SortColumn::Album;
                app.right_panel_sort_order = SortOrder::Asc;
            }
            app.right_panel_shuffled = false;
            Task::none()
        },
        Message::SortRightPanelByTitle => {
            if app.right_panel_sort_column == SortColumn::Title {
                app.right_panel_sort_order = match app.right_panel_sort_order {
                    SortOrder::Asc => SortOrder::Desc,
                    SortOrder::Desc => SortOrder::Asc,
                };
            } else {
                app.right_panel_sort_column = SortColumn::Title;
                app.right_panel_sort_order = SortOrder::Asc;
            }
            app.right_panel_shuffled = false;
            Task::none()
        },
        Message::SortRightPanelByGenre => {
            if app.right_panel_sort_column == SortColumn::Genre {
                app.right_panel_sort_order = match app.right_panel_sort_order {
                    SortOrder::Asc => SortOrder::Desc,
                    SortOrder::Desc => SortOrder::Asc,
                };
            } else {
                app.right_panel_sort_column = SortColumn::Genre;
                app.right_panel_sort_order = SortOrder::Asc;
            }
            app.right_panel_shuffled = false;
            Task::none()
        },
        Message::SortRightPanelByDuration => {
            if app.right_panel_sort_column == SortColumn::Duration {
                app.right_panel_sort_order = match app.right_panel_sort_order {
                    SortOrder::Asc => SortOrder::Desc,
                    SortOrder::Desc => SortOrder::Asc,
                };
            } else {
                app.right_panel_sort_column = SortColumn::Duration;
                app.right_panel_sort_order = SortOrder::Asc;
            }
            app.right_panel_shuffled = false;
            Task::none()
        },
        Message::ShuffleRightPanel => {
            use rand::seq::SliceRandom;
            let mut rng = rand::rng();
            app.right_panel_files.shuffle(&mut rng);
            app.right_panel_shuffled = true;
            Task::none()
        },
        Message::ExportRightPanelAsXspf => {
            Task::perform(
                async move {
                    rfd::FileDialog::new()
                        .set_file_name("playlist.xspf")
                        .save_file()
                },
                |opt| match opt {
                    Some(path) => Message::ExportRightPanelAsXspfTo(path),
                    None => Message::ToggleExtensionsMenu, // no-op or feedback
                },
            )
        },
        Message::ExportRightPanelAsXspfTo(path) => {
            let audio_exts: &Vec<String> = &app.all_extensions;
            let audio_files: Vec<RightPanelFile> =
                displayed_right_panel_files(app)
                    .into_iter()
                    .filter(|f| {
                        f.path
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|ext| audio_exts.iter().any(|ae| ae == ext))
                            .unwrap_or(false)
                    })
                    .collect();
            let _ = crate::fs::xspf::export_xspf_playlist(&audio_files, &path);
            Task::none()
        },
        Message::ExportAndPlayRightPanelAsXspf => {
            use std::env::temp_dir;
            use std::process::Command;

            let audio_exts: &Vec<String> = &app.all_extensions;
            let audio_files: Vec<RightPanelFile> =
                displayed_right_panel_files(app)
                    .into_iter()
                    .filter(|f| {
                        f.path
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|ext| audio_exts.iter().any(|ae| ae == ext))
                            .unwrap_or(false)
                    })
                    .collect();

            let xspf_path = temp_dir().join("playlist.xspf");
            let _ =
                crate::fs::xspf::export_xspf_playlist(&audio_files, &xspf_path);

            // Launch VLC with the playlist
            let _ =
                Command::new("vlc").arg(xspf_path.to_str().unwrap()).spawn();

            Task::none()
        },
        Message::OpenRightPanelFile(path) => {
            #[cfg(target_os = "windows")]
            let _ = std::process::Command::new("cmd")
                .args(&["/C", "start", "", path.to_str().unwrap()])
                .spawn();

            #[cfg(target_os = "macos")]
            let _ = std::process::Command::new("open")
                .arg(path.to_str().unwrap())
                .spawn();

            #[cfg(target_os = "linux")]
            let _ = std::process::Command::new("xdg-open")
                .arg(path.to_str().unwrap())
                .spawn();

            Task::none()
        },
        Message::ToggleLeftPanel => {
            app.left_panel_expanded = !app.left_panel_expanded;
            Task::none()
        },
        Message::ToggleLeftPanelSortMode => {
            app.left_panel_sort_mode = match app.left_panel_sort_mode {
                LeftPanelSortMode::Alphanumeric => {
                    LeftPanelSortMode::ModifiedDate
                },
                LeftPanelSortMode::ModifiedDate => LeftPanelSortMode::FileCount,
                LeftPanelSortMode::FileCount => LeftPanelSortMode::Alphanumeric,
            };
            Task::none()
        },
        Message::SearchCleared => {
            app.search_query = String::new();
            app.filtered_root_nodes = recompute_filtered_nodes(app);
            app.filtered_tag_tree_roots = recompute_filtered_tag_nodes(app);
            app.filtered_right_panel_files =
                recompute_filtered_right_panel_files(app);
            Task::none()
        },
        Message::SearchQueryChanged(query) => {
            app.search_query = query;
            app.filtered_root_nodes = recompute_filtered_nodes(app);
            app.filtered_tag_tree_roots = recompute_filtered_tag_nodes(app);
            app.filtered_right_panel_files =
                recompute_filtered_right_panel_files(app);
            Task::none()
        },
        Message::ToggleSearchMode => {
            app.search_mode = match app.search_mode {
                TextSearchMode::All => TextSearchMode::DirectoryPath,
                TextSearchMode::DirectoryPath => TextSearchMode::TrackFilename,
                TextSearchMode::TrackFilename => TextSearchMode::Creator,
                TextSearchMode::Creator => TextSearchMode::Album,
                TextSearchMode::Album => TextSearchMode::Title,
                TextSearchMode::Title => TextSearchMode::Genre,
                TextSearchMode::Genre => TextSearchMode::All,
            };
            app.filtered_root_nodes = recompute_filtered_nodes(app);
            app.filtered_tag_tree_roots = recompute_filtered_tag_nodes(app);
            app.filtered_right_panel_files =
                recompute_filtered_right_panel_files(app);
            Task::none()
        },
        Message::ToggleLeftPanelSelectMode => {
            app.left_panel_selection_mode = match app.left_panel_selection_mode
            {
                LeftPanelSelectMode::Directory => {
                    // Switch to tag mode: load from sled if possible
                    if let Some(ref sled_store) = app.sled_store {
                        if let Some(tree) = sled_store.load_genre_tag_tree() {
                            app.tag_tree_roots = tree;
                        } else {
                            let tree = build_genre_tag_tree(
                                &app.top_dirs,
                                &app.selected_extensions,
                            );
                            sled_store.save_genre_tag_tree(&tree).ok();
                            app.tag_tree_roots = tree;
                        }
                    } else {
                        app.tag_tree_roots = build_genre_tag_tree(
                            &app.top_dirs,
                            &app.selected_extensions,
                        );
                    }
                    LeftPanelSelectMode::GenreTag
                },
                LeftPanelSelectMode::GenreTag => {
                    // Switch to creator mode: load from sled if possible
                    if let Some(ref sled_store) = app.sled_store {
                        if let Some(tree) = sled_store.load_creator_tag_tree() {
                            app.tag_tree_roots = tree;
                        } else {
                            let tree = build_creator_tag_tree(
                                &app.top_dirs,
                                &app.selected_extensions,
                            );
                            sled_store.save_creator_tag_tree(&tree).ok();
                            app.tag_tree_roots = tree;
                        }
                    } else {
                        app.tag_tree_roots = build_creator_tag_tree(
                            &app.top_dirs,
                            &app.selected_extensions,
                        );
                    }
                    LeftPanelSelectMode::CreatorTag
                },
                LeftPanelSelectMode::CreatorTag => {
                    LeftPanelSelectMode::Directory
                },
            };
            Task::none()
        },
        Message::ToggleTagExpansion(path) => {
            if let Some(node) =
                find_tag_node_mut(&mut app.tag_tree_roots, &path)
            {
                node.is_expanded = !node.is_expanded;
            }
            app.filtered_tag_tree_roots = recompute_filtered_tag_nodes(app);
            Task::none()
        },
        Message::ClearRightPanel => {
            app.right_panel_files.clear();
            app.right_panel_shuffled = false;
            Task::none()
        },
        Message::AddTagNodeToRightPanel(path) => {
            app.right_panel_shuffled = false;
            if let Some(node) =
                find_tag_node_mut(&mut app.tag_tree_roots, &path)
            {
                let mut files = Vec::new();
                collect_tag_node_files(node, &mut files);
                for file in files {
                    if !app.right_panel_files.iter().any(|f| f.path == file) {
                        let meta = extract_media_metadata(&file);
                        app.right_panel_files.push(RightPanelFile {
                            path: file,
                            creator: meta.creator,
                            album: meta.album,
                            title: meta.title,
                            genre: meta.genre,
                            duration_ms: meta.duration_ms,
                        });
                    }
                }
            }
            Task::none()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::TextSearchMode;
    use std::path::PathBuf;

    #[test]
    fn test_search_query_changed() {
        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        let _ = update(
            &mut app,
            Message::SearchQueryChanged("test query".to_string()),
        );
        assert_eq!(app.search_query, "test query");
    }

    #[test]
    fn test_search_cleared() {
        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.search_query = "something".to_string();
        let _ = update(&mut app, Message::SearchCleared);
        assert_eq!(app.search_query, "");
    }

    #[test]
    fn test_search_query_cleared() {
        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.search_query = "previous".to_string();
        let _ = update(&mut app, Message::SearchQueryChanged(String::new()));
        assert_eq!(app.search_query, "");
    }

    #[test]
    fn test_toggle_tag_expansion_during_search_updates_filtered() {
        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.tag_tree_roots = vec![TagTreeNode {
            label: "Jazz".to_string(),
            children: vec![],
            file_paths: vec![],
            is_expanded: false,
            file_count: 42,
        }];
        app.search_query = "Jazz".to_string();
        app.filtered_tag_tree_roots = recompute_filtered_tag_nodes(&app);

        assert!(!app.filtered_tag_tree_roots[0].is_expanded);
        let path = vec!["Jazz".to_string()];
        let _ = update(&mut app, Message::ToggleTagExpansion(path));

        assert!(app.tag_tree_roots[0].is_expanded);
        assert!(app.filtered_tag_tree_roots[0].is_expanded);
    }

    #[test]
    fn test_toggle_tag_expansion_no_search_preserves_filtered() {
        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.tag_tree_roots = vec![TagTreeNode {
            label: "Jazz".to_string(),
            children: vec![],
            file_paths: vec![],
            is_expanded: false,
            file_count: 42,
        }];
        // No search query set — filtered matches original
        app.filtered_tag_tree_roots = app.tag_tree_roots.clone();

        let path = vec!["Jazz".to_string()];
        let _ = update(&mut app, Message::ToggleTagExpansion(path));

        assert!(app.tag_tree_roots[0].is_expanded);
        assert!(app.filtered_tag_tree_roots[0].is_expanded);
    }

    #[test]
    fn test_tag_expansion_nonmatching_parent_matching_child() {
        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.tag_tree_roots = vec![TagTreeNode {
            label: "Parent".to_string(),
            children: vec![TagTreeNode {
                label: "Jazz".to_string(),
                children: vec![],
                file_paths: vec![PathBuf::from("/music/track.mp3")],
                is_expanded: false,
                file_count: 1,
            }],
            file_paths: vec![],
            is_expanded: false,
            file_count: 1,
        }];
        // Search for a child label, not the parent label
        app.search_query = "Jazz".to_string();
        app.filtered_tag_tree_roots = recompute_filtered_tag_nodes(&app);

        // Parent should appear in filtered via child match, not expanded
        assert!(!app.filtered_tag_tree_roots[0].is_expanded);
        assert_eq!(app.filtered_tag_tree_roots[0].children.len(), 1);

        // Toggle expansion on the original parent node
        let path = vec!["Parent".to_string()];
        let _ = update(&mut app, Message::ToggleTagExpansion(path));

        assert!(app.tag_tree_roots[0].is_expanded);
        // After recompute, filtered parent should also be expanded
        assert!(app.filtered_tag_tree_roots[0].is_expanded);
        // Matching child should still be present
        assert_eq!(app.filtered_tag_tree_roots[0].children.len(), 1);
    }

    #[test]
    fn test_toggle_expansion_during_search_updates_filtered() {
        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        let dir = FileNode::new_directory(
            "Music".to_string(),
            PathBuf::from("/Music"),
            vec![FileNode::new_file(
                "song.mp3".to_string(),
                PathBuf::from("/Music/song.mp3"),
            )],
        );
        app.root_nodes = vec![Some(dir)];
        app.search_query = "song".to_string();
        app.filtered_root_nodes = recompute_filtered_nodes(&app);

        // Directory is in filtered (child matches search) but not expanded
        assert!(!app.filtered_root_nodes[0].as_ref().unwrap().is_expanded);

        let _ =
            update(&mut app, Message::ToggleExpansion(PathBuf::from("/Music")));

        // Both original and filtered should now be expanded
        assert!(app.root_nodes[0].as_ref().unwrap().is_expanded);
        assert!(app.filtered_root_nodes[0].as_ref().unwrap().is_expanded);
    }

    #[test]
    fn test_toggle_expansion_no_search_preserves_filtered() {
        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        let dir = FileNode::new_directory(
            "Music".to_string(),
            PathBuf::from("/Music"),
            vec![FileNode::new_file(
                "song.mp3".to_string(),
                PathBuf::from("/Music/song.mp3"),
            )],
        );
        app.root_nodes = vec![Some(dir)];
        // No search — filtered is a clone of original
        app.filtered_root_nodes = app.root_nodes.clone();

        let _ =
            update(&mut app, Message::ToggleExpansion(PathBuf::from("/Music")));

        assert!(app.root_nodes[0].as_ref().unwrap().is_expanded);
        assert!(app.filtered_root_nodes[0].as_ref().unwrap().is_expanded);
    }

    #[test]
    fn test_toggle_extension_recomputes_filtered_trees() {
        let mut app = FileTreeApp::new(
            vec![PathBuf::from("/dummy")],
            &["mp3", "flac"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.tag_tree_roots = vec![TagTreeNode {
            label: "Jazz".to_string(),
            children: vec![],
            file_paths: vec![],
            is_expanded: false,
            file_count: 42,
        }];
        app.search_query = "Jazz".to_string();
        // Set stale filtered trees to empty — they will be recomputed
        app.filtered_root_nodes = vec![];
        app.filtered_tag_tree_roots = vec![];

        let msg = Message::ToggleExtension("flac".to_string());
        let _ = update(&mut app, msg);

        // filtered_root_nodes should match recomputed state (None from
        // non-existent dir, no matching files)
        assert_eq!(app.filtered_root_nodes.len(), 1);
        assert!(app.filtered_root_nodes[0].is_none());

        // filtered_tag_tree_roots should have the matching Jazz node
        assert_eq!(app.filtered_tag_tree_roots.len(), 1);
        assert_eq!(app.filtered_tag_tree_roots[0].label, "Jazz");
    }

    #[test]
    fn test_toggle_extension_recomputes_filtered_trees_no_search() {
        let mut app = FileTreeApp::new(
            vec![PathBuf::from("/dummy")],
            &["mp3", "flac"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.tag_tree_roots = vec![TagTreeNode {
            label: "Jazz".to_string(),
            children: vec![],
            file_paths: vec![],
            is_expanded: true,
            file_count: 42,
        }];
        // No search — filtered is a clone of original
        app.filtered_root_nodes = vec![];
        app.filtered_tag_tree_roots = vec![];

        let msg = Message::ToggleExtension("flac".to_string());
        let _ = update(&mut app, msg);

        // Without search, both filtered trees should be clones of originals
        assert_eq!(app.filtered_root_nodes.len(), app.root_nodes.len());
        assert_eq!(app.filtered_tag_tree_roots.len(), app.tag_tree_roots.len());
        assert!(app.filtered_tag_tree_roots[0].is_expanded);
    }

    #[test]
    fn test_toggle_search_mode_cycles_all_modes() {
        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        // Start at All (default)
        assert_eq!(app.search_mode, TextSearchMode::All);

        // Cycle through all modes
        let _ = update(&mut app, Message::ToggleSearchMode);
        assert_eq!(app.search_mode, TextSearchMode::DirectoryPath);

        let _ = update(&mut app, Message::ToggleSearchMode);
        assert_eq!(app.search_mode, TextSearchMode::TrackFilename);

        let _ = update(&mut app, Message::ToggleSearchMode);
        assert_eq!(app.search_mode, TextSearchMode::Creator);

        let _ = update(&mut app, Message::ToggleSearchMode);
        assert_eq!(app.search_mode, TextSearchMode::Album);

        let _ = update(&mut app, Message::ToggleSearchMode);
        assert_eq!(app.search_mode, TextSearchMode::Title);

        let _ = update(&mut app, Message::ToggleSearchMode);
        assert_eq!(app.search_mode, TextSearchMode::Genre);

        // One more wraps back to All
        let _ = update(&mut app, Message::ToggleSearchMode);
        assert_eq!(app.search_mode, TextSearchMode::All);
    }

    // ── right-panel filtering tests ──────────────────────────────────

    /// Helper to create a RightPanelFile with metadata.
    fn rp_file(
        path: &str,
        creator: Option<&str>,
        album: Option<&str>,
        title: Option<&str>,
        genre: Option<&str>,
    ) -> RightPanelFile {
        RightPanelFile {
            path: PathBuf::from(path),
            creator: creator.map(|s| s.to_string()),
            album: album.map(|s| s.to_string()),
            title: title.map(|s| s.to_string()),
            genre: genre.map(|s| s.to_string()),
            duration_ms: None,
        }
    }

    fn app_with_right_panel_files(files: Vec<RightPanelFile>) -> FileTreeApp {
        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.right_panel_files = files;
        app
    }

    #[test]
    fn test_right_panel_filter_empty_query() {
        let app = app_with_right_panel_files(vec![
            rp_file("/a/song.mp3", Some("Artist"), None, None, None),
            rp_file("/b/track.mp3", None, None, None, None),
        ]);
        let result = recompute_filtered_right_panel_files(&app);
        // Empty query returns empty (signals "no filtering needed")
        assert!(result.is_empty());
    }

    #[test]
    fn test_right_panel_filter_genre_mode() {
        let mut app = app_with_right_panel_files(vec![
            rp_file("/a/rock.mp3", None, None, None, Some("Rock")),
            rp_file("/b/jazz.mp3", None, None, None, Some("Jazz")),
            rp_file("/c/unknown.mp3", None, None, None, None),
        ]);
        app.search_query = "rock".to_string();
        app.search_mode = TextSearchMode::Genre;
        let result = recompute_filtered_right_panel_files(&app);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, PathBuf::from("/a/rock.mp3"));
    }

    #[test]
    fn test_right_panel_filter_title_mode() {
        let mut app = app_with_right_panel_files(vec![
            rp_file("/a/track.mp3", None, None, Some("My Song"), None),
            rp_file("/b/other.mp3", None, None, Some("Different"), None),
        ]);
        app.search_query = "my song".to_string();
        app.search_mode = TextSearchMode::Title;
        let result = recompute_filtered_right_panel_files(&app);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, Some("My Song".to_string()));
    }

    #[test]
    fn test_right_panel_filter_album_mode() {
        let mut app = app_with_right_panel_files(vec![
            rp_file("/a/a.mp3", None, Some("Abbey Road"), None, None),
            rp_file("/b/b.mp3", None, Some("Revolver"), None, None),
        ]);
        app.search_query = "abbey".to_string();
        app.search_mode = TextSearchMode::Album;
        let result = recompute_filtered_right_panel_files(&app);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].album, Some("Abbey Road".to_string()));
    }

    #[test]
    fn test_right_panel_filter_creator_mode() {
        let mut app = app_with_right_panel_files(vec![
            rp_file("/a/a.mp3", Some("Miles Davis"), None, None, None),
            rp_file("/b/b.mp3", Some("Coltrane"), None, None, None),
        ]);
        app.search_query = "miles".to_string();
        app.search_mode = TextSearchMode::Creator;
        let result = recompute_filtered_right_panel_files(&app);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].creator, Some("Miles Davis".to_string()));
    }

    #[test]
    fn test_right_panel_filter_path_mode() {
        let mut app = app_with_right_panel_files(vec![
            rp_file("/music/jazz/song.mp3", None, None, None, None),
            rp_file("/music/rock/track.mp3", None, None, None, None),
        ]);
        app.search_query = "jazz".to_string();
        app.search_mode = TextSearchMode::DirectoryPath;
        let result = recompute_filtered_right_panel_files(&app);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, PathBuf::from("/music/jazz/song.mp3"));
    }

    #[test]
    fn test_right_panel_filter_filename_mode() {
        let mut app = app_with_right_panel_files(vec![
            rp_file("/a/my_song.mp3", None, None, None, None),
            rp_file("/b/other_track.mp3", None, None, None, None),
        ]);
        app.search_query = "my_song".to_string();
        app.search_mode = TextSearchMode::TrackFilename;
        let result = recompute_filtered_right_panel_files(&app);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, PathBuf::from("/a/my_song.mp3"));
    }

    #[test]
    fn test_right_panel_filter_all_mode() {
        let mut app = app_with_right_panel_files(vec![
            rp_file("/a/a.mp3", None, None, Some("Little Wing"), None),
            rp_file("/b/b.mp3", None, None, None, Some("Littlesound")),
            rp_file("/c/c.mp3", None, None, None, None),
        ]);
        app.search_query = "little".to_string();
        app.search_mode = TextSearchMode::All;
        let result = recompute_filtered_right_panel_files(&app);
        // Title match for first, genre match for second, third excluded
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_right_panel_filter_no_match() {
        let mut app = app_with_right_panel_files(vec![rp_file(
            "/a/a.mp3",
            Some("Artist"),
            None,
            None,
            None,
        )]);
        app.search_query = "nonexistent".to_string();
        app.search_mode = TextSearchMode::All;
        let result = recompute_filtered_right_panel_files(&app);
        assert!(result.is_empty());
    }

    #[test]
    fn test_right_panel_filter_case_insensitive() {
        let mut app = app_with_right_panel_files(vec![rp_file(
            "/a/a.mp3",
            None,
            None,
            Some("LITTLE WING"),
            None,
        )]);
        app.search_query = "little".to_string();
        app.search_mode = TextSearchMode::Title;
        let result = recompute_filtered_right_panel_files(&app);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_right_panel_filter_metadata_none_fields() {
        let mut app = app_with_right_panel_files(vec![
            rp_file("/a/a.mp3", None, None, None, None),
            rp_file("/b/b.mp3", Some("Artist"), None, None, None),
        ]);
        app.search_query = "Artist".to_string();
        app.search_mode = TextSearchMode::Creator;
        let result = recompute_filtered_right_panel_files(&app);
        // Only the file with a creator matching "Artist" should be kept
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].creator, Some("Artist".to_string()));
    }

    #[test]
    fn test_toggle_search_mode_changes_filtered_right_panel() {
        let mut app = app_with_right_panel_files(vec![
            rp_file("/a/a.mp3", None, None, Some("Rock Song"), None),
            rp_file("/b/b.mp3", None, None, None, Some("Rock")),
        ]);
        app.search_query = "rock".to_string();
        // Start in Title mode — verify title matching
        app.search_mode = TextSearchMode::Title;
        let _ =
            update(&mut app, Message::SearchQueryChanged("rock".to_string()));
        let title_filtered = app.filtered_right_panel_files.clone();
        // Only "Rock Song" matches title
        assert_eq!(title_filtered.len(), 1);

        // Toggle once: Title → Genre
        let _ = update(&mut app, Message::ToggleSearchMode);
        assert_eq!(app.search_mode, TextSearchMode::Genre);

        let genre_filtered = app.filtered_right_panel_files.clone();
        // Only the file with genre "Rock" matches
        assert_eq!(genre_filtered.len(), 1);
        assert!(title_filtered != genre_filtered);
    }

    #[test]
    fn test_toggle_search_mode_changes_filtered_tag_roots() {
        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.tag_tree_roots = vec![TagTreeNode {
            label: "Jazz".to_string(),
            children: vec![],
            file_paths: vec![PathBuf::from("/music/jazz/track.mp3")],
            is_expanded: false,
            file_count: 1,
        }];
        app.search_query = "jazz".to_string();
        // Start in Genre mode — label "Jazz" matches
        app.search_mode = TextSearchMode::Genre;
        let _ =
            update(&mut app, Message::SearchQueryChanged("jazz".to_string()));
        let genre_filtered = app.filtered_tag_tree_roots.clone();
        assert_eq!(genre_filtered.len(), 1);

        // Toggle to DirectoryPath — label still matches, but also
        // checks path (which also matches). Still 1 result.
        // Genre → All → Dir (2 toggles from Genre)
        let _ = update(&mut app, Message::ToggleSearchMode);
        let _ = update(&mut app, Message::ToggleSearchMode);
        assert_eq!(app.search_mode, TextSearchMode::DirectoryPath);
        let path_filtered = app.filtered_tag_tree_roots.clone();
        assert_eq!(path_filtered.len(), 1);

        // Now search for something in the file path but NOT in the label
        app.search_mode = TextSearchMode::Genre;
        app.search_query = "track".to_string();
        let _ =
            update(&mut app, Message::SearchQueryChanged("track".to_string()));
        // Genre mode checks labels only — "track" not in label
        assert!(app.filtered_tag_tree_roots.is_empty());

        // Toggle to DirectoryPath mode — checks file path too
        // Genre → All (1), All → Dir (2)
        let _ = update(&mut app, Message::ToggleSearchMode);
        let _ = update(&mut app, Message::ToggleSearchMode);
        assert_eq!(app.search_mode, TextSearchMode::DirectoryPath);
        // Now "track" is in the file path
        assert_eq!(app.filtered_tag_tree_roots.len(), 1);
    }
}
