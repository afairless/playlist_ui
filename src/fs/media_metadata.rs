use std::path::PathBuf;
use lofty::{read_from_path, file::TaggedFileExt, tag::Accessor};

pub struct MediaMetadata {
    pub musician: Option<String>,
    pub album: Option<String>,
    pub title: Option<String>,
    pub genre: Option<String>,
}

pub fn extract_media_metadata(path: &PathBuf) -> MediaMetadata {
    match read_from_path(path) {
        Ok(tagged_file) => {
            let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());
            if let Some(tag) = tag {
                MediaMetadata {
                    musician: tag.artist().map(|s| s.to_string()),
                    album: tag.album().map(|s| s.to_string()),
                    title: tag.title().map(|s| s.to_string()),
                    genre: tag.genre().map(|s| s.to_string()),
                }
            } else {
                MediaMetadata {
                    musician: None,
                    album: None,
                    title: None,
                    genre: None,
                }
            }
        }
        Err(_) => MediaMetadata {
            musician: None,
            album: None,
            title: None,
            genre: None,
        },
    }
}
