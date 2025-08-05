use std::path::{PathBuf};
use lofty::{
    file::{AudioFile, TaggedFileExt},
    prelude::ItemKey,
    read_from_path,
    tag::Accessor,
};

pub struct MediaMetadata {
    pub musician: Option<String>,
    pub album: Option<String>,
    pub title: Option<String>,
    pub genre: Option<String>,
    pub track_num: Option<u32>,
    pub duration_ms: Option<u64>,
    pub image_uri: Option<String>,
    pub identifier: Option<String>,
    pub annotation: Option<String>,
}

pub fn extract_media_metadata(path: &PathBuf) -> MediaMetadata {
    match read_from_path(path) {
        Ok(tagged_file) => {
            let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());
            let duration_ms = Some(tagged_file.properties().duration().as_millis() as u64);

            let (track_num, annotation, identifier, image_uri) = if let Some(tag) = tag {
                // Track number
                let track_num = tag.track();

                // Annotation (comment)
                let annotation = tag.comment().map(|s| s.to_string());

                // Identifier (try MusicBrainz or ISRC)
                let identifier = tag.get_string(&ItemKey::MusicBrainzTrackId)
                    .or_else(|| tag.get_string(&ItemKey::Isrc))
                    .map(|s| s.to_string());

                // Album art (save first picture if present)
                let image_uri = tag.pictures().first().and_then(|pic| {
                    let img_path = path.with_extension("cover.jpg");
                    if std::fs::write(&img_path, pic.data()).is_ok() {
                        Some(format!("file://{}", img_path.to_string_lossy()))
                    } else {
                        None
                    }
                });

                (track_num, annotation, identifier, image_uri)
            } else {
                (None, None, None, None)
            };

            MediaMetadata {
                musician: tag.and_then(|t| t.artist().map(|s| s.to_string())),
                album: tag.and_then(|t| t.album().map(|s| s.to_string())),
                title: tag.and_then(|t| t.title().map(|s| s.to_string())),
                genre: tag.and_then(|t| t.genre().map(|s| s.to_string())),
                track_num,
                duration_ms,
                image_uri,
                identifier,
                annotation,
            }
        }
        Err(_) => MediaMetadata {
            musician: None,
            album: None,
            title: None,
            genre: None,
            track_num: None,
            duration_ms: None,
            image_uri: None,
            identifier: None,
            annotation: None,
        },
    }
}
