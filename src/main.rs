mod fs;
mod gui;

use gui::{FileTreeApp, update, view};

fn main() -> iced::Result {
    let non_audio_export_extensions = vec![
        "txt", "md", "sh", "json", "toml",
    ].into_iter().map(|s| s.to_string()).collect::<Vec<_>>();

    let audio_export_extensions = vec![
        "aac", "m4a", "mp4", "ape", "aiff", "aif", "flac", "mp3", "mp4", 
        "m4a", "m4b", "m4p", "mpc", "opus", "ogg", "oga", "spx", "wav", 
        "wv"
    ].into_iter().map(|s| s.to_string()).collect::<Vec<_>>();

    let all_extensions: Vec<String> = non_audio_export_extensions.iter()
        .chain(audio_export_extensions.iter())
        .cloned()
        .collect();

    iced::application("File Tree Viewer", update, view)
        .run_with(|| (FileTreeApp::load(all_extensions, Some(audio_export_extensions), None), iced::Task::none()))
}
