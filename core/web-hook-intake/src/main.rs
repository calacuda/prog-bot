use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use actix::{clock::sleep, spawn};
use actix_web::{web, App, Error, HttpRequest, HttpServer};
use anyhow::Result;
use futures_util::SinkExt;
use prog_bot_common::{
    connect_to_messagebus, start_logging,
    tokio::sync::mpsc::{unbounded_channel, UnboundedSender},
};
use prog_bot_data_types::{
    Configuration, ProgBotMessage, ProgBotMessageContext, ProgBotMessageType,
};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite::Message;
use tracing::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GitHubWebHook {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GitLabWebHook {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum WebHookData {
    GitHub(GitHubWebHook),
    GitLab(GitLabWebHook),
}

async fn github(
    // data: web::Data<Addr<MessageEvent>>,
    write: web::Data<Mutex<UnboundedSender<WebHookData>>>,
    _req: HttpRequest,
    // stream: web::Payload,
) -> Result<String, Error> {
    info!("got github webhooks");

    let _ = write
        .lock()
        .unwrap()
        .send(WebHookData::GitHub(GitHubWebHook {}));

    Ok(String::new())
}

async fn gitlab(
    // data: web::Data<Addr<MessageEvent>>,
    write: web::Data<Mutex<UnboundedSender<WebHookData>>>,
    req: HttpRequest,
    // stream: web::Payload,
) -> Result<String, Error> {
    info!("got gitlab webhooks");
    println!("{req:?}");

    let _ = write
        .lock()
        .unwrap()
        .send(WebHookData::GitLab(GitLabWebHook {}));

    Ok(String::new())
}

fn on_ready() {
    debug!("webhook-intake node service started!");
}

fn on_stopping() {
    debug!("webhook-intake node is shutting down...");
}

pub async fn start(configs: Configuration) -> Result<()> {
    let (uuid, (mut writer, _reader)) = connect_to_messagebus(HashSet::new()).await?;
    let (write, mut read) = unbounded_channel::<WebHookData>();

    spawn(async move {
        while let Some(raw_hook_data) = read.recv().await {
            let data = match raw_hook_data {
                WebHookData::GitHub(_data) => {
                    // TODO: Parse GitHub data and return serde_json Value,
                    debug!("got github webhook data...");
                    serde_json::to_value(HashMap::<String, String>::new())
                }
                WebHookData::GitLab(_data) => {
                    // TODO: Parse GitLab data and return serde_json value
                    debug!("got gitlab webhook data...");
                    serde_json::to_value(HashMap::<String, String>::new())
                }
            };

            if let Ok(data) = data {
                let msg = ProgBotMessage {
                    msg_type: ProgBotMessageType::RecvWebHook,
                    data,
                    context: ProgBotMessageContext {
                        sender: Some(uuid),
                        response_to: None,
                    },
                };

                if let Ok(msg) = serde_json::to_string(&msg) {
                    let _ = writer.send(Message::Text(msg)).await;
                } else {
                    error!("failed to serialize ProgBotMessage to string")
                }
            } else {
                error!("serializing web-hook data failed");
            }
        }
    });

    // let msg_event_addr = web::Data::new(MessageEvent.start());
    let write = web::Data::new(write);
    let config = web::Data::new(Configuration::get());

    HttpServer::new(move || {
        App::new()
            // .app_data(msg_event_addr.clone())
            .app_data(write.clone())
            .app_data(config.clone())
            .route("/github", web::get().to(github))
            .route("/gitlab", web::get().to(gitlab))
        // .service(index)
    })
    .bind((configs.webhook.host, configs.webhook.port))?
    .run()
    .await?;

    Ok(())
}

#[actix::main]
async fn main() -> Result<()> {
    start_logging()?;

    debug!("Loading webhook-intake configs");

    let configs = Configuration::get();

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        on_stopping();
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    let r = running.clone();

    spawn(async move {
        if let Err(e) = start(configs).await {
            error!("webhook-intake failed to start: {e}");
            r.store(false, Ordering::SeqCst);
        }
    });

    on_ready();

    while running.load(Ordering::SeqCst) {
        sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}
