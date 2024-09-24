use crate::{ast::Definitions, lsp::TermType, todo::Definition, Uuid};
use anyhow::Result;
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

type Name = String;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DefenitionsDB {
    pub by_uuid: FxHashMap<Uuid, Definition>,
    by_file: FxHashMap<PathBuf, Vec<Uuid>>,
    pub structs: FxHashMap<Name, Uuid>,
    pub funcs: FxHashMap<Name, Uuid>,
    pub enums: FxHashMap<Name, Uuid>,
    pub vars: FxHashMap<Name, Uuid>,
    pub mods: FxHashMap<Name, Uuid>,
    names: Vec<(Name, Uuid)>,
}

impl DefenitionsDB {
    pub fn insert(&mut self, defs: &Definitions) {
        defs.structs.iter().for_each(|s| {
            let uuid = Uuid::new_v4();
            let file = s.location.file_path.clone();
            let name = s.name.clone();

            self.by_uuid.insert(uuid, Definition::Struct(s.clone()));
            self.structs.insert(name.clone(), uuid);
            self.names.push((name, uuid));

            if !self.by_file.contains_key(&file) {
                self.by_file.insert(file.clone(), Vec::new());
            }

            self.by_file.get_mut(&file).unwrap().push(uuid);
        });
        defs.enums.iter().for_each(|e| {
            let uuid = Uuid::new_v4();
            let file = e.location.file_path.clone();
            let name = e.name.clone();

            self.by_uuid.insert(uuid, Definition::Enum(e.clone()));
            self.enums.insert(name.clone(), uuid);
            self.names.push((name, uuid));

            if !self.by_file.contains_key(&file) {
                self.by_file.insert(file.clone(), Vec::new());
            }

            self.by_file.get_mut(&file).unwrap().push(uuid);
        });
        defs.funcs.iter().for_each(|f| {
            let uuid = Uuid::new_v4();
            let file = f.location.file_path.clone();
            let name = f.name.clone();

            self.by_uuid.insert(uuid, Definition::Func(f.clone()));
            self.funcs.insert(name.clone(), uuid);
            self.names.push((name, uuid));

            if !self.by_file.contains_key(&file) {
                self.by_file.insert(file.clone(), Vec::new());
            }

            self.by_file.get_mut(&file).unwrap().push(uuid);
        });
        defs.vars.iter().for_each(|v| {
            let uuid = Uuid::new_v4();
            let file = v.location.file_path.clone();
            let name = v.name.clone();

            self.by_uuid.insert(uuid, Definition::Var(v.clone()));
            self.vars.insert(name.clone(), uuid);
            self.names.push((name, uuid));

            if !self.by_file.contains_key(&file) {
                self.by_file.insert(file.clone(), Vec::new());
            }

            self.by_file.get_mut(&file).unwrap().push(uuid);
        });
        defs.mods.iter().for_each(|m| {
            let uuid = Uuid::new_v4();
            let file = m.location.file_path.clone();
            let name = m.name.clone();

            self.by_uuid.insert(uuid, Definition::Mod(m.clone()));
            self.mods.insert(name.clone(), uuid);
            self.names.push((name, uuid));

            if !self.by_file.contains_key(&file) {
                self.by_file.insert(file.clone(), Vec::new());
            }

            self.by_file.get_mut(&file).unwrap().push(uuid);
        });
    }

    pub fn add_def(&mut self, def: Definition) {
        let uuid = Uuid::new_v4();

        match def.clone() {
            Definition::Struct(s) => {
                self.by_uuid.insert(uuid, def);

                match self.by_file.get_mut(&s.location.file_path) {
                    Some(paths) => paths.push(uuid),
                    None => {
                        self.by_file.insert(s.location.file_path, vec![uuid]);
                    }
                };

                self.names.push((s.name.clone(), uuid));

                self.structs.insert(s.name, uuid);
            }
            Definition::Func(f) => {
                self.by_uuid.insert(uuid, def);

                match self.by_file.get_mut(&f.location.file_path) {
                    Some(paths) => paths.push(uuid),
                    None => {
                        self.by_file.insert(f.location.file_path, vec![uuid]);
                    }
                };

                self.names.push((f.name.clone(), uuid));

                self.funcs.insert(f.name, uuid);
            }
            Definition::Enum(e) => {
                self.by_uuid.insert(uuid, def);

                match self.by_file.get_mut(&e.location.file_path) {
                    Some(paths) => paths.push(uuid),
                    None => {
                        self.by_file.insert(e.location.file_path, vec![uuid]);
                    }
                };

                self.names.push((e.name.clone(), uuid));

                self.enums.insert(e.name, uuid);
            }
            Definition::Var(v) => {
                self.by_uuid.insert(uuid, def);

                match self.by_file.get_mut(&v.location.file_path) {
                    Some(paths) => paths.push(uuid),
                    None => {
                        self.by_file.insert(v.location.file_path, vec![uuid]);
                    }
                };

                self.names.push((v.name.clone(), uuid));

                self.vars.insert(v.name, uuid);
            }
            Definition::Mod(m) => {
                self.by_uuid.insert(uuid, def);

                match self.by_file.get_mut(&m.location.file_path) {
                    Some(paths) => paths.push(uuid),
                    None => {
                        self.by_file.insert(m.location.file_path, vec![uuid]);
                    }
                };

                self.names.push((m.name.clone(), uuid));

                self.structs.insert(m.name, uuid);
            }
        }
    }

    pub fn append(&mut self, other: DefenitionsDB) {
        self.by_uuid.extend(other.by_uuid);
        // BUG: this should add to the vectors individually.
        self.by_file.extend(other.by_file);
        self.names.extend(other.names);
        self.structs.extend(other.structs);
        self.enums.extend(other.enums);
        self.vars.extend(other.vars);
        self.mods.extend(other.mods);
        self.funcs.extend(other.funcs);
    }

    /// purges all definitions from file from the database
    pub fn remove(&mut self, file: PathBuf) -> Result<()> {
        // TODO: write this
        let Some(uuid_to_rm) = self.by_file.remove(&file).clone() else {
            return Ok(());
        };

        let names_to_rm: Vec<Name> = self
            .names
            .iter()
            .filter_map(|(name, id)| {
                if uuid_to_rm.contains(id) {
                    Some(name.to_owned())
                } else {
                    None
                }
            })
            .collect();

        // rm names from structs, enums, funcs, vars, mods
        let rm_f = |hm: &mut FxHashMap<Name, Uuid>| {
            names_to_rm.iter().for_each(|name| {
                hm.remove(name);
            })
        };
        rm_f(&mut self.structs);
        rm_f(&mut self.enums);
        rm_f(&mut self.funcs);
        rm_f(&mut self.vars);
        rm_f(&mut self.mods);

        // rm from names
        self.names.retain(|(name, _id)| !names_to_rm.contains(name));

        // rm from by_uuid
        uuid_to_rm.iter().for_each(|id| {
            self.by_uuid.remove(id);
        });

        Ok(())
    }

    pub fn get(&mut self, query_name: &str) -> Option<Definition> {
        let matcher = SkimMatcherV2::default();

        if let Some((_score, (_name, uuid))) = self
            .names
            .iter()
            .filter_map(|(name, uuid)| {
                if let Some(score) = matcher.fuzzy_match(query_name, name) {
                    Some((score, (name, uuid)))
                } else {
                    None
                }
            })
            .max_by_key(|(i, _)| *i)
        {
            Some(self.by_uuid.get(uuid)?.to_owned())
        } else {
            None
        }
    }

    pub fn get_type(&mut self, query_name: &str, term_type: TermType) -> Option<Definition> {
        let matcher = SkimMatcherV2::default();

        let uuid = match term_type {
            TermType::Structure => {
                if let Some((_score, name)) = self
                    .structs
                    .keys()
                    .filter_map(|name| {
                        if let Some(score) = matcher.fuzzy_match(query_name, name) {
                            Some((score, name))
                        } else {
                            None
                        }
                    })
                    .max_by_key(|(i, _)| *i)
                {
                    Some(self.structs.get(name)?.to_owned())
                } else {
                    None
                }
            }
            TermType::Function => {
                if let Some((_score, name)) = self
                    .funcs
                    .keys()
                    .filter_map(|name| {
                        if let Some(score) = matcher.fuzzy_match(query_name, name) {
                            Some((score, name))
                        } else {
                            None
                        }
                    })
                    .max_by_key(|(i, _)| *i)
                {
                    Some(self.funcs.get(name)?.to_owned())
                } else {
                    None
                }
            }
            TermType::Module => {
                if let Some((_score, name)) = self
                    .mods
                    .keys()
                    .filter_map(|name| {
                        if let Some(score) = matcher.fuzzy_match(query_name, name) {
                            Some((score, name))
                        } else {
                            None
                        }
                    })
                    .max_by_key(|(i, _)| *i)
                {
                    Some(self.mods.get(name)?.to_owned())
                } else {
                    None
                }
            }
            TermType::Enum => {
                if let Some((_score, name)) = self
                    .enums
                    .keys()
                    .filter_map(|name| {
                        if let Some(score) = matcher.fuzzy_match(query_name, name) {
                            Some((score, name))
                        } else {
                            None
                        }
                    })
                    .max_by_key(|(i, _)| *i)
                {
                    Some(self.enums.get(name)?.to_owned())
                } else {
                    None
                }
            }
            TermType::Var => {
                if let Some((_score, name)) = self
                    .vars
                    .keys()
                    .filter_map(|name| {
                        if let Some(score) = matcher.fuzzy_match(query_name, name) {
                            Some((score, name))
                        } else {
                            None
                        }
                    })
                    .max_by_key(|(i, _)| *i)
                {
                    Some(self.vars.get(name)?.to_owned())
                } else {
                    None
                }
            }
        }?;

        Some(self.by_uuid.get(&uuid)?.to_owned())
    }
}
