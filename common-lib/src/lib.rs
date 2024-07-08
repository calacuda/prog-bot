use anyhow::{bail, Result};
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use prog_bot_data_types::{
    get_new_uuid, ProgBotMessage, ProgBotMessageContext, ProgBotMessageType, SubscribeTo, Uuid,
};
pub use tokio;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream};
use tracing::*;

// TODO: make logging macros that take a write connections to the message bus and a message to be
// logged.

/// returns user configs from config file
pub fn get_config_file() {}

/// returns the url for the message bus
fn get_message_bus_url() -> String {
    // TODO: call get_config_file
    // TODO: build url from that

    "ws://127.0.0.1:8080".into()
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
                sender: get_new_uuid(),
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
                            "got wrogn response type from server. get {:?}",
                            msg.msg_type
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
