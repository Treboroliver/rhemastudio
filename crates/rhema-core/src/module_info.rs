use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModuleKind {
    Bible,
    Commentary,
    Dictionary,
    Book,
    Devotional,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleInfo {
    pub id: String,
    pub path: PathBuf,
    pub kind: ModuleKind,
    pub title: String,
    pub abbreviation: Option<String>,
    pub language: Option<String>,
    pub has_strong: bool,
    pub props: HashMap<String, String>,
}
