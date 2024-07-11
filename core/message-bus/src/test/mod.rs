use super::*;
use anyhow::{bail, Result};
use core::panic;
use futures_util::{sink::SinkExt, StreamExt};
use prog_bot_common::connect_to_messagebus;
use std::collections::HashSet;
use tokio::{select, time::timeout};
use tokio_tungstenite::tungstenite::Message;

async fn server_syn_ack_runner() -> Result<()> {
    // sleep(Duration::from_secs(2)).await;
    let (_uuid, (_write, _read)) = connect_to_messagebus(HashSet::new()).await?;

    Ok(())
}

#[actix::test]
async fn server_syn_ack() {
    // let _ = start_logging();
    let res = server_syn_ack_runner().await;

    println!("{res:?}");

    assert!(!res.is_err())
}

async fn server_subscribe_to_positive_runner() -> Result<()> {
    // start_logging();
    let mut sub_to = HashSet::new();
    sub_to.insert(ProgBotMessageType::Speak);
    let (uuid, (mut write, mut read)) = connect_to_messagebus(sub_to).await?;

    println!("connected");

    let mesg = ProgBotMessage {
        msg_type: ProgBotMessageType::Speak,
        data: serde_json::to_value("hello world!")?,
        context: ProgBotMessageContext {
            sender: Some(uuid),
            response_to: None,
        },
    };

    sleep(Duration::from_secs(1)).await;
    println!("message sending");

    if let Err(e) = write
        .send(Message::Text(serde_json::to_string(&mesg)?))
        .await
    {
        info!("failed to send message to message bus. failed with error {e}");
    }

    println!("message sent");

    if let Ok(Some(Ok(message))) = timeout(Duration::from_secs(10), read.next()).await {
        println!("message {message:?}");

        match message {
            Message::Text(msg) => match serde_json::from_str::<ProgBotMessage>(&msg) {
                Ok(msg) => {
                    println!("msg {msg:?}");

                    if msg.msg_type != ProgBotMessageType::Speak {
                        let msg = "read message successfully but message had wrong type.";
                        println!("{msg}");
                        bail!(msg);
                    } else {
                        Ok(())
                    }
                }
                Err(e) => {
                    println!("{e}");
                    bail!("failed to decipher message bus. failed with error {e}");
                }
            },
            _ => unreachable!("this message should never be returned"),
        }
    } else {
        let msg = "failed to read message";
        error!(msg);
        bail!(msg);
    }
}

#[actix::test]
async fn server_subscribe_to_positive() {
    // let _ = start_logging();
    // let res = server_syn_ack_runner().await;

    // sleep(Duration::from_secs(2)).await;

    let res = server_subscribe_to_positive_runner().await;
    println!("{res:?}");

    assert!(res.is_ok())
}

async fn server_subscribe_to_negative_runner() -> Result<()> {
    let mut sub_to = HashSet::new();
    sub_to.insert(ProgBotMessageType::Spoken);
    let (uuid, (mut write, mut read)) = connect_to_messagebus(sub_to).await?;

    let mesg = ProgBotMessage {
        msg_type: ProgBotMessageType::Speak,
        data: serde_json::to_value("hello world!").unwrap(),
        context: ProgBotMessageContext {
            sender: Some(uuid),
            response_to: None,
        },
    };

    if let Err(e) = write
        .send(Message::Text(serde_json::to_string(&mesg).unwrap()))
        .await
    {
        println!("failed to send message to message bus. failed with error {e}");
    }

    // sleep(Duration::from_secs(1)).await;

    select! {
        message = timeout(Duration::from_secs(4), read.next()) => {
            println!("{message:?}");

            if let Ok(msg) = message {
                bail!("received a message that should not have been received. {msg:?}")
            } else {
                Ok(())
            }
        },
        // _ = timeout(Duration::from_secs(5), start_test_server(2)) => {
        //     bail!("server got timed out")
        // }
    }
}

#[actix::test]
async fn server_subscribe_to_negative() {
    // let _ = start_logging();
    // let res = server_syn_ack_runner().await;
    // start_test_server(2).await;

    let res = server_subscribe_to_negative_runner().await;
    println!("{res:?}");

    assert!(res.is_ok())
}
