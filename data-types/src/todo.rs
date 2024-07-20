use serde::{Deserialize, Serialize};
use std::{path::PathBuf, str::FromStr};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct FileLocation {
    pub file: PathBuf,
    pub line_num: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Todo {
    pub todo_type: TodoType,
    // pub uuid: Uuid,
    pub uuid: String,
    pub message: String,
    pub file_loc: FileLocation,
    pub scope: Scope,
    // pub : Option<>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Function {
    /// the name of the functions
    pub name: String,
    /// if the function is syncronouse or async (false if async)
    pub asyncro: bool,
    /// the location with in the file
    pub file_loc: FileLocation,
    // pub args: Vec<FunctionParam>,
    pub return_type: Option<String>,
    pub def_line: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct FunctionParam {
    pub name: String,
    pub param_type: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Struct {
    /// the location with in the file
    pub file_loc: FileLocation,
    pub name: String,
    pub def_line: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Enum {
    /// the location with in the file
    pub file_loc: FileLocation,
    pub name: String,
    pub def_line: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Scope {
    #[default]
    Global,
    Function(Function),
    Struct(Struct),
    Enum(Enum),
    // TODO: add Macro & Var
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum TodoType {
    /// generic TODO
    #[default]
    Todo,
    /// indicates something that needs fixing & isn't compilling or is jank
    FixMe,
    /// indicates something compiles but isn't working correctly/as-intended and needs fixing
    Bug,
    /// a note about implementation (why something works, how something could be improved, etc)
    Note,
    /// indicates that something is a hack and should be fixed eventually
    Hack,
    /// means that the code needs to be optimized
    Optimize,
    Untitled,
}

impl FromStr for TodoType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let s = s.to_lowercase().replace(":", "");

        Ok(if s == "todo" {
            TodoType::Todo
        } else if s == "fixme" {
            TodoType::FixMe
        } else if s == "bug" {
            TodoType::Bug
        } else if s == "note" {
            TodoType::Note
        } else if s == "hack" {
            TodoType::Hack
        } else if s == "optimize" {
            TodoType::Optimize
        } else {
            return Err(format!("not a recognized todo type {s}"));
        })
    }
}

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
