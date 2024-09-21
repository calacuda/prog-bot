use anyhow::Result;
use prog_bot_common::start_logging;
use tokio::fs::read_to_string;

use crate::ast::Todos;

async fn parser_runner(test_file: &str) -> Result<()> {
    // read in ./test-data/todo-test-1/src/main.rs
    let file_contents = read_to_string(test_file).await?;

    // parse tokens
    let res = Todos::from_source(&file_contents, test_file.into());

    println!("{res:?}");
    let n_todos = res?.todos.len();
    let expected_n_todos = 16;

    assert_eq!(
        n_todos, expected_n_todos,
        "wrong number fo todos found. expected, {expected_n_todos}, found {n_todos}.",
    );

    Ok(())
}

#[tokio::test]
async fn parser() {
    let res = parser_runner("test-data/todo-test-1/src/main.rs").await;

    assert!(res.is_ok());
}

#[tokio::test]
async fn message_bus() {
    // TODO: write
}

#[tokio::test]
async fn lsp_test() {
    let test_file = "test-data/lsp-test-1/src/main.rs";

    let file_contents = read_to_string(test_file).await;

    assert!(file_contents.is_ok(), "failed to read file contents");

    let file_contents = file_contents.unwrap();

    // start_logging();
    // parse tokens
    let res = Todos::from_source(&file_contents, test_file.into());

    println!("{res:#?}");
    assert!(res.is_ok(), "failed to parse src code file.");

    let res = res.unwrap();
    // assert!(false);

    assert_eq!(
        res.funcs.len(),
        13,
        "wrong number of functions found, expected 13 found {}",
        res.funcs.len()
    );

    assert_eq!(
        res.enums.len(),
        2,
        "wrong number of enums found, expected 2 found {}",
        res.enums.len()
    );

    assert_eq!(
        res.structs.len(),
        3,
        "wrong number of structs found, expected 3 found {}",
        res.structs.len()
    );
}
