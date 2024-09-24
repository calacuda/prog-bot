use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, SampleRate, StreamConfig};
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
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    spawn,
    sync::mpsc::{unbounded_channel, UnboundedSender},
    time::sleep,
};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};
use tracing::*;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

struct SendStream(cpal::Stream);

unsafe impl Send for SendStream {}

pub async fn talk_with_message_bus(
    tx: UnboundedSender<()>,
    // lsp_tx: UnboundedSender<LspCommand>,
    mut reader: SplitStream<WebSocketStream<impl AsyncRead + AsyncWrite + Unpin>>,
) -> Result<()> {
    while let Some(websocket_message) = reader.next().await {
        match websocket_message {
            Ok(Message::Text(raw_message)) => {
                if let Ok(message) = serde_json::from_str::<ProgBotMessage>(&raw_message) {
                    match message.msg_type {
                        ProgBotMessageType::UserSpeakStart => {
                            tx.send(())?;
                        }
                        ProgBotMessageType::UserSpeakStop => {
                            tx.send(())?;
                        }
                        _ => {}
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
    mut rx: UnboundedReceiver<()>,
    mut writer: SplitSink<WebSocketStream<impl AsyncRead + AsyncWrite + Unpin>, Message>,
    uuid: Uuid,
) -> Result<()> {
    let path_to_model = "./ggml-base.en.bin";
    // let path_to_model = "./base.en.pt";

    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .expect("no input device available");

    // load a context and model
    let ctx = WhisperContext::new_with_params(path_to_model, WhisperContextParameters::default())
        .expect("failed to load model");

    let mut state = ctx.create_state().expect("failed to create key");

    let (tx, mut audio_rx) = unbounded_channel();

    let stream = SendStream(
        device
            .build_input_stream_raw(
                &StreamConfig {
                    channels: 1,
                    sample_rate: SampleRate(16_000),
                    buffer_size: cpal::BufferSize::Default,
                },
                SampleFormat::I16,
                move |data, _| {
                    if let Some(data) = data.as_slice() {
                        let mut audio = vec![0.0f32; data.len().try_into().unwrap()];
                        whisper_rs::convert_integer_to_float_audio(&data, &mut audio)
                            .expect("Conversion error");
                        tx.send(audio).expect("failed to send audio data ");
                    }
                },
                |e| {
                    error!("{e}");
                },
                Some(Duration::from_secs(1)),
            )
            .expect("device configs are jank"),
    );

    while rx.recv().await.is_some() {
        info!("user started speaking");
        stream.0.play().expect("failed to record form the mic.");
        // TODO: record voice clip,
        let mut audio = Vec::with_capacity(16_000 * 30);

        while rx.try_recv().is_err() {
            if let Some(mut a) = audio_rx.recv().await {
                audio.append(&mut a);
                // println!("{}", audio.len());
            }
        }
        stream.0.pause().expect("failed to record form the mic.");
        info!("user stoped speaking");

        // TODO: process_clip

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        // Edit params as needed.
        // Set the number of threads to use to 1.
        params.set_n_threads(4);
        // Enable translation.
        params.set_translate(true);
        // Set the language to translate to to English.
        params.set_language(Some("en"));
        // Disable anything that prints to stdout.
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        // Enable token level timestamps
        params.set_token_timestamps(false);

        state
            .full(params, &audio)
            .expect("whisper failed to handle spaech");

        let num_segments = state
            .full_n_segments()
            .expect("failed to get number of segments");
        // println!("{num_segments}");
        let mut utterance = String::new();

        for i in 0..num_segments {
            let segment = state
                .full_get_segment_text(i)
                .expect("failed to get segment");
            // let start_timestamp = state
            //     .full_get_segment_t0(i)
            //     .expect("failed to get segment start timestamp");
            // let end_timestamp = state
            //     .full_get_segment_t1(i)
            //     .expect("failed to get segment end timestamp");
            debug!("segment: {}", segment);
            utterance = format!("{utterance} {segment}");
        }

        info!("utterance: {utterance}");

        let message = ProgBotMessage {
            msg_type: ProgBotMessageType::UserUtterance,
            context: ProgBotMessageContext {
                sender: Some(uuid),
                response_to: None,
            },
            data: serde_json::to_value(&utterance)?,
        };

        writer
            .send(Message::Text(serde_json::to_string(&message)?))
            .await?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    start_logging()?;

    let mut sub_to = HashSet::new();
    sub_to.insert(ProgBotMessageType::UserSpeakStart);
    sub_to.insert(ProgBotMessageType::UserSpeakStop);
    let (uuid, (write, read)) = connect_to_messagebus(sub_to).await?;
    let (tx, rx) = unbounded_channel();

    info!("utterance detector connected to message-bus. assigned uuid: {uuid}");

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        info!("terminating utterance detector node");
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    let r = running.clone();
    spawn(async move {
        if let Err(e) = talk_with_message_bus(tx, read).await {
            error!("encountered an error while listening for user utterances: {e}");
            r.store(false, Ordering::SeqCst);
        }
    });

    // start process thread to process messages from the message bus
    let r = running.clone();

    spawn(async move {
        if let Err(e) = process_messages(rx, write, uuid).await {
            error!("utterance detection process encountered an error: {e}");
            r.store(false, Ordering::SeqCst);
        }
    });

    info!("utterance detector node started");

    while running.load(Ordering::SeqCst) {
        // debug!("waiting...");
        sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}
