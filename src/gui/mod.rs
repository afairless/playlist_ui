//! GUI module for the Playlist UI application.
//!
//! Implements the iced Elm-architecture UI with two panels: a left panel for
//! browsing files by directory or tags (genre/creator), and a right panel for
//! managing the playlist. Exports the core application types and the update/
//! view functions wired into `main.rs`.
//!
//! Public API:
//!     FileTreeApp        — root application model
//!     Message            — all user-action messages
//!     TagTreeNode        — genre/creator/album/track hierarchy node
//!     RightPanelFile     — a file entry in the playlist
//!     LeftPanelSelectMode — directory / genre / creator selection mode
//!     LeftPanelSortMode  — alphanumeric, modified-date, or file-count sort
//!     SortColumn         — column key for right-panel sorting
//!     SortOrder          — ascending or descending
//!     TextSearchMode     — search mode for text filtering
//!     update             — message handler (pure state transition)
//!     view               — layout composer

mod left_panel;
mod render_node;
mod right_panel;
mod state;
mod tantivy_search;
mod update;
mod view;

pub use state::{
    FileTreeApp, LeftPanelSelectMode, LeftPanelSortMode, Message,
    RightPanelFile, SortColumn, SortOrder, TagTreeNode, TextSearchMode,
};
pub use update::update;
pub use view::view;
