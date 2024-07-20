use prog_bot_data_types::{todo::Todo, ProgBotMessage, Uuid};
use std::collections::HashMap;
use tokio::{
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
};

pub struct AppState {
    /// a receiver to get messages from the message bus
    from_mb: UnboundedReceiver<ProgBotMessage>,
    /// a sender to comunicate with the the message send thread
    to_mb: UnboundedSender<ProgBotMessage>,
    /// the join handles to the threads comunicating with the message bus
    thread_handles: (JoinHandle<()>, JoinHandle<()>),
    /// a list of all known Todos (in the order they were found in)
    all_todos: Data,
}
