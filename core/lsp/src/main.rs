use anyhow::Result;
use async_lsp::{
    concurrency::ConcurrencyLayer,
    lsp_types::{
        notification::{Progress, PublishDiagnostics, ShowMessage},
        request::GotoDefinition,
        ClientCapabilities, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
        GotoDefinitionParams, InitializeParams, InitializedParams, NumberOrString,
        ProgressParamsValue, TextDocumentIdentifier, TextDocumentItem, TextDocumentPositionParams,
        Url, WindowClientCapabilities, WorkDoneProgress, WorkspaceFolder,
    },
    panic::CatchUnwindLayer,
    router::Router,
    tracing::TracingLayer,
    LanguageServer,
};
use futures_util::{
    stream::{SplitSink, SplitStream},
    StreamExt, TryFutureExt,
};
use prog_bot_common::{connect_to_messagebus, start_logging};
use prog_bot_data_types::{lsp::LspCommand, ProgBotMessage, ProgBotMessageType, Uuid};
use std::{
    collections::HashSet,
    ops::ControlFlow,
    path::PathBuf,
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::{
    fs::read_to_string,
    io::{AsyncRead, AsyncWrite},
    spawn,
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    time::{sleep, Duration},
};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};
use tower::ServiceBuilder;
use tracing::*;

struct ClientState {
    indexed_tx: Option<UnboundedSender<()>>,
}

struct Stop;

#[derive(Debug, Clone)]
pub enum LspMsg {
    FileOpen(PathBuf),
    FileClosed(PathBuf),
    Command(LspCommand),
}

#[tokio::main]
async fn main() -> Result<()> {
    start_logging()?;
    let mut sub_to = HashSet::new();
    sub_to.insert(ProgBotMessageType::LspCommand);
    sub_to.insert(ProgBotMessageType::FileOpened);
    sub_to.insert(ProgBotMessageType::FileClosed);

    let (uuid, (write, read)) = connect_to_messagebus(sub_to).await?;

    info!("TTS engine connected to message-bus. assigned uuid: {uuid}");

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

    info!("LSP node started");

    while running.load(Ordering::SeqCst) {
        // debug!("waiting...");
        sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}

pub async fn talk_with_message_bus(
    tx: UnboundedSender<LspMsg>,
    mut reader: SplitStream<WebSocketStream<impl AsyncRead + AsyncWrite + Unpin>>,
) -> Result<()> {
    while let Some(websocket_message) = reader.next().await {
        match websocket_message {
            Ok(Message::Text(raw_message)) => {
                if let Ok(message) = serde_json::from_str::<ProgBotMessage>(&raw_message) {
                    // if let Ok(utterance) = serde_json::from_value(message.data) {
                    //     tx.send(utterance)?;
                    // } else {
                    //     error!(
                    //         "unable to to parse the data field of a received message as a utterance"
                    //     );
                    // }
                    match message.msg_type {
                        ProgBotMessageType::LspCommand => {
                            if let Ok(cmd) = serde_json::from_value(message.data) {
                                tx.send(LspMsg::Command(cmd))?;
                            } else {
                                error!(
                                    "unable to to parse the data field of a received message as an LspCommand"
                                );
                            }
                        }
                        ProgBotMessageType::FileOpened => {
                            if let Ok(path) = serde_json::from_value(message.data) {
                                tx.send(LspMsg::FileOpen(path))?;
                            } else {
                                error!(
                                    "unable to to parse the data field of a received message as an path"
                                );
                            }
                        }
                        ProgBotMessageType::FileClosed => {
                            if let Ok(path) = serde_json::from_value(message.data) {
                                tx.send(LspMsg::FileClosed(path))?;
                            } else {
                                error!(
                                    "unable to to parse the data field of a received message as an path"
                                );
                            }
                        }
                        _ => unreachable!(
                            "not subscribed to a message of type: {:?}",
                            message.msg_type
                        ),
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
    mut rx: UnboundedReceiver<LspMsg>,
    mut writer: SplitSink<WebSocketStream<impl AsyncRead + AsyncWrite + Unpin>, Message>,
    uuid: Uuid,
) -> Result<()> {
    while let Some(msg) = rx.recv().await {
        match msg {
            LspMsg::Command(cmd) => {
                // exec lsp command
                match cmd {
                    LspCommand::FindDef(def) => {
                        let term = def.term;

                        // server
                        //     .definition(GotoDefinitionParams {
                        //         text_document_position_params: TextDocumentPositionParams {
                        //             text_document: TextDocumentIdentifier {},
                        //         },
                        //     })
                        //     .await
                        //     .unwrap();
                    }
                }
            }
            LspMsg::FileOpen(path) => {
                // Initialize.

                let init_ret = server
                    .initialize(InitializeParams {
                        workspace_folders: Some(vec![WorkspaceFolder {
                            uri: Url::from_file_path(&path.parent().unwrap().parent().unwrap())
                                .unwrap(),
                            name: "root".into(),
                        }]),
                        capabilities: ClientCapabilities {
                            window: Some(WindowClientCapabilities {
                                work_done_progress: Some(true),
                                ..WindowClientCapabilities::default()
                            }),
                            ..ClientCapabilities::default()
                        },
                        ..InitializeParams::default()
                    })
                    .await
                    .unwrap();
                info!("Initialized: {init_ret:?}");
                server.initialized(InitializedParams {}).unwrap();

                // Synchronize documents.
                let file_uri = Url::from_file_path(path.clone()).unwrap();
                server
                    .did_open(DidOpenTextDocumentParams {
                        text_document: TextDocumentItem {
                            uri: file_uri.clone(),
                            language_id: "rust".into(),
                            version: 0,
                            text: read_to_string(path).await.unwrap().into(),
                        },
                    })
                    .unwrap();

                // Wait until indexed.
                indexed_rx.recv().await.unwrap();
            }
            LspMsg::FileClosed(path) => {
                // unload file
                server
                    .did_close(DidCloseTextDocumentParams {
                        text_document: TextDocumentIdentifier {
                            uri: Url::from_file_path(path.clone()).unwrap(),
                        },
                    })
                    .unwrap();
            }
        }
    }

    mainloop_fut.await.unwrap();

    Ok(())
}
