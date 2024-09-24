use anyhow::Result;
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use prog_bot_common::{connect_to_messagebus, start_logging};
use prog_bot_data_types::{ProgBotMessage, ProgBotMessageContext, ProgBotMessageType, Uuid};
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    process::Command,
    spawn,
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    time::sleep,
};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};
use tracing::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    start_logging()?;

    let mut sub_to = HashSet::new();
    sub_to.insert(ProgBotMessageType::Speak);
    // will emitt ProgBotMessageType::TodoFound and TodoEdited

    let (uuid, (write, read)) = connect_to_messagebus(sub_to).await?;

    info!("TTS engine connected to message-bus. assigned uuid: {uuid}");

    let (from_mb_tx, from_mb_rx) = unbounded_channel();
    // let (to_mb_tx, to_mb_rx) = unbounded_channel::<ProgBotMessage>();

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        info!("terminating TTS node");
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    // start recv thread to recv messages from message bus
    let r = running.clone();

    spawn(async move {
        if let Err(e) = talk_with_message_bus(from_mb_tx, read).await {
            error!("connection with messagebus encountered an error: {e}");
            r.store(false, Ordering::SeqCst);
        }
    });

    // start process thread to process messages from the message bus
    let r = running.clone();

    spawn(async move {
        if let Err(e) = process_messages(from_mb_rx, write, uuid).await {
            error!("connection with messagebus encountered an error: {e}");
            r.store(false, Ordering::SeqCst);
        }
    });

    info!("TTS node started");

    while running.load(Ordering::SeqCst) {
        // debug!("waiting...");
        sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}

pub async fn talk_with_message_bus(
    tx: UnboundedSender<String>,
    mut reader: SplitStream<WebSocketStream<impl AsyncRead + AsyncWrite + Unpin>>,
) -> Result<()> {
    while let Some(websocket_message) = reader.next().await {
        match websocket_message {
            Ok(Message::Text(raw_message)) => {
                if let Ok(message) = serde_json::from_str::<ProgBotMessage>(&raw_message) {
                    if let Ok(utterance) = serde_json::from_value(message.data) {
                        tx.send(utterance)?;
                    } else {
                        error!(
                            "unable to to parse the data field of a received message as a utterance"
                        );
                    }
                } else {
                    error!("serde_json failed to deserialize a message: {raw_message}");
                }
            }
            Ok(_) => error!("received an invalid websocket message type. HINT: only text is valid"),
            Err(e) => {
                error!("websocket messaged was an error: {e} (IDK, your guess is as good as mine)")
            }
        }
    }

    Ok(())
}

pub async fn process_messages(
    mut rx: UnboundedReceiver<String>,
    mut writer: SplitSink<WebSocketStream<impl AsyncRead + AsyncWrite + Unpin>, Message>,
    uuid: Uuid,
) -> Result<()> {
    while let Some(utterance) = rx.recv().await {
        match Command::new("mimic3").arg(&utterance).output().await {
            Ok(_) => {
                let prog_msg = ProgBotMessage {
                    msg_type: ProgBotMessageType::Spoken,
                    context: ProgBotMessageContext {
                        sender: Some(uuid),
                        response_to: None,
                    },
                    data: serde_json::to_value(utterance)?,
                };

                let _ = writer
                    .send(Message::Text(serde_json::to_string(&prog_msg)?))
                    .await;
            }
            Err(e) => error!("attempting to speak resulted in error: {e}"),
        }
    }

    Ok(())
}
