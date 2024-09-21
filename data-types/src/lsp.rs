use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::{database::DefenitionsDB, todo::Definition};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum LspCommand {
    FindDef(LspFindDef),
    GetAllDefs,
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

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum LspResponse {
    FoundDef(Definition),
    AllDefs(DefenitionsDB),
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct LspFoundDef {
    pub term: String,
    pub term_type: TermType,
    pub is_pub: bool,
    pub file: PathBuf,
    pub start: (usize, usize),
    pub end: (usize, usize),
}
