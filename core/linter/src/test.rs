use crate::process_messages;
use anyhow::{bail, Result};
use futures_util::StreamExt;
use prog_bot_common::connect_to_messagebus;
use prog_bot_data_types::{ProgBotMessage, ProgBotMessageType};
use std::{collections::HashSet, env::current_dir};
use tokio::{spawn, sync::mpsc::unbounded_channel};
use tokio_tungstenite::tungstenite::Message;

async fn test_set_1() -> Result<()> {
    // start_logging()?;
    // connect to mb (for reading)
    let mut sub_to = HashSet::new();
    sub_to.insert(ProgBotMessageType::Speak);
    let (_uuid, (mut _write, mut read)) = connect_to_messagebus(sub_to).await?;
    // info!("connected 1");

    // connect to mb (for test sending)
    let sub_to = HashSet::new();
    let (uuid, (write, mut _read)) = connect_to_messagebus(sub_to).await?;

    let (from_mb_tx, from_mb_rx) = unbounded_channel();
    spawn(process_messages(from_mb_rx, write, uuid));

    let mut path = current_dir()?;
    path.push("test-data");
    path.push("linter-test-1");

    from_mb_tx.send(path)?;

    // recv speak messages and check for correctness
    let msg = read.next().await.unwrap();
    // warn!("{:?}", msg);

    match msg? {
        Message::Text(msg) => {
            let msg: ProgBotMessage = serde_json::from_str(&msg)?;
            assert!(msg.msg_type == ProgBotMessageType::Speak);
        }
        _ => bail!("message bus returned invaluid message type"),
    }

    Ok(())
}

#[tokio::test]
async fn linter_test_set_1() {
    let res = test_set_1().await;

    println!("{res:?}");

    assert!(res.is_ok())
}
