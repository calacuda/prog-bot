use anyhow::{bail, Result};
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use prog_bot_common::{connect_to_messagebus, start_logging};
use prog_bot_data_types::{ProgBotMessage, ProgBotMessageContext, ProgBotMessageType, Uuid};
use rodio::{source::Source, Decoder, OutputStream};
use std::{
    collections::HashSet,
    fs::File,
    io::BufReader,
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

pub enum AudioOutput {
    Beep(Option<String>),
    Speak(String),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    start_logging()?;

    let mut sub_to = HashSet::new();
    sub_to.insert(ProgBotMessageType::Speak);
    sub_to.insert(ProgBotMessageType::Beep);
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
    tx: UnboundedSender<AudioOutput>,
    mut reader: SplitStream<WebSocketStream<impl AsyncRead + AsyncWrite + Unpin>>,
) -> Result<()> {
    while let Some(websocket_message) = reader.next().await {
        match websocket_message {
            Ok(Message::Text(raw_message)) => {
                if let Ok(message) = serde_json::from_str::<ProgBotMessage>(&raw_message) {
                    let Ok(msg_data) = serde_json::from_value(message.data) else {
                        error!(
                                    "unable to to parse the data field of a received message as a utterance"
                                );
                        continue;
                    };

                    let audio_cmd = match message.msg_type {
                        ProgBotMessageType::Speak => {
                            let Some(utterance) = msg_data else {
                                error!("TTS node was instructed to speak but no phrase was given.");
                                continue;
                            };

                            AudioOutput::Speak(utterance)
                        }
                        ProgBotMessageType::Beep => AudioOutput::Beep(msg_data),
                        _ => {
                            error!("TTS node recieved a message type, {:?}, that it was not programmed to handle.", message.msg_type);

                            continue;
                        }
                    };

                    tx.send(audio_cmd)?;
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
    mut rx: UnboundedReceiver<AudioOutput>,
    mut writer: SplitSink<WebSocketStream<impl AsyncRead + AsyncWrite + Unpin>, Message>,
    uuid: Uuid,
) -> Result<()> {
    // TODO: read config in fn main and pass default beep value here
    let default_beep: String = "beep.wav".into();

    while let Some(audio_cmd) = rx.recv().await {
        match audio_cmd {
            AudioOutput::Speak(utterance) => {
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
            AudioOutput::Beep(Some(beep_file)) => {
                beep(&beep_file);
            }
            AudioOutput::Beep(None) => {
                beep(&default_beep);
            }
        }
    }

    Ok(())
}

fn beep(beep_file: &str) {
    if let Err(e) = internal_beep(beep_file) {
        error!("{e}");
    }
}

fn internal_beep(beep_file: &str) -> Result<()> {
    // Get an output stream handle to the default physical sound device
    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    // Load a sound from a file
    let f_path = format!("/opt/prog-bot/sounds/{beep_file}");
    let Ok(audio_file) = File::open(&f_path) else {
        error!("beep file: {f_path}, could not be found");
        bail!("beep- file not found")
    };
    let file = BufReader::new(audio_file);
    // Decode that sound file into a source
    let Ok(source) = Decoder::new(file) else {
        error!("failed while decoding the beep file: {f_path}");
        bail!("could not decode beep_file")
    };
    // Play the sound directly on the device
    Ok(stream_handle.play_raw(source.convert_samples())?)
}
