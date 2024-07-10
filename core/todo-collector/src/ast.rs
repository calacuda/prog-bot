use crate::{Enum, FileLocation, Function, Scope, Struct, Todo, TodoType};
use anyhow::{bail, Result};
use pest::{iterators::Pairs, Parser};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, str::FromStr};
use tracing::*;

type Output = Result<Vec<Todo>>;

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
        let mut todos = Vec::new();
        let mut scope = vec![Scope::default()];
        let mut next_todo = Todo::default();
        next_todo.file_loc.file = file_path.clone();

        for token in tokens.flatten() {
            debug!("{:?}", token);

            // make todos struct
            match token.as_rule() {
                Rule::todo => {
                    // println!("{:?}", token.as_rule());
                    // println!("{:?}", token.as_node_tag());
                    next_todo.scope = scope[scope.len() - 1].clone();
                    todos.push(next_todo);
                    next_todo = Todo::default();
                    next_todo.file_loc.line_num = token.line_col().0;
                }
                Rule::todo_uuid => next_todo.uuid = token.as_str().replace("// ", "").into(),
                Rule::todo_label => match TodoType::from_str(token.as_str()) {
                    Ok(tdt) => next_todo.todo_type = tdt,
                    Err(e) => bail!(e),
                },
                Rule::todo_body => next_todo.message = token.as_str().into(),
                Rule::struct_name => scope.push({
                    let mut file_loc = FileLocation::default();
                    file_loc.line_num = token.line_col().0;
                    file_loc.file = file_path.clone();

                    Scope::Struct(Struct {
                        name: token.as_str().into(),
                        file_loc,
                    })
                }),
                Rule::struct_end => {
                    scope.pop();
                }
                Rule::enum_name => scope.push({
                    let mut file_loc = FileLocation::default();
                    file_loc.line_num = token.line_col().0;
                    file_loc.file = file_path.clone();

                    Scope::Enum(Enum {
                        name: token.as_str().into(),
                        file_loc,
                    })
                }),
                Rule::enum_end => {
                    scope.pop();
                }
                Rule::func_declaration => scope.push(Scope::Function({
                    let mut f = Function::default();
                    let mut file_loc = FileLocation::default();
                    file_loc.line_num = token.line_col().0;
                    file_loc.file = file_path.clone();

                    f.file_loc = file_loc;
                    f.asyncro = token.as_str().starts_with("async")
                        || token.as_str().starts_with("pub async");

                    f
                })),
                Rule::func_name => {
                    if let Some(Scope::Function(mut f)) = scope.pop() {
                        f.name = token.as_str().into();

                        scope.push(Scope::Function(f));
                    }
                }
                Rule::func_end => {
                    scope.pop();
                }
                _ => {}
            }
        }

        next_todo.scope = scope[scope.len() - 1].clone();
        todos.push(next_todo);

        Ok(todos[1..].to_vec())
    }
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
