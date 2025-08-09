use crate::fs::media_metadata::extract_media_metadata;
use crate::gui::RightPanelFile;
use std::fs::File;
use std::io::Write;

/// Exports a playlist of the given files to an XSPF (XML Shareable Playlist
/// Format) file at the specified output path, including metadata such as title,
/// artist, album, duration, genre, and more for each track.
pub(crate) fn export_xspf_playlist(
    files: &[RightPanelFile],
    output_path: &std::path::Path,
) -> std::io::Result<()> {
    let mut xml = String::from(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    xml.push_str(
        r#"<playlist version="1" xmlns="http://xspf.org/ns/0/"><trackList>"#,
    );

    for file in files {
        let meta = extract_media_metadata(&file.path);
        xml.push_str("<track>");
        xml.push_str(&format!(
            "<location>file://{}</location>",
            file.path.display()
        ));
        if let Some(title) = meta.title {
            xml.push_str(&format!("<title>{}</title>", xml_escape(&title)));
        }
        if let Some(creator) = meta.musician {
            xml.push_str(&format!(
                "<creator>{}</creator>",
                xml_escape(&creator)
            ));
        }
        if let Some(album) = meta.album {
            xml.push_str(&format!("<album>{}</album>", xml_escape(&album)));
        }
        if let Some(duration) = meta.duration_ms {
            xml.push_str(&format!("<duration>{duration}</duration>"));
        }
        if let Some(genre) = meta.genre {
            xml.push_str(&format!("<genre>{}</genre>", xml_escape(&genre)));
        }
        if let Some(identifier) = meta.identifier {
            xml.push_str(&format!(
                "<identifier>{}</identifier>",
                xml_escape(&identifier)
            ));
        }
        if let Some(annotation) = meta.annotation {
            xml.push_str(&format!(
                "<annotation>{}</annotation>",
                xml_escape(&annotation)
            ));
        }
        if let Some(track_num) = meta.track_num {
            xml.push_str(&format!("<trackNum>{track_num}</trackNum>"));
        }
        if let Some(image_uri) = meta.image_uri {
            xml.push_str(&format!("<image>{}</image>", xml_escape(&image_uri)));
        }
        xml.push_str("</track>");
    }

    xml.push_str("</trackList></playlist>");

    let mut file = File::create(output_path)?;
    file.write_all(xml.as_bytes())?;
    Ok(())
}

// Simple XML escape for special characters
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use crate::gui::{FileTreeApp, RightPanelFile, SortColumn, SortOrder};
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    #[test]
    fn test_xspf_export_order_matches_right_panel() {
        // Setup dummy files in a specific order
        let file1 = RightPanelFile {
            path: PathBuf::from("/music/a.mp3"),
            musician: Some("Artist1".to_string()),
            album: Some("Album1".to_string()),
            title: Some("Title1".to_string()),
            genre: Some("Genre1".to_string()),
            duration_ms: Some(1),
        };
        let file2 = RightPanelFile {
            path: PathBuf::from("/music/b.mp3"),
            musician: Some("Artist2".to_string()),
            album: Some("Album2".to_string()),
            title: Some("Title2".to_string()),
            genre: Some("Genre2".to_string()),
            duration_ms: Some(1),
        };

        let persist_path = NamedTempFile::new().unwrap().path().to_path_buf();
        let mut app = FileTreeApp::new(vec![], &[], persist_path);
        app.right_panel_files = vec![file2.clone(), file1.clone()]; // Intentionally reversed
        app.right_panel_sort_column = SortColumn::File;
        app.right_panel_sort_order = SortOrder::Asc;

        let sorted = app.sorted_right_panel_files();
        assert_eq!(sorted[0].path, file1.path);
        assert_eq!(sorted[1].path, file2.path);

        let out_file = NamedTempFile::new().unwrap();
        crate::fs::xspf::export_xspf_playlist(&sorted, out_file.path())
            .unwrap();

        let xml = std::fs::read_to_string(out_file.path()).unwrap();
        let locations: Vec<_> = xml
            .split("<location>")
            .skip(1)
            .map(|s| s.split("</location>").next().unwrap().to_string())
            .collect();

        assert_eq!(
            locations[0],
            format!("file://{}", file1.path.to_string_lossy())
        );
        assert_eq!(
            locations[1],
            format!("file://{}", file2.path.to_string_lossy())
        );
    }
}
