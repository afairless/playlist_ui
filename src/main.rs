mod fs;
mod gui;
mod utils;

use gui::{FileTreeApp, update, view};

fn main() -> iced::Result {
    env_logger::init();

    const AUDIO_EXPORT_EXTENSIONS: &[&str] = &[
        "aac", "m4a", "mp4", "ape", "aiff", "aif", "flac", "mp3", "mp4", "m4a",
        "m4b", "m4p", "mpc", "opus", "ogg", "oga", "spx", "wav", "wv",
    ];

    iced::application("File Tree Viewer", update, view).run_with(move || {
        (FileTreeApp::load(AUDIO_EXPORT_EXTENSIONS, None), iced::Task::none())
    })
}
