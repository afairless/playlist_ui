//! Tantivy-based full-text search for audio file metadata.
#![allow(dead_code)]
//!
//! Provides an in-memory tantivy index that maps audio file metadata
//! (title, creator, album, genre, filename, path) to file paths for
//! fast set-based tree pruning.
//!
//! Public API:
//!     TantivyIndexWrapper  - in-memory index + reader + schema
//!     build_tantivy_index  - construct an index from FileNode trees
//!     prune_file_tree      - prune a FileNode tree against a match set
//!     prune_tag_node       - prune a TagTreeNode tree against a match set

use crate::fs::file_tree::FileNode;
use crate::fs::media_metadata::extract_media_metadata;
use crate::gui::TagTreeNode;
use crate::gui::state::TextSearchMode;
use std::collections::HashSet;
use std::path::PathBuf;
use tantivy::Index;
use tantivy::collector::DocSetCollector;
use tantivy::doc;
use tantivy::query::{
    BooleanQuery, FuzzyTermQuery, Occur, PhrasePrefixQuery, Query, RegexQuery,
};
use tantivy::schema::*;
use tantivy::tokenizer::*;

#[derive(Clone)]
#[allow(dead_code)]
pub(crate) struct TantivyIndexWrapper {
    index: Index,
    reader: tantivy::IndexReader,
    schema: Schema,
}

impl std::fmt::Debug for TantivyIndexWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TantivyIndexWrapper")
            .field("num_docs", &self.reader.searcher().num_docs())
            .field("schema", &self.schema)
            .finish_non_exhaustive()
    }
}

impl TantivyIndexWrapper {
    pub(crate) fn search(
        &self,
        query: &str,
        mode: TextSearchMode,
    ) -> Result<HashSet<PathBuf>, tantivy::TantivyError> {
        let searcher = self.reader.searcher();
        let query_obj = self.build_query(query, mode)?;
        let matching_docs = searcher.search(&query_obj, &DocSetCollector)?;
        let path_field =
            self.schema.get_field("path").expect("path field exists in schema");
        let mut results = HashSet::new();
        for doc_addr in &matching_docs {
            let doc = searcher.doc::<TantivyDocument>(*doc_addr)?;
            if let Some(value) = doc.get_first(path_field)
                && let Some(path_str) = value.as_str()
            {
                results.insert(PathBuf::from(path_str));
            }
        }
        if results.is_empty() && query.len() <= 3 {
            return self.search_fuzzy(query, mode);
        }
        Ok(results)
    }

    fn build_query(
        &self,
        query: &str,
        mode: TextSearchMode,
    ) -> Result<Box<dyn Query>, tantivy::TantivyError> {
        let fields = self.search_field_names(mode);
        let trimmed = query.trim();
        if fields.is_empty() {
            return Ok(Box::new(BooleanQuery::new(Vec::new())));
        }
        let mut subqueries: Vec<(Occur, Box<dyn Query>)> = Vec::new();
        for field_name in &fields {
            let field = self
                .schema
                .get_field(field_name)
                .expect("field exists in schema");
            if *field_name == "path" {
                let escaped = regex_escape(trimmed);
                let pattern = format!("(?i).*{}.*", escaped);
                if let Ok(rq) = RegexQuery::from_pattern(&pattern, field) {
                    subqueries.push((Occur::Should, Box::new(rq)));
                }
            } else {
                let terms: Vec<&str> = trimmed.split_whitespace().collect();
                if !terms.is_empty() {
                    let term_objs: Vec<Term> = terms
                        .iter()
                        .map(|t| Term::from_field_text(field, t))
                        .collect();
                    let ppq = PhrasePrefixQuery::new(term_objs);
                    subqueries.push((Occur::Should, Box::new(ppq)));
                }
            }
        }
        if subqueries.len() == 1 {
            Ok(subqueries.into_iter().next().unwrap().1)
        } else {
            Ok(Box::new(BooleanQuery::new(subqueries)))
        }
    }

    fn search_fuzzy(
        &self,
        query: &str,
        mode: TextSearchMode,
    ) -> Result<HashSet<PathBuf>, tantivy::TantivyError> {
        let searcher = self.reader.searcher();
        let trimmed = query.trim();
        let fields = self.search_field_names(mode);
        let mut subqueries: Vec<(Occur, Box<dyn Query>)> = Vec::new();
        for field_name in &fields {
            let field = self
                .schema
                .get_field(field_name)
                .expect("field exists in schema");
            if *field_name == "path" {
                let escaped = regex_escape(trimmed);
                let pattern = format!("(?i).*{}.*", escaped);
                if let Ok(rq) = RegexQuery::from_pattern(&pattern, field) {
                    subqueries.push((Occur::Should, Box::new(rq)));
                }
            } else {
                let term = tantivy::Term::from_field_text(
                    field,
                    &trimmed.to_lowercase(),
                );
                let ftq = FuzzyTermQuery::new(term, 1, true);
                subqueries.push((Occur::Should, Box::new(ftq)));
            }
        }
        let query_obj: Box<dyn Query> = if subqueries.len() == 1 {
            subqueries.into_iter().next().unwrap().1
        } else {
            Box::new(BooleanQuery::new(subqueries))
        };
        let matching_docs = searcher.search(&query_obj, &DocSetCollector)?;
        let path_field =
            self.schema.get_field("path").expect("path field exists in schema");
        let mut results = HashSet::new();
        for doc_addr in &matching_docs {
            let doc = searcher.doc::<TantivyDocument>(*doc_addr)?;
            if let Some(value) = doc.get_first(path_field)
                && let Some(path_str) = value.as_str()
            {
                results.insert(PathBuf::from(path_str));
            }
        }
        Ok(results)
    }

    fn search_field_names(&self, mode: TextSearchMode) -> Vec<&str> {
        match mode {
            TextSearchMode::All => {
                vec!["creator", "album", "title", "genre", "filename", "path"]
            },
            TextSearchMode::DirectoryPath => vec!["path"],
            TextSearchMode::TrackFilename => vec!["filename"],
            TextSearchMode::Creator => vec!["creator"],
            TextSearchMode::Album => vec!["album"],
            TextSearchMode::Title => vec!["title"],
            TextSearchMode::Genre => vec!["genre"],
        }
    }
}

fn regex_escape(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']'
            | '{' | '}' | '^' | '$' | '#' => {
                result.push('\\');
                result.push(c);
            },
            _ => result.push(c),
        }
    }
    result
}

pub(crate) fn build_tantivy_index(
    root_nodes: &[Option<FileNode>],
) -> TantivyIndexWrapper {
    let mut schema_builder = Schema::builder();
    let path_field = schema_builder.add_text_field("path", STRING | STORED);
    let filename_field = schema_builder.add_text_field("filename", TEXT);
    let creator_field = schema_builder.add_text_field("creator", TEXT);
    let album_field = schema_builder.add_text_field("album", TEXT);
    let title_field = schema_builder.add_text_field("title", TEXT);
    let genre_field = schema_builder.add_text_field("genre", TEXT);
    let schema = schema_builder.build();
    let index = Index::create_in_ram(schema.clone());
    index.tokenizers().register(
        "default",
        TextAnalyzer::builder(SimpleTokenizer::default())
            .filter(LowerCaser)
            .build(),
    );
    let mut writer =
        index.writer(50_000_000).expect("failed to create tantivy writer");
    for node in root_nodes.iter().flatten() {
        index_file_node(
            node,
            &mut writer,
            &path_field,
            &filename_field,
            &creator_field,
            &album_field,
            &title_field,
            &genre_field,
        );
    }
    writer.commit().expect("failed to commit tantivy index");
    let reader = index.reader().expect("failed to create tantivy reader");
    TantivyIndexWrapper { index, reader, schema }
}

#[allow(clippy::too_many_arguments)]
fn index_file_node(
    node: &FileNode,
    writer: &mut tantivy::IndexWriter,
    path_field: &Field,
    filename_field: &Field,
    creator_field: &Field,
    album_field: &Field,
    title_field: &Field,
    genre_field: &Field,
) {
    use crate::fs::file_tree::NodeType;
    match node.node_type {
        NodeType::File => {
            let metadata = extract_media_metadata(&node.path);
            let filename = node
                .path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let _ = writer.add_document(tantivy::doc!(
                *path_field => node.path.to_string_lossy().to_string(),
                *filename_field => filename,
                *creator_field => metadata.creator.unwrap_or_default(),
                *album_field => metadata.album.unwrap_or_default(),
                *title_field => metadata.title.unwrap_or_default(),
                *genre_field => metadata.genre.unwrap_or_default(),
            ));
        },
        NodeType::Directory => {
            for child in &node.children {
                index_file_node(
                    child,
                    writer,
                    path_field,
                    filename_field,
                    creator_field,
                    album_field,
                    title_field,
                    genre_field,
                );
            }
        },
    }
}

pub(crate) fn prune_file_tree(
    node: &FileNode,
    matches: &HashSet<PathBuf>,
    query: &str,
    mode: TextSearchMode,
) -> Option<FileNode> {
    use crate::fs::file_tree::NodeType;
    match node.node_type {
        NodeType::File => {
            if matches.contains(&node.path) {
                Some(node.clone())
            } else {
                None
            }
        },
        NodeType::Directory => {
            let dir_matches = match mode {
                TextSearchMode::DirectoryPath | TextSearchMode::All => {
                    let q = query.to_lowercase();
                    node.path.to_string_lossy().to_lowercase().contains(&q)
                        || node.name.to_lowercase().contains(&q)
                },
                _ => false,
            };
            if dir_matches {
                let mut pruned_children: Vec<FileNode> = node
                    .children
                    .iter()
                    .filter_map(|c| prune_file_tree(c, matches, query, mode))
                    .collect();
                let file_count =
                    pruned_children.iter().map(|c| c.file_count).sum();
                pruned_children.sort_by(|a, b| a.name.cmp(&b.name));
                Some(FileNode {
                    name: node.name.clone(),
                    path: node.path.clone(),
                    node_type: NodeType::Directory,
                    children: pruned_children,
                    is_expanded: node.is_expanded,
                    file_count,
                })
            } else {
                let mut pruned_children: Vec<FileNode> = node
                    .children
                    .iter()
                    .filter_map(|c| prune_file_tree(c, matches, query, mode))
                    .collect();
                if pruned_children.is_empty() {
                    None
                } else {
                    let file_count =
                        pruned_children.iter().map(|c| c.file_count).sum();
                    pruned_children.sort_by(|a, b| a.name.cmp(&b.name));
                    Some(FileNode {
                        name: node.name.clone(),
                        path: node.path.clone(),
                        node_type: NodeType::Directory,
                        children: pruned_children,
                        is_expanded: node.is_expanded,
                        file_count,
                    })
                }
            }
        },
    }
}

pub(crate) fn prune_tag_node(
    node: &TagTreeNode,
    matches: &HashSet<PathBuf>,
) -> Option<TagTreeNode> {
    if node.children.is_empty() {
        if node.file_paths.iter().any(|p| matches.contains(p)) {
            Some(node.clone())
        } else {
            None
        }
    } else {
        let pruned: Vec<TagTreeNode> = node
            .children
            .iter()
            .filter_map(|c| prune_tag_node(c, matches))
            .collect();
        if pruned.is_empty() {
            None
        } else {
            let mut cloned = node.clone();
            cloned.children = pruned;
            cloned.file_count =
                cloned.children.iter().map(|c| c.file_count).sum();
            Some(cloned)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::file_tree::NodeType;
    use std::path::Path;

    fn make_single_doc_wrapper(
        path_str: &str,
        creator: &str,
        album: &str,
        title: &str,
        genre: &str,
    ) -> TantivyIndexWrapper {
        let mut sb = Schema::builder();
        let pf = sb.add_text_field("path", STRING | STORED);
        let ff = sb.add_text_field("filename", TEXT);
        let cf = sb.add_text_field("creator", TEXT);
        let af = sb.add_text_field("album", TEXT);
        let tf = sb.add_text_field("title", TEXT);
        let gf = sb.add_text_field("genre", TEXT);
        let schema = sb.build();
        let index = Index::create_in_ram(schema.clone());
        index.tokenizers().register(
            "default",
            TextAnalyzer::builder(SimpleTokenizer::default())
                .filter(LowerCaser)
                .build(),
        );
        let mut w = index.writer(50_000_000).unwrap();
        let fn_ = Path::new(path_str)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let _ = w.add_document(tantivy::doc!(
            pf => path_str.to_string(),
            ff => fn_,
            cf => creator.to_string(),
            af => album.to_string(),
            tf => title.to_string(),
            gf => genre.to_string(),
        ));
        w.commit().unwrap();
        let reader = index.reader().unwrap();
        TantivyIndexWrapper { index, reader, schema }
    }

    fn build_index_from_pairs(
        docs: &[(&str, &str, &str, &str, &str)],
    ) -> TantivyIndexWrapper {
        let mut sb = Schema::builder();
        let pf = sb.add_text_field("path", STRING | STORED);
        let ff = sb.add_text_field("filename", TEXT);
        let cf = sb.add_text_field("creator", TEXT);
        let af = sb.add_text_field("album", TEXT);
        let tf = sb.add_text_field("title", TEXT);
        let gf = sb.add_text_field("genre", TEXT);
        let schema = sb.build();
        let index = Index::create_in_ram(schema.clone());
        index.tokenizers().register(
            "default",
            TextAnalyzer::builder(SimpleTokenizer::default())
                .filter(LowerCaser)
                .build(),
        );
        let mut w = index.writer(50_000_000).unwrap();
        for (p, c, a, t, g) in docs {
            let fn_ = Path::new(p)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let _ = w.add_document(tantivy::doc!(
                pf => p.to_string(), ff => fn_, cf => c.to_string(),
                af => a.to_string(), tf => t.to_string(), gf => g.to_string(),
            ));
        }
        w.commit().unwrap();
        let reader = index.reader().unwrap();
        TantivyIndexWrapper { index, reader, schema }
    }

    #[test]
    fn test_search_empty_index() {
        let w = make_single_doc_wrapper("/a.mp3", "", "", "", "");
        assert!(w.search("xyz", TextSearchMode::All).unwrap().is_empty());
    }

    #[test]
    fn test_search_empty_index_empty_query() {
        let w = build_tantivy_index(&[]);
        assert!(w.search("anything", TextSearchMode::All).unwrap().is_empty());
    }

    #[test]
    fn test_search_single_doc_by_title() {
        let w =
            make_single_doc_wrapper("/music/test.mp3", "", "", "Test Song", "");
        let r = w.search("test", TextSearchMode::Title).unwrap();
        assert_eq!(r.len(), 1);
        assert!(r.contains(&PathBuf::from("/music/test.mp3")));
    }

    #[test]
    fn test_search_single_doc_by_creator() {
        let w = make_single_doc_wrapper(
            "/music/artist.mp3",
            "Some Artist",
            "",
            "",
            "",
        );
        let r = w.search("artist", TextSearchMode::Creator).unwrap();
        assert_eq!(r.len(), 1);
        assert!(r.contains(&PathBuf::from("/music/artist.mp3")));
    }

    #[test]
    fn test_search_all_mode_multi_field() {
        let w = build_index_from_pairs(&[
            ("/a.mp3", "Artist One", "Album A", "Title One", "Rock"),
            ("/b.mp3", "Artist Two", "Album B", "Title Two", "Jazz"),
        ]);
        let r = w.search("rock", TextSearchMode::All).unwrap();
        assert_eq!(r.len(), 1);
        assert!(r.contains(&PathBuf::from("/a.mp3")));
        let r = w.search("two", TextSearchMode::All).unwrap();
        assert_eq!(r.len(), 1);
        assert!(r.contains(&PathBuf::from("/b.mp3")));
    }

    #[test]
    fn test_search_path_regex() {
        let w =
            make_single_doc_wrapper("/music/jazz/track.mp3", "", "", "", "");
        let r = w.search("jazz", TextSearchMode::DirectoryPath).unwrap();
        assert_eq!(r.len(), 1);
        assert!(r.contains(&PathBuf::from("/music/jazz/track.mp3")));
    }

    #[test]
    fn test_search_filename_prefix() {
        let w = make_single_doc_wrapper(
            "/music/progressive_rock.mp3",
            "",
            "",
            "",
            "",
        );
        let r = w.search("prog", TextSearchMode::TrackFilename).unwrap();
        assert_eq!(r.len(), 1);
        assert!(r.contains(&PathBuf::from("/music/progressive_rock.mp3")));
    }

    #[test]
    fn test_search_no_match() {
        let w = make_single_doc_wrapper(
            "/music/song.mp3",
            "Artist",
            "Album",
            "Title",
            "Rock",
        );
        let r = w.search("nonexistent", TextSearchMode::All).unwrap();
        assert!(r.is_empty());
    }

    #[test]
    fn test_search_debug_format() {
        let w = make_single_doc_wrapper("/music/test.mp3", "", "", "", "");
        let d = format!("{:?}", w);
        assert!(d.contains("TantivyIndexWrapper"));
        assert!(d.contains("num_docs"));
    }

    #[test]
    fn test_build_empty() {
        let w = build_tantivy_index(&[]);
        assert!(w.search("anything", TextSearchMode::All).unwrap().is_empty());
    }

    #[test]
    fn test_build_single_file() {
        let fn_ = FileNode {
            name: "test.mp3".to_string(),
            path: PathBuf::from("/tmp/test.mp3"),
            node_type: NodeType::File,
            children: vec![],
            is_expanded: false,
            file_count: 1,
        };
        let w = build_tantivy_index(&[Some(fn_)]);
        let d = format!("{:?}", w);
        assert!(d.contains("num_docs"));
    }

    #[test]
    fn test_prune_file_tree_empty_tree() {
        let r = prune_file_tree(
            &FileNode {
                name: "r".into(),
                path: PathBuf::from("/r"),
                node_type: NodeType::Directory,
                children: vec![],
                is_expanded: false,
                file_count: 0,
            },
            &HashSet::new(),
            "NONEXISTENT",
            TextSearchMode::All,
        );
        assert!(r.is_none());
    }

    #[test]
    fn test_prune_file_tree_no_matches() {
        let c =
            FileNode::new_file("a.mp3".into(), PathBuf::from("/root/a.mp3"));
        let t = FileNode {
            name: "root".into(),
            path: PathBuf::from("/root"),
            node_type: NodeType::Directory,
            children: vec![c],
            is_expanded: false,
            file_count: 1,
        };
        let r = prune_file_tree(
            &t,
            &HashSet::new(),
            "NONEXISTENT",
            TextSearchMode::All,
        );
        assert!(r.is_none());
    }

    #[test]
    fn test_prune_file_tree_all_match() {
        let c =
            FileNode::new_file("a.mp3".into(), PathBuf::from("/root/a.mp3"));
        let t = FileNode {
            name: "root".into(),
            path: PathBuf::from("/root"),
            node_type: NodeType::Directory,
            children: vec![c],
            is_expanded: false,
            file_count: 1,
        };
        let mut m = HashSet::new();
        m.insert(PathBuf::from("/root/a.mp3"));
        let r = prune_file_tree(&t, &m, "", TextSearchMode::All);
        assert!(r.is_some());
        assert_eq!(r.unwrap().children.len(), 1);
    }

    #[test]
    fn test_prune_file_tree_partial() {
        let c1 =
            FileNode::new_file("a.mp3".into(), PathBuf::from("/root/a.mp3"));
        let c2 =
            FileNode::new_file("b.mp3".into(), PathBuf::from("/root/b.mp3"));
        let t = FileNode {
            name: "root".into(),
            path: PathBuf::from("/root"),
            node_type: NodeType::Directory,
            children: vec![c1, c2],
            is_expanded: false,
            file_count: 2,
        };
        let mut m = HashSet::new();
        m.insert(PathBuf::from("/root/a.mp3"));
        let r = prune_file_tree(&t, &m, "", TextSearchMode::All);
        assert!(r.is_some());
        let p = r.unwrap();
        assert_eq!(p.children.len(), 1);
        assert_eq!(p.file_count, 1);
        assert_eq!(p.children[0].name, "a.mp3");
    }

    #[test]
    fn test_prune_file_tree_file_count_updated() {
        let c1 =
            FileNode::new_file("a.mp3".into(), PathBuf::from("/root/a.mp3"));
        let c2 =
            FileNode::new_file("b.mp3".into(), PathBuf::from("/root/b.mp3"));
        let t = FileNode {
            name: "root".into(),
            path: PathBuf::from("/root"),
            node_type: NodeType::Directory,
            children: vec![c1, c2],
            is_expanded: false,
            file_count: 2,
        };
        let mut m = HashSet::new();
        m.insert(PathBuf::from("/root/a.mp3"));
        let r = prune_file_tree(&t, &m, "a", TextSearchMode::All);
        assert!(r.is_some());
        assert_eq!(r.unwrap().file_count, 1);
    }

    #[test]
    fn test_prune_file_tree_directory_name_match() {
        let child = FileNode::new_file(
            "song.mp3".into(),
            PathBuf::from("/music/rock/song.mp3"),
        );
        let subdir = FileNode {
            name: "rock".into(),
            path: PathBuf::from("/music/rock"),
            node_type: NodeType::Directory,
            children: vec![child],
            is_expanded: false,
            file_count: 1,
        };
        let tree = FileNode {
            name: "music".into(),
            path: PathBuf::from("/music"),
            node_type: NodeType::Directory,
            children: vec![subdir],
            is_expanded: false,
            file_count: 1,
        };
        let r = prune_file_tree(
            &tree,
            &HashSet::new(),
            "rock",
            TextSearchMode::All,
        );
        assert!(r.is_some());
    }

    #[test]
    fn test_prune_tag_node_empty() {
        let n = TagTreeNode {
            label: "g".into(),
            children: vec![],
            file_paths: vec![PathBuf::from("/a.mp3")],
            is_expanded: false,
            file_count: 1,
        };
        assert!(prune_tag_node(&n, &HashSet::new()).is_none());
    }

    #[test]
    fn test_prune_tag_node_all_match() {
        let track = TagTreeNode {
            label: "T1".into(),
            children: vec![],
            file_paths: vec![PathBuf::from("/a.mp3")],
            is_expanded: false,
            file_count: 1,
        };
        let album = TagTreeNode {
            label: "A1".into(),
            children: vec![track],
            file_paths: vec![],
            is_expanded: false,
            file_count: 1,
        };
        let genre = TagTreeNode {
            label: "R".into(),
            children: vec![album],
            file_paths: vec![],
            is_expanded: false,
            file_count: 1,
        };
        let mut m = HashSet::new();
        m.insert(PathBuf::from("/a.mp3"));
        let r = prune_tag_node(&genre, &m);
        assert!(r.is_some());
        assert_eq!(r.unwrap().file_count, 1);
    }

    #[test]
    fn test_prune_tag_node_partial() {
        let t1 = TagTreeNode {
            label: "T1".into(),
            children: vec![],
            file_paths: vec![PathBuf::from("/a.mp3")],
            is_expanded: false,
            file_count: 1,
        };
        let t2 = TagTreeNode {
            label: "T2".into(),
            children: vec![],
            file_paths: vec![PathBuf::from("/b.mp3")],
            is_expanded: false,
            file_count: 1,
        };
        let album = TagTreeNode {
            label: "A1".into(),
            children: vec![t1, t2],
            file_paths: vec![],
            is_expanded: false,
            file_count: 2,
        };
        let genre = TagTreeNode {
            label: "R".into(),
            children: vec![album],
            file_paths: vec![],
            is_expanded: false,
            file_count: 2,
        };
        let mut m = HashSet::new();
        m.insert(PathBuf::from("/a.mp3"));
        let r = prune_tag_node(&genre, &m);
        assert!(r.is_some());
        let root = r.unwrap();
        assert_eq!(root.file_count, 1);
        assert_eq!(root.children[0].children.len(), 1);
    }

    #[test]
    fn test_prune_tag_node_file_count_updated() {
        let tracks: Vec<TagTreeNode> = (0..5)
            .map(|i| TagTreeNode {
                label: format!("T{}", i),
                children: vec![],
                file_paths: vec![PathBuf::from(format!("/t{}.mp3", i))],
                is_expanded: false,
                file_count: 1,
            })
            .collect();
        let mut m = HashSet::new();
        m.insert(PathBuf::from("/t0.mp3"));
        m.insert(PathBuf::from("/t3.mp3"));
        let r = prune_tag_node(
            &TagTreeNode {
                label: "g".into(),
                children: tracks,
                file_paths: vec![],
                is_expanded: false,
                file_count: 5,
            },
            &m,
        );
        assert!(r.is_some());
        assert_eq!(r.unwrap().file_count, 2);
    }

    #[test]
    fn test_prune_tag_node_intermediate_empty_paths() {
        let track = TagTreeNode {
            label: "T".into(),
            children: vec![],
            file_paths: vec![PathBuf::from("/match.mp3")],
            is_expanded: false,
            file_count: 1,
        };
        let album = TagTreeNode {
            label: "A".into(),
            children: vec![track],
            file_paths: vec![],
            is_expanded: false,
            file_count: 1,
        };
        let mut m = HashSet::new();
        m.insert(PathBuf::from("/match.mp3"));
        assert!(prune_tag_node(&album, &m).is_some());
    }
}
