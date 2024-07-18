use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;

pub mod lsp;

pub type SubscribeTo = HashSet<ProgBotMessageType>;
pub type Uuid = uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ProgBotMessage {
    #[serde(rename = "type")]
    pub msg_type: ProgBotMessageType,
    #[serde(default = "empty_map")]
    pub data: Value,
    // #[serde(default = "empty_map")]
    pub context: ProgBotMessageContext,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Default, Copy)]
pub struct ProgBotMessageContext {
    /// uuid of sender
    pub sender: Option<Uuid>,
    // /// node type of sender
    // pub node_type: NodeType,
    pub response_to: Option<Uuid>,
}

pub fn empty_map() -> Value {
    json!({})
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Configuration {
    pub websocket: WebsocketConf,
    pub webhook: WebHookConf,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct WebsocketConf {
    pub host: String,
    pub port: u16,
    pub route: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct WebHookConf {
    pub host: String,
    pub port: u16,
    pub secret: String,
}

impl Configuration {
    pub fn get() -> Self {
        Self {
            websocket: WebsocketConf {
                host: "127.0.0.1".into(),
                port: 8080,
                route: "message-bus".into(),
            },
            webhook: WebHookConf {
                host: "0.0.0.0".into(),
                port: 8888,
                secret: "foobar".into(),
            },
        }
    }
}

// #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Copy)]
// pub enum NodeType {
//     TodoCollector,
//     GUI,
//
// }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Copy)]
pub enum ProgBotMessageType {
    /// indicates that the TTS engine said something and it should be added to the transcript
    Spoken,
    /// instructs the TTS engine to speak an utterance
    Speak,
    /// indicates that the mic is picking up audio from the user
    UserSpeakStart,
    /// indicates that the mic was, but is no longer picking up audio from the user
    UserSpeakStop,
    /// indicates that the user spoke something to the chat-bot
    UserUtterance,
    /// a webhook message was received
    RecvWebHook,
    /// a file was saved. which file is defined in the data section of the message
    FileSaved,
    /// a file was opened and should be processed by the lsp and and the TODO collector
    FileOpened,
    /// see: description for FileOpened.
    FileClosed,
    /// instructs the chat-bot to listen
    ChatBotListen,
    /// the chat-bot started listening
    ChatBotListeningStart,
    /// the chat-bot stopped listening
    ChatBotListeningStop,
    /// discovereed a new TODO
    FoundTodo,
    /// indicates that a TODO has been edited (which TODO and how it was edited is described by the
    /// data section)
    EditTodo,
    /// requests a new uuid for TODOs
    GetNewTodoUuid,
    /// the new uuid for TODOs
    NewTodoUuid,
    /// subscribe to a new message type
    Subscribe,
    /// unsubcribe from a subscribed message type
    Unsubscribe,
    LspStartDir,
    /// a message to the lsp server. data field contains command_type and additional arguments.
    LspCommand,
    /// alerts the location of a deffinition. the data feild is a map that contains three keys;
    /// defined_term, term_type, file, def_start (line_num, col_num), def_end (line_num, col_num).
    LspDefinitionLocation,
    /// sent to the message bus when a node first connects. it describes what messages it wishes to
    /// subscribe to.
    Syn,
    /// sent from the messages bus to a client to acknowledge that it has been registered and will
    /// receive messages.
    Ack,
    /// instructs the message bus to log something contains a log level and message
    Log,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Copy)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

pub fn get_new_uuid() -> Uuid {
    Uuid::new_v4()
}
