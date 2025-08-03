use std::path::PathBuf;
use id3::TagLike;

pub struct MediaMetadata {
    pub musician: Option<String>,
    pub album: Option<String>,
    pub title: Option<String>,
    pub genre: Option<String>,
}

pub fn extract_mp3_metadata(path: &PathBuf) -> MediaMetadata {
    if path.extension().map(|e| e == "mp3").unwrap_or(false) {
        if let Ok(tag) = id3::Tag::read_from_path(path) {
            return MediaMetadata {
                musician: tag.artist().map(|s| s.to_string()),
                album: tag.album().map(|s| s.to_string()),
                title: tag.title().map(|s| s.to_string()),
                genre: tag.genre().map(|s| s.to_string()),
            };
        }
    }
    MediaMetadata {
        musician: None,
        album: None,
        title: None,
        genre: None,
    }
}
