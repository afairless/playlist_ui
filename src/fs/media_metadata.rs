use crate::gui::TagTreeNode;
use lofty::{
    file::{AudioFile, TaggedFileExt},
    prelude::ItemKey,
    read_from_path,
    tag::Accessor,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Default)]
pub(crate) struct MediaMetadata {
    pub creator: Option<String>,
    pub album: Option<String>,
    pub title: Option<String>,
    pub genre: Option<String>,
    pub track_num: Option<u32>,
    pub duration_ms: Option<u64>,
    pub image_uri: Option<String>,
    pub identifier: Option<String>,
    pub annotation: Option<String>,
}

/// Extracts media metadata from the given file path using the `lofty` crate,
///     returning information such as artist, album, title, genre, track number,
///     duration, album art URI, identifier, and annotation if available.
pub(crate) fn extract_media_metadata(path: &Path) -> MediaMetadata {
    if let Ok(tagged_file) = read_from_path(path) {
        let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());
        let duration_ms =
            Some(tagged_file.properties().duration().as_millis() as u64);

        let (track_num, annotation, identifier, image_uri) =
            if let Some(tag) = tag {
                // Track number
                let track_num = tag.track();

                // Annotation (comment)
                let annotation = tag.comment().map(|s| s.to_string());

                // Identifier (try MusicBrainz or ISRC)
                let identifier = tag
                    .get_string(&ItemKey::MusicBrainzTrackId)
                    .or_else(|| tag.get_string(&ItemKey::Isrc))
                    .map(|s| s.to_string());

                // Album art (save first picture if present)
                let image_uri = tag.pictures().first().and_then(|pic| {
                    let img_path = path.with_extension("cover.jpg");
                    if std::fs::write(&img_path, pic.data()).is_ok() {
                        Some(format!("file://{}", img_path.display()))
                    } else {
                        None
                    }
                });

                (track_num, annotation, identifier, image_uri)
            } else {
                (None, None, None, None)
            };

        MediaMetadata {
            creator: tag.and_then(|t| t.artist().map(|s| s.to_string())),
            album: tag.and_then(|t| t.album().map(|s| s.to_string())),
            title: tag.and_then(|t| t.title().map(|s| s.to_string())),
            genre: tag.and_then(|t| t.genre().map(|s| s.to_string())),
            track_num,
            duration_ms,
            image_uri,
            identifier,
            annotation,
        }
    } else {
        MediaMetadata::default()
    }
}

/// Builds a tag-based navigation/selection tree from the given top-level
/// directories.
///
/// Recursively scans all files in `top_dirs` whose extensions match
/// `allowed_extensions`,
/// extracts media metadata, and organizes the files into a hierarchy of
/// genre → musician/creator → album → track. Each node in the resulting tree
/// represents a tag category or a track, and can be used for tag-based
/// navigation/selection in the UI.
pub(crate) fn build_genre_tag_tree(
    top_dirs: &[PathBuf],
    allowed_extensions: &[String],
) -> Vec<TagTreeNode> {
    let mut genre_map: BTreeMap<
        String,
        BTreeMap<String, BTreeMap<String, Vec<(String, PathBuf)>>>,
    > = BTreeMap::new();

    for dir in top_dirs {
        for entry in WalkDir::new(dir).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if allowed_extensions.iter().any(|ae| ae == ext) {
                        let meta = extract_media_metadata(path);
                        let genre =
                            meta.genre.unwrap_or_else(|| "Unknown".to_string());
                        let artist = meta
                            .creator
                            .unwrap_or_else(|| "Unknown".to_string());
                        let album =
                            meta.album.unwrap_or_else(|| "Unknown".to_string());
                        let title = meta.title.clone().unwrap_or_else(|| {
                            path.file_name()
                                .unwrap()
                                .to_string_lossy()
                                .to_string()
                        });
                        genre_map
                            .entry(genre)
                            .or_default()
                            .entry(artist)
                            .or_default()
                            .entry(album)
                            .or_default()
                            .push((title, path.to_path_buf()));
                    }
                }
            }
        }
    }

    // Convert to TagTreeNode hierarchy
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

/// Builds a tag-based navigation/selection  tree using musician/creator as the
/// top-level category.
///
/// Recursively scans all files in `top_dirs` whose extensions match
/// `allowed_extensions`, extracts media metadata, and organizes the files into
/// a hierarchy of musician/creator → album → track. Each node in the resulting
/// tree represents a musician/creator, album, or track, and can be used for
/// tag-based navigation/selection in the UI without including genre as a
/// category.
pub(crate) fn build_creator_tag_tree(
    top_dirs: &[PathBuf],
    allowed_extensions: &[String],
) -> Vec<TagTreeNode> {
    let mut creator_map: BTreeMap<
        String,
        BTreeMap<String, Vec<(String, PathBuf)>>,
    > = BTreeMap::new();

    for dir in top_dirs {
        for entry in WalkDir::new(dir).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if allowed_extensions.iter().any(|ae| ae == ext) {
                        let meta = extract_media_metadata(path);
                        let artist = meta
                            .creator
                            .unwrap_or_else(|| "Unknown".to_string());
                        let album =
                            meta.album.unwrap_or_else(|| "Unknown".to_string());
                        let title = meta.title.clone().unwrap_or_else(|| {
                            path.file_name()
                                .unwrap()
                                .to_string_lossy()
                                .to_string()
                        });
                        creator_map
                            .entry(artist)
                            .or_default()
                            .entry(album)
                            .or_default()
                            .push((title, path.to_path_buf()));
                    }
                }
            }
        }
    }

    let mut roots = Vec::new();
    for (artist, albums) in creator_map {
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
