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
use crate::gui::tantivy_search::{
    build_tantivy_index, prune_file_tree, prune_tag_node,
};
use crate::gui::{
    FileTreeApp, LeftPanelSelectMode, LeftPanelSortMode, Message,
    RightPanelFile, SortColumn, SortOrder, TagTreeNode, TextSearchMode,
};
use iced::Task;
use rfd::FileDialog;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Recomputes `filtered_root_nodes` from `app.root_nodes` using the current
/// search query and mode. Called whenever the query string or search mode
/// changes.
#[allow(dead_code)]
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

#[allow(dead_code)]
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

/// Returns all files in the right panel, sorted according to the
/// current sort settings. The right panel is the cumulative playlist
/// and is never filtered by the search query.
fn displayed_right_panel_files(app: &FileTreeApp) -> Vec<RightPanelFile> {
    let mut files = app.right_panel_files.clone();
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
            // Sync expansion state into the unfiltered tree.
            for root in app.root_nodes.iter_mut().flatten() {
                restore_expansion_state(root, &app.expanded_dirs);
            }
            // Sync expansion state into the already-filtered tree in-place.
            // No re-filter is needed — the filter query hasn't changed and
            // filter_{file,tag}_node do not inspect is_expanded.
            for filtered in app.filtered_root_nodes.iter_mut().flatten() {
                restore_expansion_state(filtered, &app.expanded_dirs);
            }
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
            app.tantivy_index = Some(build_tantivy_index(&app.root_nodes));
            if !app.search_query.is_empty() {
                app.perform_search();
            } else {
                app.filtered_root_nodes = app.root_nodes.clone();
                app.filtered_tag_tree_roots = app.tag_tree_roots.clone();
            }
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
                app.tantivy_index = Some(build_tantivy_index(&app.root_nodes));
                if !app.search_query.is_empty() {
                    app.perform_search();
                } else {
                    app.filtered_root_nodes = app.root_nodes.clone();
                    app.filtered_tag_tree_roots = app.tag_tree_roots.clone();
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
                app.tantivy_index = Some(build_tantivy_index(&app.root_nodes));
                if !app.search_query.is_empty() {
                    app.perform_search();
                } else {
                    app.filtered_root_nodes = app.root_nodes.clone();
                    app.filtered_tag_tree_roots = app.tag_tree_roots.clone();
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
                    // Filter files by active search, if any
                    if let Some(ref matches) = app.last_search_matches {
                        files.retain(|f| matches.contains(f));
                    }
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
            app.last_search_matches = None;
            app.filtered_root_nodes = app.root_nodes.clone();
            app.filtered_tag_tree_roots = app.tag_tree_roots.clone();
            Task::none()
        },
        Message::SearchQueryChanged(query) => {
            app.search_query = query;
            if app.search_query.is_empty() {
                app.last_search_matches = None;
                app.filtered_root_nodes = app.root_nodes.clone();
                app.filtered_tag_tree_roots = app.tag_tree_roots.clone();
            } else {
                app.perform_search();
            }
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
            if !app.search_query.is_empty() {
                app.perform_search();
            }
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
            // Re-apply search filter if active
            if !app.search_query.is_empty() {
                if let Some(ref matches) = app.last_search_matches.clone() {
                    app.filtered_root_nodes = app
                        .root_nodes
                        .iter()
                        .map(|node_opt| {
                            node_opt.as_ref().and_then(|n| {
                                prune_file_tree(
                                    n,
                                    matches,
                                    &app.search_query,
                                    app.search_mode,
                                )
                            })
                        })
                        .collect();
                    app.filtered_tag_tree_roots = app
                        .tag_tree_roots
                        .iter()
                        .filter_map(|n| prune_tag_node(n, matches))
                        .collect();
                } else {
                    app.perform_search();
                }
            }
            Task::none()
        },
        Message::ToggleTagExpansion(path) => {
            // Toggle in the unfiltered tree and capture the new state.
            let new_state = if let Some(node) =
                find_tag_node_mut(&mut app.tag_tree_roots, &path)
            {
                node.is_expanded = !node.is_expanded;
                node.is_expanded
            } else {
                return Task::none();
            };
            // Apply the same toggle to the already-filtered tree in-place.
            // The filtered tree shares the same path structure as the
            // unfiltered tree; nodes pruned by the filter are simply absent.
            if let Some(node) =
                find_tag_node_mut(&mut app.filtered_tag_tree_roots, &path)
            {
                node.is_expanded = new_state;
            }
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
                // Filter files by active search, if any
                if let Some(ref matches) = app.last_search_matches {
                    files.retain(|f| matches.contains(f));
                }
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
        Message::RandomCountChanged(new_text) => {
            if new_text.is_empty() {
                // Allow empty input so the user can clear and retype
                app.random_count_input = new_text;
            } else if let Ok(n) = new_text.parse::<usize>() {
                if n > 0 {
                    app.random_count = n;
                    app.random_count_input = new_text;
                } else {
                    // 0 is not a positive integer — revert
                    app.random_count_input = app.random_count.to_string();
                }
            } else {
                // Not a valid integer — revert
                app.random_count_input = app.random_count.to_string();
            }
            Task::none()
        },
        Message::AddRandomTagNodeToRightPanel(path) => {
            app.right_panel_shuffled = false;
            if let Some(node) =
                find_tag_node_mut(&mut app.tag_tree_roots, &path)
            {
                let mut files = Vec::new();
                collect_tag_node_files(node, &mut files);
                // Filter by active search, if any
                if let Some(ref matches) = app.last_search_matches {
                    files.retain(|f| matches.contains(f));
                }
                // Random subset
                let n = app.random_count.min(files.len());
                if n < files.len() {
                    use rand::seq::SliceRandom;
                    let mut rng = rand::rng();
                    files.partial_shuffle(&mut rng, n);
                    files.truncate(n);
                }
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
        Message::AddRandomDirectoryToRightPanel(dir_path) => {
            app.right_panel_shuffled = false;
            for root in app.root_nodes.iter().flatten() {
                if let Some(node) = find_node_by_path(root, &dir_path) {
                    let mut files = Vec::new();
                    collect_files_recursively(node, &mut files);
                    // Filter by active search, if any
                    if let Some(ref matches) = app.last_search_matches {
                        files.retain(|f| matches.contains(f));
                    }
                    // Random subset
                    let n = app.random_count.min(files.len());
                    if n < files.len() {
                        use rand::seq::SliceRandom;
                        let mut rng = rand::rng();
                        files.partial_shuffle(&mut rng, n);
                        files.truncate(n);
                    }
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
            file_paths: vec![PathBuf::from("/dummy/jazz.mp3")],
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

        // With tantivy, no files are indexed so filtered tag tree is empty
        assert_eq!(app.filtered_tag_tree_roots.len(), 0);
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

    // ── right-panel tests (search-ignorance) ────────────────────────

    /// File added before search remains visible after search activated.
    #[test]
    fn test_right_panel_shows_all_files_regardless_of_search() {
        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );

        // Add file A while no search is active
        let _ = update(
            &mut app,
            Message::AddToRightPanel(PathBuf::from("/music/song_a.mp3")),
        );
        assert_eq!(app.right_panel_files.len(), 1);

        // Activate a search — this should NOT affect right_panel_files
        let _ = update(
            &mut app,
            Message::SearchQueryChanged("zzz_nonexistent".to_string()),
        );

        // The file should still be in the playlist
        assert_eq!(
            app.right_panel_files.len(),
            1,
            "playlist should retain all files despite search"
        );

        // sorted_right_panel_files() should return the file
        assert_eq!(app.sorted_right_panel_files().len(), 1);
    }

    // ── AddDirectoryToRightPanel search-filter tests ────────────────

    /// With an active search, AddDirectoryToRightPanel should only add
    /// files matching the search results.
    #[test]
    fn test_add_directory_filtered_by_search() {
        let dir_path = PathBuf::from("/music");
        let file_a = PathBuf::from("/music/song_a.mp3");
        let file_b = PathBuf::from("/music/song_b.mp3");
        let file_c = PathBuf::from("/music/song_c.mp3");

        let dir_node = FileNode::new_directory(
            "music".to_string(),
            dir_path.clone(),
            vec![
                FileNode::new_file("song_a.mp3".to_string(), file_a.clone()),
                FileNode::new_file("song_b.mp3".to_string(), file_b.clone()),
                FileNode::new_file("song_c.mp3".to_string(), file_c.clone()),
            ],
        );

        let mut app = FileTreeApp::new(
            vec![dir_path.clone()],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.root_nodes[0] = Some(dir_node);

        // Activate a search that matches only file_a and file_b
        let mut matches = HashSet::new();
        matches.insert(file_a.clone());
        matches.insert(file_b.clone());
        app.last_search_matches = Some(matches);

        let msg = Message::AddDirectoryToRightPanel(dir_path);
        let _ = update(&mut app, msg);

        assert_eq!(
            app.right_panel_files.len(),
            2,
            "only search-matching files should be added"
        );
        assert!(app.right_panel_files.iter().any(|f| f.path == file_a));
        assert!(app.right_panel_files.iter().any(|f| f.path == file_b));
    }

    /// Without an active search, AddDirectoryToRightPanel should add all
    /// files (no regression).
    #[test]
    fn test_add_directory_without_search_adds_all() {
        let dir_path = PathBuf::from("/music");
        let file_a = PathBuf::from("/music/song_a.mp3");
        let file_b = PathBuf::from("/music/song_b.mp3");

        let dir_node = FileNode::new_directory(
            "music".to_string(),
            dir_path.clone(),
            vec![
                FileNode::new_file("song_a.mp3".to_string(), file_a.clone()),
                FileNode::new_file("song_b.mp3".to_string(), file_b.clone()),
            ],
        );

        let mut app = FileTreeApp::new(
            vec![dir_path.clone()],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.root_nodes[0] = Some(dir_node);

        // No search activated — last_search_matches is None
        app.last_search_matches = None;

        let msg = Message::AddDirectoryToRightPanel(dir_path);
        let _ = update(&mut app, msg);

        assert_eq!(
            app.right_panel_files.len(),
            2,
            "all files should be added when no search is active"
        );
    }

    // ── AddRandomDirectoryToRightPanel tests ──────────────────────────

    #[test]
    fn test_add_random_directory_selects_subset() {
        let dir_path = PathBuf::from("/music");
        let file_a = PathBuf::from("/music/song_a.mp3");
        let file_b = PathBuf::from("/music/song_b.mp3");
        let file_c = PathBuf::from("/music/song_c.mp3");

        let dir_node = FileNode::new_directory(
            "music".to_string(),
            dir_path.clone(),
            vec![
                FileNode::new_file("song_a.mp3".to_string(), file_a.clone()),
                FileNode::new_file("song_b.mp3".to_string(), file_b.clone()),
                FileNode::new_file("song_c.mp3".to_string(), file_c.clone()),
            ],
        );

        let mut app = FileTreeApp::new(
            vec![dir_path.clone()],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.root_nodes[0] = Some(dir_node);
        app.random_count = 2;

        let msg = Message::AddRandomDirectoryToRightPanel(dir_path);
        let _ = update(&mut app, msg);

        assert_eq!(app.right_panel_files.len(), 2);
        assert!(!app.right_panel_shuffled);
    }

    #[test]
    fn test_add_random_directory_all_when_n_exceeds() {
        let dir_path = PathBuf::from("/music");
        let file_a = PathBuf::from("/music/song_a.mp3");
        let file_b = PathBuf::from("/music/song_b.mp3");

        let dir_node = FileNode::new_directory(
            "music".to_string(),
            dir_path.clone(),
            vec![
                FileNode::new_file("song_a.mp3".to_string(), file_a.clone()),
                FileNode::new_file("song_b.mp3".to_string(), file_b.clone()),
            ],
        );

        let mut app = FileTreeApp::new(
            vec![dir_path.clone()],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.root_nodes[0] = Some(dir_node);
        // N exceeds file count — should add all
        app.random_count = 10;

        let msg = Message::AddRandomDirectoryToRightPanel(dir_path);
        let _ = update(&mut app, msg);

        assert_eq!(app.right_panel_files.len(), 2);
    }

    #[test]
    fn test_add_random_directory_respects_search_filter() {
        let dir_path = PathBuf::from("/music");
        let file_a = PathBuf::from("/music/song_a.mp3");
        let file_b = PathBuf::from("/music/song_b.mp3");
        let file_c = PathBuf::from("/music/song_c.mp3");

        let dir_node = FileNode::new_directory(
            "music".to_string(),
            dir_path.clone(),
            vec![
                FileNode::new_file("song_a.mp3".to_string(), file_a.clone()),
                FileNode::new_file("song_b.mp3".to_string(), file_b.clone()),
                FileNode::new_file("song_c.mp3".to_string(), file_c.clone()),
            ],
        );

        let mut app = FileTreeApp::new(
            vec![dir_path.clone()],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.root_nodes[0] = Some(dir_node);
        app.random_count = 5;

        // Only file_a and file_b match the search
        let mut matches = HashSet::new();
        matches.insert(file_a.clone());
        matches.insert(file_b.clone());
        app.last_search_matches = Some(matches);

        let msg = Message::AddRandomDirectoryToRightPanel(dir_path);
        let _ = update(&mut app, msg);

        // After filtering, only 2 files remain, and N=5 exceeds 2, so both
        assert_eq!(app.right_panel_files.len(), 2);
    }

    #[test]
    fn test_add_random_directory_no_filter_when_search_inactive() {
        let dir_path = PathBuf::from("/music");
        let file_a = PathBuf::from("/music/song_a.mp3");
        let file_b = PathBuf::from("/music/song_b.mp3");
        let file_c = PathBuf::from("/music/song_c.mp3");

        let dir_node = FileNode::new_directory(
            "music".to_string(),
            dir_path.clone(),
            vec![
                FileNode::new_file("song_a.mp3".to_string(), file_a.clone()),
                FileNode::new_file("song_b.mp3".to_string(), file_b.clone()),
                FileNode::new_file("song_c.mp3".to_string(), file_c.clone()),
            ],
        );

        let mut app = FileTreeApp::new(
            vec![dir_path.clone()],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.root_nodes[0] = Some(dir_node);
        app.random_count = 5;
        app.last_search_matches = None;

        let msg = Message::AddRandomDirectoryToRightPanel(dir_path);
        let _ = update(&mut app, msg);

        // No search filter — all 3 files pass through, N=5 exceeds count
        assert_eq!(app.right_panel_files.len(), 3);
    }

    #[test]
    fn test_add_random_directory_no_duplicates() {
        let dir_path = PathBuf::from("/music");
        let file_a = PathBuf::from("/music/song_a.mp3");

        let dir_node = FileNode::new_directory(
            "music".to_string(),
            dir_path.clone(),
            vec![FileNode::new_file("song_a.mp3".to_string(), file_a.clone())],
        );

        let mut app = FileTreeApp::new(
            vec![dir_path.clone()],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.root_nodes[0] = Some(dir_node);
        app.random_count = 5;

        // Add file_a first via AddToRightPanel
        let _ = update(&mut app, Message::AddToRightPanel(file_a.clone()));
        assert_eq!(app.right_panel_files.len(), 1);

        let msg = Message::AddRandomDirectoryToRightPanel(dir_path);
        let _ = update(&mut app, msg);

        // Should still be 1 — no duplicates added
        assert_eq!(app.right_panel_files.len(), 1);
    }

    #[test]
    fn test_add_random_directory_n_zero_adds_none() {
        let dir_path = PathBuf::from("/music");
        let file_a = PathBuf::from("/music/song_a.mp3");

        let dir_node = FileNode::new_directory(
            "music".to_string(),
            dir_path.clone(),
            vec![FileNode::new_file("song_a.mp3".to_string(), file_a.clone())],
        );

        let mut app = FileTreeApp::new(
            vec![dir_path.clone()],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.root_nodes[0] = Some(dir_node);
        app.random_count = 0;

        let msg = Message::AddRandomDirectoryToRightPanel(dir_path);
        let _ = update(&mut app, msg);

        // n = min(0, 1) = 0, so no files should be added
        assert_eq!(app.right_panel_files.len(), 0);
    }

    // ── AddTagNodeToRightPanel search-filter tests ──────────────────

    /// With an active search, AddTagNodeToRightPanel should only add files
    /// matching the search results.
    #[test]
    fn test_add_tag_node_filtered_by_search() {
        let track_1 = PathBuf::from("/music/jazz/track_1.mp3");
        let track_2 = PathBuf::from("/music/jazz/track_2.mp3");
        let track_3 = PathBuf::from("/music/rock/track_3.mp3");

        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.tag_tree_roots = vec![TagTreeNode {
            label: "Genre".to_string(),
            children: vec![
                TagTreeNode {
                    label: "Jazz".to_string(),
                    children: vec![],
                    file_paths: vec![track_1.clone(), track_2.clone()],
                    is_expanded: false,
                    file_count: 2,
                },
                TagTreeNode {
                    label: "Rock".to_string(),
                    children: vec![],
                    file_paths: vec![track_3.clone()],
                    is_expanded: false,
                    file_count: 1,
                },
            ],
            file_paths: vec![],
            is_expanded: false,
            file_count: 3,
        }];

        // Activate a search that matches only track_1 and track_2
        let mut matches = HashSet::new();
        matches.insert(track_1.clone());
        matches.insert(track_2.clone());
        app.last_search_matches = Some(matches);

        let path = vec!["Genre".to_string()];
        let msg = Message::AddTagNodeToRightPanel(path);
        let _ = update(&mut app, msg);

        assert_eq!(
            app.right_panel_files.len(),
            2,
            "only search-matching tracks should be added"
        );
        assert!(app.right_panel_files.iter().any(|f| f.path == track_1));
        assert!(app.right_panel_files.iter().any(|f| f.path == track_2));
    }

    /// Without an active search, AddTagNodeToRightPanel should add all
    /// files (no regression).
    #[test]
    fn test_add_tag_node_without_search_adds_all() {
        let track_1 = PathBuf::from("/music/jazz/track_1.mp3");
        let track_2 = PathBuf::from("/music/rock/track_2.mp3");

        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.tag_tree_roots = vec![TagTreeNode {
            label: "Genre".to_string(),
            children: vec![
                TagTreeNode {
                    label: "Jazz".to_string(),
                    children: vec![],
                    file_paths: vec![track_1.clone()],
                    is_expanded: false,
                    file_count: 1,
                },
                TagTreeNode {
                    label: "Rock".to_string(),
                    children: vec![],
                    file_paths: vec![track_2.clone()],
                    is_expanded: false,
                    file_count: 1,
                },
            ],
            file_paths: vec![],
            is_expanded: false,
            file_count: 2,
        }];

        // No search activated
        app.last_search_matches = None;

        let path = vec!["Genre".to_string()];
        let msg = Message::AddTagNodeToRightPanel(path);
        let _ = update(&mut app, msg);

        assert_eq!(
            app.right_panel_files.len(),
            2,
            "all tracks should be added when no search is active"
        );
    }

    /// With an active search that matches no files, AddTagNodeToRightPanel
    /// should add zero files (not fall through to adding everything).
    #[test]
    fn test_add_tag_node_empty_search_adds_none() {
        let track = PathBuf::from("/music/jazz/track.mp3");

        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.tag_tree_roots = vec![TagTreeNode {
            label: "Genre".to_string(),
            children: vec![TagTreeNode {
                label: "Jazz".to_string(),
                children: vec![],
                file_paths: vec![track.clone()],
                is_expanded: false,
                file_count: 1,
            }],
            file_paths: vec![],
            is_expanded: false,
            file_count: 1,
        }];

        // Empty search results — last_search_matches is Some(empty set)
        app.last_search_matches = Some(HashSet::new());

        let path = vec!["Genre".to_string()];
        let msg = Message::AddTagNodeToRightPanel(path);
        let _ = update(&mut app, msg);

        assert_eq!(
            app.right_panel_files.len(),
            0,
            "zero files should be added when search matches nothing"
        );
    }

    /// displayed_right_panel_files always returns all files regardless of
    /// search state. Covers both the pre-search state (last_search_matches
    /// is None) and the active-search state (last_search_matches exists but
    /// contains no matches for the playlist file).
    #[test]
    fn test_displayed_right_panel_files_ignores_search() {
        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );

        let _ = update(
            &mut app,
            Message::AddToRightPanel(PathBuf::from("/music/song.mp3")),
        );
        assert_eq!(app.right_panel_files.len(), 1);

        // Pre-search state: last_search_matches is None (no tantivy search
        // has run yet). displayed_right_panel_files should still return all
        // files.
        app.search_query = "something".to_string();
        assert!(
            app.last_search_matches.is_none(),
            "pre-search state: no matches cached"
        );
        let displayed = displayed_right_panel_files(&app);
        assert_eq!(
            displayed.len(),
            1,
            "displayed files should ignore pre-search state"
        );

        // Active-search state: last_search_matches exists but is empty.
        app.last_search_matches = Some(HashSet::new());
        let displayed = displayed_right_panel_files(&app);
        assert_eq!(
            displayed.len(),
            1,
            "displayed files should ignore active search state"
        );
    }

    #[test]
    fn test_toggle_expansion_preserves_filtered_structure() {
        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        let dir = FileNode::new_directory(
            "Music".to_string(),
            PathBuf::from("/Music"),
            vec![
                FileNode::new_file(
                    "rock.mp3".to_string(),
                    PathBuf::from("/Music/rock.mp3"),
                ),
                FileNode::new_file(
                    "jazz.mp3".to_string(),
                    PathBuf::from("/Music/jazz.mp3"),
                ),
            ],
        );
        app.root_nodes = vec![Some(dir)];
        app.search_query = "rock".to_string();
        app.filtered_root_nodes = recompute_filtered_nodes(&app);

        // Capture the structure before toggle
        let child_count_before =
            app.filtered_root_nodes[0].as_ref().unwrap().children.len();
        let file_count_before =
            app.filtered_root_nodes[0].as_ref().unwrap().file_count;

        // Toggle expansion
        let _ =
            update(&mut app, Message::ToggleExpansion(PathBuf::from("/Music")));

        // Structure must be unchanged — same children, same file count
        let filtered = app.filtered_root_nodes[0].as_ref().unwrap();
        assert_eq!(filtered.children.len(), child_count_before);
        assert_eq!(filtered.file_count, file_count_before);
        // Only expansion flag should have changed
        assert!(filtered.is_expanded);
    }

    #[test]
    fn test_toggle_tag_expansion_preserves_filtered_structure() {
        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.tag_tree_roots = vec![TagTreeNode {
            label: "Parent".to_string(),
            children: vec![
                TagTreeNode {
                    label: "Rock".to_string(),
                    children: vec![],
                    file_paths: vec![PathBuf::from("/a.mp3")],
                    is_expanded: false,
                    file_count: 1,
                },
                TagTreeNode {
                    label: "Jazz".to_string(),
                    children: vec![],
                    file_paths: vec![PathBuf::from("/b.mp3")],
                    is_expanded: false,
                    file_count: 1,
                },
            ],
            file_paths: vec![],
            is_expanded: false,
            file_count: 2,
        }];
        app.search_query = "Rock".to_string();
        app.filtered_tag_tree_roots = recompute_filtered_tag_nodes(&app);

        let child_count_before = app.filtered_tag_tree_roots[0].children.len();
        let file_count_before = app.filtered_tag_tree_roots[0].file_count;

        let path = vec!["Parent".to_string()];
        let _ = update(&mut app, Message::ToggleTagExpansion(path));

        // Structure must be unchanged
        assert_eq!(
            app.filtered_tag_tree_roots[0].children.len(),
            child_count_before
        );
        assert_eq!(
            app.filtered_tag_tree_roots[0].file_count,
            file_count_before
        );
        assert!(app.filtered_tag_tree_roots[0].is_expanded);
        // Matching child still present, non-matching child still absent
        assert_eq!(
            app.filtered_tag_tree_roots[0]
                .children
                .iter()
                .map(|c| c.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Rock"]
        );
    }

    // ── AddRandomTagNodeToRightPanel tests ──────────────────────────────

    #[test]
    fn test_add_random_tag_node_selects_subset() {
        let track_1 = PathBuf::from("/music/jazz/track_1.mp3");
        let track_2 = PathBuf::from("/music/jazz/track_2.mp3");
        let track_3 = PathBuf::from("/music/jazz/track_3.mp3");

        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.tag_tree_roots = vec![TagTreeNode {
            label: "Genre".to_string(),
            children: vec![TagTreeNode {
                label: "Jazz".to_string(),
                children: vec![],
                file_paths: vec![
                    track_1.clone(),
                    track_2.clone(),
                    track_3.clone(),
                ],
                is_expanded: false,
                file_count: 3,
            }],
            file_paths: vec![],
            is_expanded: false,
            file_count: 3,
        }];
        app.random_count = 2;

        let path = vec!["Genre".to_string(), "Jazz".to_string()];
        let msg = Message::AddRandomTagNodeToRightPanel(path);
        let _ = update(&mut app, msg);

        assert_eq!(app.right_panel_files.len(), 2);
        assert!(!app.right_panel_shuffled);
    }

    #[test]
    fn test_add_random_tag_node_all_when_n_exceeds() {
        let track_1 = PathBuf::from("/music/jazz/track_1.mp3");
        let track_2 = PathBuf::from("/music/jazz/track_2.mp3");

        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.tag_tree_roots = vec![TagTreeNode {
            label: "Genre".to_string(),
            children: vec![TagTreeNode {
                label: "Jazz".to_string(),
                children: vec![],
                file_paths: vec![track_1.clone(), track_2.clone()],
                is_expanded: false,
                file_count: 2,
            }],
            file_paths: vec![],
            is_expanded: false,
            file_count: 2,
        }];
        app.random_count = 10;

        let path = vec!["Genre".to_string(), "Jazz".to_string()];
        let msg = Message::AddRandomTagNodeToRightPanel(path);
        let _ = update(&mut app, msg);

        assert_eq!(app.right_panel_files.len(), 2);
    }

    #[test]
    fn test_add_random_tag_node_respects_search_filter() {
        let track_1 = PathBuf::from("/music/jazz/track_1.mp3");
        let track_2 = PathBuf::from("/music/jazz/track_2.mp3");
        let track_3 = PathBuf::from("/music/rock/track_3.mp3");

        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.tag_tree_roots = vec![TagTreeNode {
            label: "Genre".to_string(),
            children: vec![
                TagTreeNode {
                    label: "Jazz".to_string(),
                    children: vec![],
                    file_paths: vec![track_1.clone(), track_2.clone()],
                    is_expanded: false,
                    file_count: 2,
                },
                TagTreeNode {
                    label: "Rock".to_string(),
                    children: vec![],
                    file_paths: vec![track_3.clone()],
                    is_expanded: false,
                    file_count: 1,
                },
            ],
            file_paths: vec![],
            is_expanded: false,
            file_count: 3,
        }];
        app.random_count = 5;

        // Only track_1 and track_2 match the search
        let mut matches = HashSet::new();
        matches.insert(track_1.clone());
        matches.insert(track_2.clone());
        app.last_search_matches = Some(matches);

        let path = vec!["Genre".to_string()];
        let msg = Message::AddRandomTagNodeToRightPanel(path);
        let _ = update(&mut app, msg);

        // After filtering, only 2 files remain, N=5 exceeds 2, so both
        assert_eq!(app.right_panel_files.len(), 2);
    }

    #[test]
    fn test_add_random_tag_node_no_filter_when_search_inactive() {
        let track_1 = PathBuf::from("/music/jazz/track_1.mp3");
        let track_2 = PathBuf::from("/music/rock/track_2.mp3");

        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.tag_tree_roots = vec![TagTreeNode {
            label: "Genre".to_string(),
            children: vec![
                TagTreeNode {
                    label: "Jazz".to_string(),
                    children: vec![],
                    file_paths: vec![track_1.clone()],
                    is_expanded: false,
                    file_count: 1,
                },
                TagTreeNode {
                    label: "Rock".to_string(),
                    children: vec![],
                    file_paths: vec![track_2.clone()],
                    is_expanded: false,
                    file_count: 1,
                },
            ],
            file_paths: vec![],
            is_expanded: false,
            file_count: 2,
        }];
        app.random_count = 5;
        app.last_search_matches = None;

        let path = vec!["Genre".to_string()];
        let msg = Message::AddRandomTagNodeToRightPanel(path);
        let _ = update(&mut app, msg);

        // No filter — all 2 files added, N=5 exceeds count
        assert_eq!(app.right_panel_files.len(), 2);
    }

    #[test]
    fn test_add_random_tag_node_no_duplicates() {
        let track = PathBuf::from("/music/jazz/track.mp3");

        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.tag_tree_roots = vec![TagTreeNode {
            label: "Genre".to_string(),
            children: vec![TagTreeNode {
                label: "Jazz".to_string(),
                children: vec![],
                file_paths: vec![track.clone()],
                is_expanded: false,
                file_count: 1,
            }],
            file_paths: vec![],
            is_expanded: false,
            file_count: 1,
        }];
        app.random_count = 5;

        // Add track first via AddToRightPanel
        let _ = update(&mut app, Message::AddToRightPanel(track.clone()));
        assert_eq!(app.right_panel_files.len(), 1);

        let path = vec!["Genre".to_string(), "Jazz".to_string()];
        let msg = Message::AddRandomTagNodeToRightPanel(path);
        let _ = update(&mut app, msg);

        // Should still be 1 — no duplicates
        assert_eq!(app.right_panel_files.len(), 1);
    }

    #[test]
    fn test_add_random_tag_node_n_zero_adds_none() {
        let track = PathBuf::from("/music/jazz/track.mp3");

        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.tag_tree_roots = vec![TagTreeNode {
            label: "Genre".to_string(),
            children: vec![TagTreeNode {
                label: "Jazz".to_string(),
                children: vec![],
                file_paths: vec![track.clone()],
                is_expanded: false,
                file_count: 1,
            }],
            file_paths: vec![],
            is_expanded: false,
            file_count: 1,
        }];
        app.random_count = 0;

        let path = vec!["Genre".to_string(), "Jazz".to_string()];
        let msg = Message::AddRandomTagNodeToRightPanel(path);
        let _ = update(&mut app, msg);

        // n = min(0, 1) = 0, so no files should be added
        assert_eq!(app.right_panel_files.len(), 0);
    }

    // ── RandomCountChanged validation tests ───────────────────────────────

    #[test]
    fn test_random_count_valid_input() {
        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        let _ = update(&mut app, Message::RandomCountChanged("12".to_string()));
        assert_eq!(app.random_count, 12);
        assert_eq!(app.random_count_input, "12");
    }

    #[test]
    fn test_random_count_invalid_text_reverts() {
        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.random_count = 5;
        app.random_count_input = "5".to_string();
        let _ =
            update(&mut app, Message::RandomCountChanged("abc".to_string()));
        assert_eq!(app.random_count, 5);
        assert_eq!(app.random_count_input, "5");
    }

    #[test]
    fn test_random_count_zero_reverts() {
        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.random_count = 3;
        app.random_count_input = "3".to_string();
        let _ = update(&mut app, Message::RandomCountChanged("0".to_string()));
        assert_eq!(app.random_count, 3);
        assert_eq!(app.random_count_input, "3");
    }

    /// When the user erases all digits, the text input shows empty while
    /// the last valid `random_count` value is preserved.
    #[test]
    fn test_random_count_empty_accepted() {
        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.random_count = 7;
        app.random_count_input = "7".to_string();
        let _ = update(&mut app, Message::RandomCountChanged("".to_string()));
        // Text input should show empty, but the numeric value stays unchanged
        assert_eq!(app.random_count_input, "");
        assert_eq!(app.random_count, 7);
    }

    /// After the field becomes empty, the previous `random_count` is
    /// preserved and can be overwritten by typing a new valid number.
    #[test]
    fn test_random_count_erase_then_type() {
        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.random_count = 6;
        app.random_count_input = "6".to_string();

        // Erase to empty
        let _ = update(&mut app, Message::RandomCountChanged("".to_string()));
        assert_eq!(app.random_count_input, "");
        assert_eq!(app.random_count, 6);

        // Type a new valid number
        let _ = update(&mut app, Message::RandomCountChanged("42".to_string()));
        assert_eq!(app.random_count_input, "42");
        assert_eq!(app.random_count, 42);
    }

    #[test]
    fn test_random_count_leading_zeros_accepted() {
        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        let _ = update(&mut app, Message::RandomCountChanged("06".to_string()));
        assert_eq!(app.random_count, 6);
        assert_eq!(app.random_count_input, "06");
    }

    #[test]
    fn test_random_count_overflow_reverts() {
        let mut app = FileTreeApp::new(
            vec![],
            &["mp3"],
            PathBuf::from("/tmp/test.json"),
            None,
        );
        app.random_count = 4;
        app.random_count_input = "4".to_string();
        let _ = update(
            &mut app,
            Message::RandomCountChanged("99999999999999999999".to_string()),
        );
        assert_eq!(app.random_count, 4);
        assert_eq!(app.random_count_input, "4");
    }
}
