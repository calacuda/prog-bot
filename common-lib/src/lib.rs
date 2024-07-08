use anyhow::{bail, Result};
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use prog_bot_data_types::{
    Configuration, ProgBotMessage, ProgBotMessageContext, ProgBotMessageType, SubscribeTo, Uuid,
};
pub use tokio;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream};
use tracing::*;

// TODO: make logging macros that take a write connections to the message bus and a message to be
// logged.

/// returns the url for the message bus
fn get_message_bus_url() -> String {
    // call get_config_file
    let conf = Configuration::get();

    // build url from that
    format!(
        "ws://{}:{}/{}",
        conf.websocket.host, conf.websocket.port, conf.websocket.route
    )
}

/// connects to the message bus and returns the Uuid returned from the message_bus server
pub async fn connect_to_messagebus(
    sub_to: SubscribeTo,
) -> Result<(
    Uuid,
    (
        SplitSink<WebSocketStream<impl AsyncRead + AsyncWrite + Unpin>, Message>,
        SplitStream<WebSocketStream<impl AsyncRead + AsyncWrite + Unpin>>,
    ),
)> {
    let url = get_message_bus_url();

    info!("connecting to message bus at - {url}");
    let (ws_stream, _) = connect_async(url).await?;
    info!("connected to message bus successfully");

    let (mut write, mut read) = ws_stream.split();

    info!("attempting to register node");
    write
        .send(Message::Text(serde_json::to_string(&ProgBotMessage {
            msg_type: ProgBotMessageType::Syn,
            data: serde_json::to_value(&sub_to)?,
            context: ProgBotMessageContext {
                sender: None,
                response_to: None,
            },
        })?))
        .await?;
    info!("sent reqestration request.");

    let uuid: Result<Uuid> = if let Some(message) = read.next().await {
        match message {
            Ok(msg) => match msg {
                Message::Text(raw_msg) => {
                    let msg: ProgBotMessage = serde_json::from_str(&raw_msg)?;

                    if msg.msg_type == ProgBotMessageType::Ack {
                        let uuid: Uuid = serde_json::from_value(msg.data)?;

                        Ok(uuid)
                    } else {
                        bail!(
                            "got wrong response type from server. got {:?}, expected {:?}",
                            msg.msg_type,
                            ProgBotMessageType::Ack,
                        )
                    }
                }
                _ => bail!("got unexceptable response message type."),
            },
            Err(e) => {
                error!("Error receiving message: {e}");
                bail!("{e}")
            }
        }
    } else {
        bail!("failed to read message_buss response")
    };
    info!("got reqestration response");

    Ok((uuid?, (write, read)))
}

pub fn start_logging() -> Result<()> {
    // construct a subscriber that prints formatted traces to stdout
    let subscriber = tracing_subscriber::fmt()
        .compact()
        .with_thread_ids(true)
        .with_target(true)
        .with_level(true)
        .with_max_level(Level::TRACE)
        .without_time()
        .finish();
    // use that subscriber to process traces emitted after this point
    tracing::subscriber::set_global_default(subscriber)?;

    Ok(())
}
