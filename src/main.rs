mod file_tree;
mod gui;

use gui::{FileTreeApp, update, view};

fn main() -> iced::Result {

    let all_extensions = vec![
        "sh", "rs", "txt", "md", "py", "json", "toml",
    ].into_iter().map(|s| s.to_string()).collect();

    iced::application("File Tree Viewer", update, view)
        .run_with(|| (FileTreeApp::load(all_extensions, None), iced::Task::none()))
}
