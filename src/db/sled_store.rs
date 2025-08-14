use crate::gui::TagTreeNode;
use bincode;
use bincode::{config::standard, decode_from_slice, encode_to_vec};
use sled::{Db, IVec};

#[derive(Debug, Clone)]
pub struct SledStore {
    db: Db,
}

impl SledStore {
    pub fn new(path: &str) -> Result<Self, sled::Error> {
        let db = sled::open(path)?;
        Ok(Self { db })
    }

    pub fn save_genre_tag_tree(
        &self,
        roots: &[TagTreeNode],
    ) -> Result<(), sled::Error> {
        let config = standard();
        let data = encode_to_vec(roots, config).unwrap();
        self.db.insert("tag_tree", data)?;
        Ok(())
    }

    pub fn load_genre_tag_tree(&self) -> Option<Vec<TagTreeNode>> {
        let config = standard();
        self.db.get("tag_tree").ok().flatten().and_then(|ivec: IVec| {
            decode_from_slice(&ivec, config).ok().map(|(val, _len)| val)
        })
    }

    pub fn clear_genre_tree(&self) -> Result<(), sled::Error> {
        self.db.remove("tag_tree")?;
        Ok(())
    }

    pub fn save_creator_tag_tree(
        &self,
        roots: &[TagTreeNode],
    ) -> Result<(), sled::Error> {
        let config = standard();
        let data = encode_to_vec(roots, config).unwrap();
        self.db.insert("creator_tag_tree", data)?;
        Ok(())
    }

    pub fn load_creator_tag_tree(&self) -> Option<Vec<TagTreeNode>> {
        let config = standard();
        self.db.get("creator_tag_tree").ok().flatten().and_then(|ivec: IVec| {
            decode_from_slice(&ivec, config).ok().map(|(val, _len)| val)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::media_metadata::build_genre_tag_tree;
    use tempfile::TempDir;

    #[test]
    fn test_save_and_load_tag_tree_with_sled() {
        // Setup a temporary directory for sled
        let temp_dir = TempDir::new().unwrap();
        let sled_path = temp_dir.path().join("sled_test_db");
        let sled_store = SledStore::new(sled_path.to_str().unwrap()).unwrap();

        // Use dummy top_dirs and extensions
        let top_dirs = vec![std::path::PathBuf::from("/tmp")];
        let extensions = vec!["mp3".to_string(), "flac".to_string()];

        // Build and save the tag tree
        let tag_tree = build_genre_tag_tree(&top_dirs, &extensions);
        sled_store.save_genre_tag_tree(&tag_tree).unwrap();

        // Load the tag tree back
        let loaded_tree = sled_store.load_genre_tag_tree().unwrap();

        // Basic check: the loaded tree should equal the saved tree
        assert_eq!(tag_tree, loaded_tree);

        // Optionally, check a specific node if you know the structure
        if let Some(first_genre) = loaded_tree.first() {
            println!("First genre label: {}", first_genre.label);
        }
    }
}
