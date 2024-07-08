use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub type SubscribeTo = Vec<ProgBotMessageType>;
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ProgBotMessageContext {
    /// uuid of sender
    pub sender: Option<Uuid>,
    // /// node type of sender
    // pub node_type: NodeType,
    pub response_to: Option<Uuid>,
}

pub fn empty_map() -> Value {
    // HashMap::new()
    json!({})
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
