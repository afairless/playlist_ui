mod file_tree;
mod gui;

use file_tree::scan_directory;
use gui::{FileTreeApp, update, view};
use std::path::Path;

fn main() -> iced::Result {
    let home_dir = std::env::var("HOME").expect("HOME environment variable not set");
    let dir = Path::new(&home_dir).join("Documents").join("ma_timing");
    let allowed = ["txt", "rs", "md"];
    
    let root_node = scan_directory(&dir, &allowed);
    
    iced::application("File Tree Viewer", update, view)
        .run_with(|| (FileTreeApp::new(root_node), iced::Task::none()))
}
