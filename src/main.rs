mod fs;
mod gui;

use gui::{FileTreeApp, update, view};

fn main() -> iced::Result {

    let all_extensions = vec![
        // "mp3", "m4a", "wav", 
        "aac", "m4a", "mp4", "ape", "aiff", "aif", "flac", "mp3", "mp4", 
        "m4a", "m4b", "m4p", "mpc", "opus", "ogg", "oga", "spx", "wav", 
        "wv"
    ].into_iter().map(|s| s.to_string()).collect();

    iced::application("File Tree Viewer", update, view)
        .run_with(|| (FileTreeApp::load(all_extensions, None), iced::Task::none()))
}

