use anyhow::Result;
use ast::Todos;
use data_types::Definition;
use database::DefenitionsDB;
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use prog_bot_common::{connect_to_messagebus, start_logging};
use prog_bot_data_types::{
    lsp::LspCommand, ProgBotMessage, ProgBotMessageContext, ProgBotMessageType, Uuid,
};
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
    fs::read_to_string,
    io::{AsyncRead, AsyncWrite},
    select, spawn,
    sync::{
        mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
        Mutex,
    },
    time::{sleep, Duration},
};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};
use tracing::*;

pub mod ast;
mod data_types;
mod database;
#[cfg(test)]
mod test;

type Defs = DefenitionsDB;

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
    pub return_type: Option<String>,
    pub def_line: String,
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
    pub def_line: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Enum {
    /// the location with in the file
    pub file_loc: FileLocation,
    pub name: String,
    pub def_line: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Scope {
    #[default]
    Global,
    Function(Function),
    Struct(Struct),
    Enum(Enum),
    // TODO: add Macro & Var
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
    sub_to.insert(ProgBotMessageType::LspCommand);

    // will emitt ProgBotMessageType::TodoFound and TodoEdited
    let (uuid, (write, read)) = connect_to_messagebus(sub_to).await?;

    info!("TODO-Collector/LSP connected to message-bus. assigned uuid: {uuid}");

    let (from_mb_tx, from_mb_rx) = unbounded_channel();
    let (from_mb_tx_lsp, from_mb_rx_lsp) = unbounded_channel();
    let (from_file_tx, from_file_rx) = unbounded_channel();
    let (from_lsp_tx, from_lsp_rx) = unbounded_channel();

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        info!("terminating TODO-Collector/LSP node");
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    // start recv thread to recv messages from message bus
    let r = running.clone();
    let defs = Arc::new(Mutex::new(Defs::default()));

    spawn(async move {
        if let Err(e) = talk_with_message_bus(from_mb_tx, from_mb_tx_lsp, read).await {
            error!("connection with messagebus encountered an error: {e}");
            r.store(false, Ordering::SeqCst);
        }
    });

    // start process thread to process messages from the message bus
    let r = running.clone();
    let defs_clone = defs.clone();

    spawn(async move {
        if let Err(e) = process_files(from_mb_rx, from_file_tx, uuid, defs_clone).await {
            error!("processing file open, close, and edit messages failed with error: {e}");
            r.store(false, Ordering::SeqCst);
        }
    });

    let r = running.clone();
    let defs_clone = defs.clone();

    spawn(async move {
        if let Err(e) = process_lsp(from_mb_rx_lsp, from_lsp_tx, uuid, defs_clone).await {
            error!("processing lsp messages failed with error: {e}");
            r.store(false, Ordering::SeqCst);
        }
    });

    let r = running.clone();

    spawn(async move {
        if let Err(e) = send_to_mb(from_lsp_rx, from_file_rx, write).await {
            error!("sending messages to message_bus failed with error: {e}");
            r.store(false, Ordering::SeqCst);
        }
    });

    info!("TODO/LSP node started");

    while running.load(Ordering::SeqCst) {
        // debug!("waiting...");
        sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}

pub async fn talk_with_message_bus(
    tx: UnboundedSender<PathBuf>,
    lsp_tx: UnboundedSender<LspCommand>,
    mut reader: SplitStream<WebSocketStream<impl AsyncRead + AsyncWrite + Unpin>>,
) -> Result<()> {
    while let Some(websocket_message) = reader.next().await {
        match websocket_message {
            Ok(Message::Text(raw_message)) => {
                if let Ok(message) = serde_json::from_str::<ProgBotMessage>(&raw_message) {
                    match message.msg_type {
                        ProgBotMessageType::LspCommand => {
                            if let Ok(lsp_cmd) = serde_json::from_value(message.data) {
                                lsp_tx.send(lsp_cmd)?;
                            } else {
                                error!(
                                    "unable to to parse the data field of a received message as a PathBuf"
                                );
                            }
                        }
                        _ => {
                            if let Ok(file) = serde_json::from_value(message.data) {
                                tx.send(file)?;
                            } else {
                                error!(
                                    "unable to to parse the data field of a received message as a PathBuf"
                                );
                            }
                        }
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

pub async fn process_files(
    mut rx: UnboundedReceiver<PathBuf>,
    // mut writer: SplitSink<WebSocketStream<impl AsyncRead + AsyncWrite + Unpin>, Message>,
    tx: UnboundedSender<String>,
    uuid: Uuid,
    defs: Arc<Mutex<Defs>>,
) -> Result<()> {
    while let Some(file_path) = rx.recv().await {
        // get todos from file
        let file_contents = read_to_string(file_path.clone()).await?;

        // parse tokens
        if let Ok(definitions) = Todos::from_source(&file_contents, file_path.clone().into()) {
            // send todo messages to server
            let messages: Vec<String> = definitions
                .todos
                .clone()
                .into_iter()
                .filter_map(|todo| {
                    if let Ok(data) = serde_json::to_value(todo) {
                        if let Ok(msg) = serde_json::to_string(&ProgBotMessage {
                            msg_type: ProgBotMessageType::FoundTodo,
                            context: ProgBotMessageContext {
                                sender: Some(uuid),
                                response_to: None,
                            },
                            data,
                        }) {
                            Some(msg)
                        } else {
                            error!("failed to serialize ProgBotMessage");
                            None
                        }
                    } else {
                        error!("failed to serialize todo");
                        None
                    }
                })
                .collect();

            defs.lock().await.insert(&definitions);

            for msg in messages.into_iter() {
                if let Err(e) = tx.send(msg) {
                    error!("failed to allert on new todo. got error: {e}");
                }
            }
        }
    }

    Ok(())
}

pub async fn process_lsp(
    mut rx: UnboundedReceiver<LspCommand>,
    tx: UnboundedSender<String>,
    uuid: Uuid,
    defs: Arc<Mutex<Defs>>,
) -> Result<()> {
    // TODO: parse lsp command and do thing
    while let Some(lsp_cmd) = rx.recv().await {
        match lsp_cmd {
            LspCommand::FindDef(target) => {
                let defs = match target.term_type {
                    Some(term_type) => defs.lock().await.get_type(&target.term, term_type),
                    None => defs.lock().await.get(&target.term),
                };

                if let Some(def) = defs {
                    let (term_type, name, file, line) = match def {
                        Definition::Struct(s) => {
                            ("Struct", s.name, s.location.file_path, s.location.start)
                        }
                        Definition::Func(f) => {
                            ("Function", f.name, f.location.file_path, f.location.start)
                        }
                        Definition::Enum(e) => {
                            ("Enum", e.name, e.location.file_path, e.location.start)
                        }
                        Definition::Mod(m) => {
                            ("Module", m.name, m.location.file_path, m.location.start)
                        }
                        Definition::Var(v) => {
                            ("Variable", v.name, v.location.file_path, v.location.start)
                        }
                    };

                    let file = file.to_string_lossy();
                    let utterance =
                        format!("found {term_type}, {name}, in file, {file} on line {line}");

                    if let Ok(msg) = serde_json::to_string(&ProgBotMessage {
                        msg_type: ProgBotMessageType::Speak,
                        data: serde_json::to_value(utterance).unwrap(),
                        context: ProgBotMessageContext {
                            sender: Some(uuid),
                            response_to: None,
                        },
                    }) {
                        tx.send(msg)?;
                    } else {
                        error!("failed to serialize progbot_message to json string.");
                    }
                } else {
                    if let Ok(msg) = serde_json::to_string(&ProgBotMessage {
                        msg_type: ProgBotMessageType::Speak,
                        data: serde_json::to_value(format!(
                            "could not file the symbol: {} in any of the open files.",
                            target.term
                        ))
                        .unwrap(),
                        context: ProgBotMessageContext {
                            sender: Some(uuid),
                            response_to: None,
                        },
                    }) {
                        tx.send(msg)?;
                    } else {
                        error!("failed to serialize progbot_message to json string.");
                    }
                }
            }
        }
    }

    Ok(())
}

pub async fn send_to_mb(
    mut file_rx: UnboundedReceiver<String>,
    mut lsp_rx: UnboundedReceiver<String>,
    mut writer: SplitSink<WebSocketStream<impl AsyncRead + AsyncWrite + Unpin>, Message>,
) -> Result<()> {
    loop {
        select! {
            msg = file_rx.recv() => {writer.send(Message::Text(msg.unwrap())).await?;},
            msg = lsp_rx.recv() => {writer.send(Message::Text(msg.unwrap())).await?;},
        }
    }
}
