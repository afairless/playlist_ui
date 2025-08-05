use std::collections::HashSet;
use std::path::PathBuf;
use iced::Task;
use rfd::FileDialog;
use crate::fs::file_tree::{FileNode, NodeType, scan_directory};
use crate::fs::media_metadata::extract_media_metadata;
use crate::gui::{FileTreeApp, Message, SortColumn, SortOrder, RightPanelFile};
 
pub fn restore_expansion_state(node: &mut FileNode, expanded_dirs: &HashSet<PathBuf>) {
    node.is_expanded = expanded_dirs.contains(&node.path);
    for child in &mut node.children {
        restore_expansion_state(child, expanded_dirs);
    }
}

fn collect_files_recursively(node: &FileNode, files: &mut Vec<PathBuf>) {
    match node.node_type {
        NodeType::File => files.push(node.path.clone()),
        NodeType::Directory => {
            for child in &node.children {
                collect_files_recursively(child, files);
            }
        }
    }
}

fn find_node_by_path<'a>(node: &'a FileNode, path: &PathBuf) -> Option<&'a FileNode> {
    if &node.path == path {
        return Some(node);
    }
    for child in &node.children {
        if let Some(found) = find_node_by_path(child, path) {
            return Some(found);
        }
    }
    None
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
            if app.all_extensions.contains(&ext) {
                if app.selected_extensions.contains(&ext) {
                    app.selected_extensions.retain(|e| e != &ext);
                } else {
                    app.selected_extensions.push(ext.clone());
                }
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
        Message::AddToRightPanel(path) => {
            if !app.right_panel_files.iter().any(|f| f.path == path) {
                let meta = extract_media_metadata(&path);
                app.right_panel_files.push(RightPanelFile {
                    path,
                    musician: meta.musician,
                    album: meta.album,
                    title: meta.title,
                    genre: meta.genre,
                });
            }
            Task::none()
        }
        Message::AddDirectoryToRightPanel(dir_path) => {
            for root in app.root_nodes.iter().flatten() {
                if let Some(node) = find_node_by_path(root, &dir_path) {
                    let mut files = Vec::new();
                    collect_files_recursively(node, &mut files);
                    for file in files {
                        if !app.right_panel_files.iter().any(|f| f.path == file) {
                            let meta = extract_media_metadata(&file);
                            app.right_panel_files.push(RightPanelFile {
                                path: file,
                                musician: meta.musician,
                                album: meta.album,
                                title: meta.title,
                                genre: meta.genre,
                            });
                        }
                    }
                }
            }
            Task::none()
        }
        Message::RemoveFromRightPanel(path) => {
            app.right_panel_files.retain(|f| f.path != path);
            Task::none()
        }
        Message::RemoveDirectoryFromRightPanel(dir_path) => {
            app.right_panel_files.retain(|file| {
                // Remove if file is not in dir_path or its subdirectories
                !file.path.starts_with(&dir_path)
            });
            Task::none()
        }
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
        }
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
        }
        Message::SortRightPanelByMusician => {
            if app.right_panel_sort_column == SortColumn::Musician {
                app.right_panel_sort_order = match app.right_panel_sort_order {
                    SortOrder::Asc => SortOrder::Desc,
                    SortOrder::Desc => SortOrder::Asc,
                };
            } else {
                app.right_panel_sort_column = SortColumn::Musician;
                app.right_panel_sort_order = SortOrder::Asc;
            }
            app.right_panel_shuffled = false;
            Task::none()
        }
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
        }
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
        }
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
        }
        Message::ShuffleRightPanel => {
            use rand::seq::SliceRandom;
            let mut rng = rand::rng();
            app.right_panel_files.shuffle(&mut rng);
            app.right_panel_shuffled = true;
            Task::none()
        }
        Message::ExportRightPanelAsXspf => {
            Task::perform(
                async move { rfd::FileDialog::new().set_file_name("playlist.xspf").save_file() },
                |opt| match opt {
                    Some(path) => Message::ExportRightPanelAsXspfTo(path),
                    None => Message::ToggleExtensionsMenu, // no-op or feedback
                }
            )
        }
        Message::ExportRightPanelAsXspfTo(path) => {
            let audio_exts: &Vec<String> = &app.audio_extensions;
            let audio_files: Vec<RightPanelFile> = app.sorted_right_panel_files()
                .into_iter()
                .filter(|f| {
                    f.path.extension()
                        .and_then(|e| e.to_str())
                        .map(|ext| audio_exts.iter().any(|ae| ae == ext))
                        .unwrap_or(false)
                })
                .collect();
            let _ = crate::fs::xspf::export_xspf_playlist(&audio_files, &path);
            Task::none()
        }
        Message::ExportAndPlayRightPanelAsXspf => {
            use std::env::temp_dir;
            use std::process::Command;

            let audio_exts: &Vec<String> = &app.audio_extensions;
            let audio_files: Vec<RightPanelFile> = app.sorted_right_panel_files()
                .into_iter()
                .filter(|f| {
                    f.path.extension()
                        .and_then(|e| e.to_str())
                        .map(|ext| audio_exts.iter().any(|ae| ae == ext))
                        .unwrap_or(false)
                })
                .collect();

            let xspf_path = temp_dir().join("playlist.xspf");
            let _ = crate::fs::xspf::export_xspf_playlist(&audio_files, &xspf_path);

            // Launch VLC with the playlist
            let _ = Command::new("vlc")
                .arg(xspf_path.to_str().unwrap())
                .spawn();

            Task::none()
        }
    }
}

