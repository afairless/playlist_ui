// trying async didn't consistently improve tree construction speed
// leaving module here for possible future use

use crate::fs::media_metadata::MediaMetadata;
use crate::fs::media_metadata::extract_media_metadata;
use crate::gui::TagTreeNode;
use futures::future::join_all;
use std::collections::BTreeMap;
use std::path::PathBuf;
use tokio;
use walkdir::WalkDir;

pub async fn extract_media_metadata_async(path: PathBuf) -> MediaMetadata {
    tokio::task::spawn_blocking(move || extract_media_metadata(&path))
        .await
        .unwrap_or_else(|_| MediaMetadata::default())
}

pub async fn build_tag_genre_tree_async(
    top_dirs: &[PathBuf],
    allowed_extensions: &[String],
) -> Vec<TagTreeNode> {
    let mut file_paths = Vec::new();
    for dir in top_dirs {
        for entry in WalkDir::new(dir).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if allowed_extensions.iter().any(|ae| ae == ext) {
                        file_paths.push(path.to_path_buf());
                    }
                }
            }
        }
    }

    let tasks = file_paths.iter().map(|path| {
        let path_clone = path.clone();
        async move {
            let meta = extract_media_metadata_async(path_clone.clone()).await;
            (path_clone, meta)
        }
    });
    let results = join_all(tasks).await;

    let mut genre_map: BTreeMap<
        String,
        BTreeMap<String, BTreeMap<String, Vec<(String, PathBuf)>>>,
    > = BTreeMap::new();

    for (path, meta) in results {
        let genre = meta.genre.unwrap_or_else(|| "Unknown".to_string());
        let artist = meta.musician.unwrap_or_else(|| "Unknown".to_string());
        let album = meta.album.unwrap_or_else(|| "Unknown".to_string());
        let title = meta.title.clone().unwrap_or_else(|| {
            path.file_name().unwrap().to_string_lossy().to_string()
        });
        genre_map
            .entry(genre)
            .or_default()
            .entry(artist)
            .or_default()
            .entry(album)
            .or_default()
            .push((title, path));
    }

    // (Tree construction code remains unchanged)
    let mut roots = Vec::new();
    for (genre, artists) in genre_map {
        let mut artist_nodes = Vec::new();
        for (artist, albums) in artists {
            let mut album_nodes = Vec::new();
            for (album, tracks) in albums {
                let mut track_nodes = Vec::new();
                for (title, path) in tracks {
                    track_nodes.push(TagTreeNode {
                        label: title,
                        children: vec![],
                        file_paths: vec![path],
                        is_expanded: false,
                    });
                }
                album_nodes.push(TagTreeNode {
                    label: album,
                    children: track_nodes,
                    file_paths: vec![],
                    is_expanded: false,
                });
            }
            artist_nodes.push(TagTreeNode {
                label: artist,
                children: album_nodes,
                file_paths: vec![],
                is_expanded: false,
            });
        }
        roots.push(TagTreeNode {
            label: genre,
            children: artist_nodes,
            file_paths: vec![],
            is_expanded: false,
        });
    }
    roots
}

pub async fn build_tag_musician_tree_async(
    top_dirs: &[PathBuf],
    allowed_extensions: &[String],
) -> Vec<TagTreeNode> {
    let mut file_paths = Vec::new();
    for dir in top_dirs {
        for entry in WalkDir::new(dir).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if allowed_extensions.iter().any(|ae| ae == ext) {
                        file_paths.push(path.to_path_buf());
                    }
                }
            }
        }
    }

    let tasks = file_paths.iter().map(|path| {
        let path_clone = path.clone();
        async move {
            let meta = extract_media_metadata_async(path_clone.clone()).await;
            (path_clone, meta)
        }
    });
    let results = join_all(tasks).await;

    let mut musician_map: BTreeMap<
        String,
        BTreeMap<String, Vec<(String, PathBuf)>>,
    > = BTreeMap::new();

    for (path, meta) in results {
        let artist = meta.musician.unwrap_or_else(|| "Unknown".to_string());
        let album = meta.album.unwrap_or_else(|| "Unknown".to_string());
        let title = meta.title.clone().unwrap_or_else(|| {
            path.file_name().unwrap().to_string_lossy().to_string()
        });
        musician_map
            .entry(artist)
            .or_default()
            .entry(album)
            .or_default()
            .push((title, path));
    }

    let mut roots = Vec::new();
    for (artist, albums) in musician_map {
        let mut album_nodes = Vec::new();
        for (album, tracks) in albums {
            let mut track_nodes = Vec::new();
            for (title, path) in tracks {
                track_nodes.push(TagTreeNode {
                    label: title,
                    children: vec![],
                    file_paths: vec![path],
                    is_expanded: false,
                });
            }
            album_nodes.push(TagTreeNode {
                label: album,
                children: track_nodes,
                file_paths: vec![],
                is_expanded: false,
            });
        }
        roots.push(TagTreeNode {
            label: artist,
            children: album_nodes,
            file_paths: vec![],
            is_expanded: false,
        });
    }
    roots
}
