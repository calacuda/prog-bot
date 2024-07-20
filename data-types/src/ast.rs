use crate::todo::{EnumDef, FuncDef, ModuleDef, StructDef, Todo, VarDef};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Definitions {
    pub todos: Vec<Todo>,
    pub structs: Vec<StructDef>,
    pub funcs: Vec<FuncDef>,
    pub enums: Vec<EnumDef>,
    pub vars: Vec<VarDef>,
    pub mods: Vec<ModuleDef>,
}
