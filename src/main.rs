mod db;
mod fs;
mod gui;
mod utils;

use crate::db::sled_store::SledStore;
use crate::fs::media_metadata::{build_creator_tag_tree, build_genre_tag_tree};
use gui::{FileTreeApp, update, view};
use std::path::PathBuf;

// Currently, Sled database is not incrementally updated when tags from media
// files metadata are changed; instead, the database must be deleted and
// completely rebuilt to include any such changes
fn get_sled_db_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".playlist_ui_db")
}

fn main() -> iced::Result {
    env_logger::init();

    const AUDIO_EXPORT_EXTENSIONS: &[&str] = &[
        "aac", "m4a", "mp4", "ape", "aiff", "aif", "flac", "mp3", "mp4", "m4a",
        "m4b", "m4p", "mpc", "opus", "ogg", "oga", "spx", "wav", "wv",
    ];

    let sled_store = SledStore::new(get_sled_db_path().to_str().unwrap())
        .expect("Failed to open sled db");

    iced::application("File Tree Viewer", update, view).run_with(move || {
        let mut app = FileTreeApp::load(
            AUDIO_EXPORT_EXTENSIONS,
            None,
            Some(sled_store.clone()),
        );

        // Ensure genre tag tree is present in sled
        if sled_store.load_genre_tag_tree().is_none() {
            let tree =
                build_genre_tag_tree(&app.top_dirs, &app.selected_extensions);
            sled_store.save_genre_tag_tree(&tree).ok();
        }

        // Ensure creator tree is present in sled
        if sled_store.load_creator_tag_tree().is_none() {
            let tree =
                build_creator_tag_tree(&app.top_dirs, &app.selected_extensions);
            sled_store.save_creator_tag_tree(&tree).ok();
        }

        // load the genre tree into app.tag_tree_roots if you want to start in
        // genre tag tree mode
        if let Some(tree) = sled_store.load_genre_tag_tree() {
            app.tag_tree_roots = tree;
        }

        (app, iced::Task::none())
    })
}
