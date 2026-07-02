//! Filesystem module for the Playlist UI.
//!
//! Handles directory scanning, media metadata extraction, tag-tree
//! construction (genre and creator hierarchies), and XSPF playlist
//! export.
//!
//! Sub-modules:
//!     file_tree           — recursive directory scanning
//!     media_metadata      — audio file metadata and tag trees
//!     media_metadata_async — async variants (experimental, not wired)
//!     xspf                — XSPF playlist export

pub mod file_tree;
pub mod media_metadata;
pub mod xspf;
