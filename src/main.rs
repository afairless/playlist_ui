mod fs;
mod gui;

use gui::{FileTreeApp, update, view};

fn main() -> iced::Result {

    let all_extensions = vec![
        "mp3", "m4a", "wav", 
    ].into_iter().map(|s| s.to_string()).collect();

    iced::application("File Tree Viewer", update, view)
        .run_with(|| (FileTreeApp::load(all_extensions, None), iced::Task::none()))
}
