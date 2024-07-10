use anyhow::Result;
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use prog_bot_common::{connect_to_messagebus, start_logging};
use prog_bot_data_types::{ProgBotMessage, ProgBotMessageContext, ProgBotMessageType, Uuid};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    spawn,
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    time::{sleep, Duration},
};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};
use tracing::*;

pub mod ast;
#[cfg(test)]
mod test;

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
    pub return_type: String,
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
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Enum {
    /// the location with in the file
    pub file_loc: FileLocation,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Scope {
    #[default]
    Global,
    Function(Function),
    Struct(Struct),
    Enum(Enum),
    // TODO: add macro
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
}

#[tokio::main]
async fn main() -> Result<()> {
    start_logging()?;

    let mut sub_to = HashSet::new();
    sub_to.insert(ProgBotMessageType::FileSaved);
    sub_to.insert(ProgBotMessageType::FileOpened);
    sub_to.insert(ProgBotMessageType::FileClosed);
    // will emitt ProgBotMessageType::TodoFound and TodoEdited

    let (uuid, (write, read)) = connect_to_messagebus(sub_to).await?;

    info!("TODO-Collector connected to message-bus. assigned uuid: {uuid}");

    let (from_mb_tx, from_mb_rx) = unbounded_channel();
    // let (to_mb_tx, to_mb_rx) = unbounded_channel::<ProgBotMessage>();

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        info!("terminating clippy node");
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

    info!("clippy node started");

    while running.load(Ordering::SeqCst) {
        // debug!("waiting...");
        sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}

pub async fn talk_with_message_bus(
    tx: UnboundedSender<PathBuf>,
    mut reader: SplitStream<WebSocketStream<impl AsyncRead + AsyncWrite + Unpin>>,
) -> Result<()> {
    while let Some(websocket_message) = reader.next().await {
        match websocket_message {
            Ok(Message::Text(raw_message)) => {
                if let Ok(message) = serde_json::from_str::<ProgBotMessage>(&raw_message) {
                    if let Ok(file) = serde_json::from_value(message.data) {
                        tx.send(file)?;
                    } else {
                        error!(
                            "unable to to parse the data field of a received message as a PathBuf"
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
    mut rx: UnboundedReceiver<PathBuf>,
    mut writer: SplitSink<WebSocketStream<impl AsyncRead + AsyncWrite + Unpin>, Message>,
    uuid: Uuid,
) -> Result<()> {
    while let Some(file_path) = rx.recv().await {
        // run cargo check / clippy & record output
        // if let Ok(res) = Command::new("cargo")
        //     .arg("check")
        //     .current_dir(file_path)
        //     .output()
        //     .await
        // {
        //     // strip ansi sequences
        //     let output = strip_ansi_escapes::strip(res.stderr);
        //     let output = String::from_utf8_lossy(&output).to_string();
        //
        //     // parse errors
        //     debug!("{output}");
        //     // for (line_1, line_2) in output.lines().zip(output.lines().skip(1)) {
        //     for lines in (&output
        //         .lines()
        //         .map(|s| s.to_string())
        //         .collect::<Vec<String>>())
        //         .windows(3)
        //     {
        //         if lines[0].is_empty() && lines[1].starts_with("error") {
        //             // Speak
        //             let file_line_num = lines[2]
        //                 .replace("-->", "")
        //                 .replace(".rs:", " dot R S on line ");
        //
        //             let message = format!("{} in file: {file_line_num}", lines[1]);
        //
        //             let message = ProgBotMessage {
        //                 msg_type: ProgBotMessageType::Speak,
        //                 context: ProgBotMessageContext {
        //                     sender: Some(uuid),
        //                     response_to: None,
        //                 },
        //                 data: serde_json::to_value(&message)?,
        //             };
        //
        //             writer
        //                 .send(Message::Text(serde_json::to_string(&message)?))
        //                 .await?;
        //         }
        //     }
        // }
    }

    Ok(())
}
