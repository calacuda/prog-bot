use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum LspCommand {
    FindDef(LspFindDef),
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct LspFindDef {
    /// the thing to find
    pub term: String,
    pub term_type: Option<TermType>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum TermType {
    Structure,
    Module,
    Enum,
    Var,
    Function,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum LspResponse {
    FoundDef(LspFoundDef),
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct LspFoundDef {
    pub term: String,
    pub term_type: TermType,
    pub file: PathBuf,
    pub start: (usize, usize),
    pub end: (usize, usize),
}
