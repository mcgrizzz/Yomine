use std::{
    collections::HashSet,
    path::PathBuf,
};

use super::models::{
    FileTreeNode,
    FileTreeState,
};

pub struct FileTreeBuilder;

impl FileTreeBuilder {
    pub fn build_tree(files: &[PathBuf]) -> FileTreeState {
        if files.is_empty() {
            return FileTreeState::default();
        }

        // "/home/andrew/project/src/main.rs" -> ["", "home", "andrew", "project", "src", "main.rs"]
        let components: Vec<Vec<String>> = files
            .iter()
            .map(|p| {
                p.components()
                    .filter_map(|c| c.as_os_str().to_str().map(ToOwned::to_owned))
                    .collect()
            })
            .collect();

        let common_depth = Self::find_common_ancestor_depth(&components);
        let display_depth = common_depth;

        let mut root = FileTreeNode { name: String::new(), path: None, children: Vec::new() };

        // Insert each file into the tree
        for (path, parts) in files.iter().zip(components.iter()) {
            let visible_parts = &parts[display_depth..];
            if !visible_parts.is_empty() {
                Self::insert_path(&mut root, visible_parts, path.clone());
            }
        }

        Self::sort_children(&mut root);

        let selected: HashSet<PathBuf> = files.iter().cloned().collect();

        FileTreeState { root: Some(root), selected }
    }

    fn find_common_ancestor_depth(paths: &[Vec<String>]) -> usize {
        if paths.is_empty() {
            return 0;
        }

        let first = &paths[0];

        (0..first.len())
            .take_while(|&depth| paths.iter().all(|p| p.get(depth) == first.get(depth)))
            .count()
    }

    fn insert_path(node: &mut FileTreeNode, parts: &[String], full_path: PathBuf) {
        match parts.split_first() {
            None => {}
            Some((segment, rest)) if rest.is_empty() => {
                // Leaf (file)
                node.children.push(FileTreeNode {
                    name: segment.clone(),
                    path: Some(full_path),
                    children: Vec::new(),
                });
            }
            Some((segment, rest)) => {
                let existing_idx =
                    node.children.iter().position(|c| c.path.is_none() && c.name == *segment);

                let child = match existing_idx {
                    Some(idx) => &mut node.children[idx],
                    None => {
                        node.children.push(FileTreeNode {
                            name: segment.clone(),
                            path: None,
                            children: Vec::new(),
                        });
                        node.children.last_mut().unwrap()
                    }
                };

                Self::insert_path(child, rest, full_path);
            }
        }
    }

    fn sort_children(node: &mut FileTreeNode) {
        node.children.sort_by(|a, b| a.name.cmp(&b.name));
        for child in &mut node.children {
            Self::sort_children(child);
        }
    }
}
