use crate::FileLocation;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// #[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
// pub struct Definitions {
//     pub structs: Vec<StructDef>,
//     pub funcs: Vec<FuncDef>,
//     pub enums: Vec<EnumDef>,
//     pub vars: Vec<VarDef>,
//     pub mods: Vec<ModuleDef>,
// }

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Location {
    pub file_path: PathBuf,
    // /// line and column number of the start of the term. in the format (line, column)
    // pub start: (usize, usize),
    // /// line and column number of the start of the term. in the format (line, column)
    // pub end: (usize, usize),
    pub start: usize,
}

impl From<FileLocation> for Location {
    fn from(value: FileLocation) -> Self {
        Self {
            file_path: value.file,
            start: value.line_num,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct StructDef {
    pub name: String,
    pub is_pub: bool,
    pub location: Location,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct FuncDef {
    pub name: String,
    pub is_pub: bool,
    pub is_async: bool,
    pub location: Location,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct EnumDef {
    pub name: String,
    pub is_pub: bool,
    pub location: Location,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct VarDef {
    pub name: String,
    pub is_mut: bool,
    pub data_type: Option<String>,
    pub location: Location,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ModuleDef {
    pub name: String,
    pub is_pub: bool,
    pub location: Location,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Definition {
    Struct(StructDef),
    Func(FuncDef),
    Enum(EnumDef),
    Var(VarDef),
    Mod(ModuleDef),
}
