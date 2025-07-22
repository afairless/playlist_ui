mod file_tree;
mod gui;

use gui::{FileTreeApp, update, view};
use std::path::Path;

fn main() -> iced::Result {
    // TODO:
    //      - select/delete top-level directories from gui
    //      - persist top-level directories 
    //      - select/delete file extensions from gui
    //      - persist file extensions directories 
    //      - divide gui vertically 
    //      - add right-click to add directories/files selected on left to right
    //      - sort/shuffle right-hand list

    let home_dir = std::env::var("HOME").expect("HOME environment variable not set");
    let dir = Path::new(&home_dir).join("Documents").join("ma_timing");
    let all_extensions = vec![
        "rs", "txt", "md", "py", "js", "json", "toml", "cpp", "h", "c", "java", "go", "ts"
    ].into_iter().map(|s| s.to_string()).collect();

    iced::application("File Tree Viewer", update, view)
        .run_with(|| (FileTreeApp::new(dir, all_extensions), iced::Task::none()))
}
