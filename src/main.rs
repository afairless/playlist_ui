mod file_tree;
mod gui;

use gui::{FileTreeApp, update, view};

fn main() -> iced::Result {
    // TODO:
    //      - select/delete file extensions from gui
    //      - persist file extensions
    //      - make sure entire path is in right-hand list, but not displayed
    //          - make path relative to home directory, if possible
    //      - sort/shuffle right-hand list

    let all_extensions = vec![
        "sh", "rs", "txt", "md", "py", "json", "toml",
    ].into_iter().map(|s| s.to_string()).collect();

    iced::application("File Tree Viewer", update, view)
        .run_with(|| (FileTreeApp::load(all_extensions), iced::Task::none()))
}
