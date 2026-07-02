//! Database module for the Playlist UI.
//!
//! Provides persistent storage for tag trees using the Sled embedded
//! database. Tag trees (genre and creator hierarchies) are cached here
//! to avoid rebuilding on every launch.
//!
//! Sub-modules:
//!     sled_store — Sled-backed key-value store for tag tree persistence

pub mod sled_store;
