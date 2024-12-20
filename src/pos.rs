use std::{collections::HashMap, fs};
use serde_hjson::{from_str, Value};
use serde::Deserialize;
use std::fmt;

#[derive(Debug, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct PartOfSpeech {
    key: String,
    english_name: String,
    hint: String,         
    examples: Vec<String>, 
}

impl fmt::Display for PartOfSpeech {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{} - {}]",
            self.key, self.english_name
        )
    }
}

#[derive(Debug, Deserialize)]
struct PosData {
    pos: Vec<PartOfSpeech>,
}

#[derive(Debug)]
pub struct TreeNode {
    pub value: Option<PartOfSpeech>,
    pub children: HashMap<String, TreeNode>,
}

impl TreeNode {
    pub fn new(value: Option<PartOfSpeech>) -> Self {
        Self {
            value,
            children: HashMap::new(),
        }
    }

    /// Insert a path into the tree
    pub fn insert(&mut self, path: &str, value: &PartOfSpeech) {
        let mut current_node = self;
        for part in path.split("->") {
            current_node = current_node
                .children
                .entry(part.to_string())
                .or_insert_with(|| TreeNode::new(Some(value.clone())));
        }
    }

    /// Print the tree structure (for debugging)
    pub fn print_tree(&self, depth: usize) {
        let indent = " ".repeat(depth * 2);
        if let Some(ref value) = self.value {
            println!("{}{}", indent, value);
        } else {
            println!("{}Root", indent);
        }
        for child in self.children.values() {
            child.print_tree(depth + 1);
        }
    }
}

pub fn load_tree() -> Result<TreeNode, String> {
    let pos_file = fs::read_to_string("lib/pos.hjson")
        .map_err(|err| format!("Error opening file: {}", err))?;

    let pos_data: PosData = from_str(&pos_file).map_err(|err| format!("Invalid pos.hjson: {}", err))?;

    let mut root = TreeNode::new(None);

    for p in pos_data.pos {
        root.insert(&p.key, &p);
    }

    Ok(root)
}
