use anyhow::Result;
use tokio::fs::read_to_string;

use crate::ast::Todos;

async fn parser_runner(test_file: &str) -> Result<()> {
    // TODO: read in ./test-data/todo-test-1/src/main.rs
    let file_contents = read_to_string(test_file).await?;

    // TODO: parse tokens
    let res = Todos::from_source(&file_contents, test_file.into());

    println!("{res:?}");
    let n_todos = res?.len();

    assert!(n_todos == 16);

    Ok(())
}

#[tokio::test]
async fn parser() {
    let res = parser_runner("test-data/todo-test-1/src/main.rs").await;

    assert!(res.is_ok());
    // assert!(1 == 0);
}
