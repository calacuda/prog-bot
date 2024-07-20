use crate::{Enum, FileLocation, Function, Scope, Struct, Todo, TodoType};
use anyhow::{bail, Result};
use pest::{iterators::Pairs, Parser};
use prog_bot_data_types::{
    ast::Definitions,
    todo::{EnumDef, FuncDef, StructDef},
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, str::FromStr};
use tracing::*;

type Output = Result<Definitions>;

#[derive(
    Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, pest_derive::Parser,
)]
#[grammar = "todo-grammar.pest"]
pub struct Todos;

impl Todos {
    pub fn from_source(source: &str, file_path: PathBuf) -> Output {
        trace!("Compiling the source: {}", source);
        // println!("Compiling the source: {}", source);
        let tokens = Todos::parse(Rule::tokens, source); // parser::parse(source).unwrap();
        trace!("tokens => {:?}", tokens);
        // println!("tokens => {:?}", tokens);
        Self::get_todos_from_scope(tokens?, file_path)
    }

    fn get_todos_from_scope(tokens: Pairs<Rule>, file_path: PathBuf) -> Output {
        // let mut todos = Vec::new();
        let mut defs = Definitions::default();
        let mut scope = vec![Scope::default()];
        let mut next_todo = Todo::default();
        next_todo.file_loc.file = file_path.clone();
        // println!("parsing {file_path:?}");
        let mut def_line = String::new();

        for token in tokens.flatten() {
            debug!("{:?}", token);
            // println!("token {:?}", token.as_rule());
            // println!("token {:?}", token);
            println!("{:?} => scope len: {}", token.as_rule(), scope.len());

            if token.as_rule() == Rule::func {
                println!("{}", token.as_str());
            }

            // make todos struct
            match token.as_rule() {
                Rule::todo => {
                    // println!("{:?}", token.as_rule());
                    // println!("{:?}", token.as_node_tag());
                    next_todo.scope = scope[scope.len() - 1].clone();
                    defs.todos.push(next_todo);
                    next_todo = Todo::default();
                    next_todo.file_loc.line_num = token.line_col().0;
                }
                Rule::todo_uuid => next_todo.uuid = token.as_str().replace("// ", "").into(),
                Rule::todo_label => match TodoType::from_str(token.as_str()) {
                    Ok(tdt) => next_todo.todo_type = tdt,
                    Err(e) => bail!(e),
                },
                Rule::todo_body => next_todo.message = token.as_str().replace("// ", "").into(),
                Rule::struct_name => scope.push({
                    let mut file_loc = FileLocation::default();
                    file_loc.line_num = token.line_col().0;
                    file_loc.file = file_path.clone();

                    Scope::Struct(Struct {
                        name: token.as_str().into(),
                        file_loc,
                        def_line: def_line.clone(),
                    })
                }),
                Rule::struct_start | Rule::enum_start => {
                    def_line = token.as_str().into();
                    // let mut most_recent_scope = scope.pop().unwrap();
                    // // println!("{}", token.as_str());
                    //
                    // match most_recent_scope {
                    //     Scope::Struct(ref mut s) => s.def_line = token.as_str().into(),
                    //     Scope::Enum(ref mut e) => e.def_line = token.as_str().into(),
                    //     _ => {}
                    // }
                    //
                    // println!("mrs = {most_recent_scope:?}");
                    //
                    // scope.push(most_recent_scope);
                }
                Rule::struct_end | Rule::func_end | Rule::enum_end => {
                    println!("scope done: {:?}", token.as_rule());

                    if let Some(struct_scope) = scope.pop() {
                        // println!("adding def to defs");

                        match struct_scope {
                            Scope::Global => {}
                            Scope::Function(scope) => {
                                let f = FuncDef {
                                    name: scope.name,
                                    is_pub: scope.def_line.replace(" ", "").starts_with("pub"),
                                    is_async: scope.asyncro,
                                    location: scope.file_loc.into(),
                                };

                                defs.funcs.push(f);
                            }
                            Scope::Enum(scope) => {
                                let e = EnumDef {
                                    name: scope.name,
                                    location: scope.file_loc.into(),
                                    is_pub: scope.def_line.starts_with("pub"),
                                };

                                defs.enums.push(e);
                            }
                            Scope::Struct(scope) => {
                                // println!("struct line def |{}|", scope.def_line);
                                let s = StructDef {
                                    name: scope.name,
                                    location: scope.file_loc.into(),
                                    is_pub: scope.def_line.starts_with("pub"),
                                };

                                defs.structs.push(s);
                            }
                        }
                    } else {
                        unreachable!("unknown scope");
                    }
                }
                Rule::enum_name => scope.push({
                    let mut file_loc = FileLocation::default();
                    file_loc.line_num = token.line_col().0;
                    file_loc.file = file_path.clone();

                    Scope::Enum(Enum {
                        name: token.as_str().into(),
                        file_loc,
                        def_line: def_line.clone(),
                    })
                }),
                // Rule::enum_end => {
                //     scope.pop();
                // }
                Rule::func => scope.push(Scope::Function({
                    let mut f = Function::default();
                    let mut file_loc = FileLocation::default();
                    file_loc.line_num = token.line_col().0;
                    file_loc.file = file_path.clone();

                    f.file_loc = file_loc;
                    f.asyncro = token.as_str().starts_with("async")
                        || token.as_str().starts_with("pub async");

                    f.def_line = token.as_str().into();
                    // println!("func :  {f:?}");

                    f
                })),
                Rule::func_name => {
                    if let Some(thing) = scope.pop() {
                        if let Scope::Function(mut f) = thing {
                            f.name = token.as_str().into();

                            scope.push(Scope::Function(f));
                        } else {
                            scope.push(thing)
                        }
                    }
                }
                // Rule::func_end => {
                //     scope.pop();
                // }
                _ => {}
            }
        }

        next_todo.scope = scope[scope.len() - 1].clone();
        defs.todos.push(next_todo);

        defs.todos = defs.todos[1..].to_vec();

        Ok(defs)
    }
}
