use anyhow::Result;
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use prog_bot_common::connect_to_messagebus;
use prog_bot_data_types::{
    database::DefenitionsDB,
    lsp::{LspCommand, LspFoundDef, LspResponse},
    todo::{Definition, Todo},
    ProgBotMessage, ProgBotMessageContext, ProgBotMessageType, Uuid,
};
use rustc_hash::FxHashMap;
use std::{collections::HashSet, path::PathBuf, sync::Arc};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    spawn,
    sync::{
        mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
        Mutex,
    },
    task::JoinHandle,
};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};
use tracing::*;

pub type Priority = u16;

pub struct AppState {
    // /// a receiver to get messages from the message bus
    // from_mb: UnboundedReceiver<ProgBotMessage>,
    // /// a sender to comunicate with the the message send thread
    // to_mb: UnboundedSender<ProgBotMessage>,
    /// the join handles to the threads comunicating with the message bus
    thread_handles: (JoinHandle<()>, JoinHandle<()>),
    /// a list of all known Todos (in the order they were found in)
    pub todos: Todos,
    /// database of defined struct, enums, functions, etc
    pub def_db: Arc<Mutex<DefenitionsDB>>,
    /// the tabs.
    tabs: Arc<[Tab]>,
    /// an index into tabs
    tab: usize,
    pub logs: Vec<String>,
    /// is the cursor in the body or header.
    pub in_body: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Todos {
    /// uuid to TODO association,
    pub by_uuid: FxHashMap<Uuid, Todo>,
    /// what todos belong to what files.
    pub by_file: FxHashMap<PathBuf, Arc<[Uuid]>>, // is Arc<[Uuid]> bc it will be rewritten every
    // time new data is received from the server.
    /// the priority (the order they should apear in)
    pub priority: Vec<(Priority, Uuid)>,
}

pub enum Tab {
    /// Todo Tab holds scroll position
    Todo(usize),
    /// holds scroll position for functions, structs, enums,
    Defs((usize, usize, usize)),
    /// the logs tabs (holds messagebus logs) (logs not included)
    Logs,
}

impl AppState {
    pub async fn new() -> Result<Self> {
        // let (to_mb, to_mb_rx) = unbounded_channel();
        let (from_mb_tx, from_mb) = unbounded_channel();

        let mut sub_to = HashSet::new();
        sub_to.insert(ProgBotMessageType::LspDefinitionLocation);
        sub_to.insert(ProgBotMessageType::LspResponce);
        sub_to.insert(ProgBotMessageType::FoundTodo);
        // sub_to.insert(ProgBotMessageType::EditTodo);
        sub_to.insert(ProgBotMessageType::FileClosed); // used to rm all defs related to the
                                                       // closed file

        // will emitt ProgBotMessageType::TodoFound and TodoEdited
        let (uuid, (write, read)) = connect_to_messagebus(sub_to).await?;

        let def_db = Arc::new(Mutex::new(DefenitionsDB::default()));
        let defs = def_db.clone();

        // write recv and send threads.
        let send_thread = spawn(async {
            if let Err(e) = talk_with_message_bus(from_mb_tx, read).await {
                eprintln!("connection with messagebus encountered an error: {e}");
            }
        });
        let recv_thread = spawn(async move {
            if let Err(e) = process_messages(from_mb, write, defs.clone(), uuid).await {
                eprintln!("processing message failed with erro: {e}")
            }
        });

        Ok(Self {
            thread_handles: (send_thread, recv_thread),
            def_db,
            logs: Vec::default(),
            tabs: vec![Tab::Todo(0), Tab::Defs((0, 0, 0)), Tab::Logs].into(),
            tab: 0,
            todos: Todos::default(),
            in_body: true,
        })
    }

    pub fn cycle_left(&mut self) {
        if self.tab != 0 {
            self.tab -= 1;
        } else {
            self.tab = self.tabs.len() - 1;
        }
    }

    pub fn cycle_right(&mut self) {
        self.tab += 1;
        self.tab %= self.tabs.len() - 1;
    }

    pub fn up(&mut self) {
        // TODO: write
    }

    pub fn down(&mut self) {
        // TODO: write
    }
}

pub async fn talk_with_message_bus(
    tx: UnboundedSender<ProgBotMessage>,
    // lsp_tx: UnboundedSender<Definition>,
    mut reader: SplitStream<WebSocketStream<impl AsyncRead + AsyncWrite + Unpin>>,
) -> Result<()> {
    while let Some(websocket_message) = reader.next().await {
        match websocket_message {
            Ok(Message::Text(raw_message)) => {
                if let Ok(message) = serde_json::from_str::<ProgBotMessage>(&raw_message) {
                    let _ = tx.send(message);
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
    mut rx: UnboundedReceiver<ProgBotMessage>,
    mut writer: SplitSink<WebSocketStream<impl AsyncRead + AsyncWrite + Unpin>, Message>,
    defs: Arc<Mutex<DefenitionsDB>>,
    uuid: Uuid,
) -> Result<()> {
    writer
        .send(Message::Text(serde_json::to_string(&ProgBotMessage {
            msg_type: ProgBotMessageType::LspCommand,
            data: serde_json::to_value(&LspCommand::GetAllDefs)?,
            context: ProgBotMessageContext {
                sender: Some(uuid),
                response_to: None,
            },
        })?))
        .await?;

    while let Some(raw_message) = rx.recv().await {
        match raw_message.msg_type {
            ProgBotMessageType::LspDefinitionLocation => {
                let def: Definition = serde_json::from_value(raw_message.data)?;

                defs.lock().await.add_def(def);
            }
            ProgBotMessageType::LspResponce => {
                let def = serde_json::from_value::<LspResponse>(raw_message.data);

                if let Ok(res) = def {
                    match res {
                        LspResponse::FoundDef(def) => defs.lock().await.add_def(def),
                        LspResponse::AllDefs(db) => defs.lock().await.append(db),
                    }
                } else {
                    error!("failed to parse lsp response");
                }
            }
            ProgBotMessageType::FileClosed => {}
            ProgBotMessageType::FoundTodo => {}
            _ => {}
        }
    }

    Ok(())
}
