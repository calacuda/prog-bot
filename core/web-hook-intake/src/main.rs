use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use actix::{clock::sleep, spawn};
use actix_web::{error, web, App, Error, HttpRequest, HttpServer};
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use prog_bot_common::{
    connect_to_messagebus, start_logging,
    tokio::sync::mpsc::{unbounded_channel, UnboundedSender},
};
use prog_bot_data_types::{
    Configuration, ProgBotMessage, ProgBotMessageContext, ProgBotMessageType,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_tungstenite::tungstenite::Message;
use tracing::*;

const MAX_SIZE: usize = 262_144; // max payload size is 256k

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GitHubWebhook {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GitlabPipeline {
    pub project_name: String,
    pub duration: f64,
    pub status: String,
    pub failed_jobs: Vec<GitlabBuild>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GitlabBuild {
    id: usize,
    pub name: String,
    pub status: String,
    pub stage: String,
    pub duration: f64,
    created_at: String,
    started_at: String,
    finished_at: String,
    queued_duration: f64,
    failure_reason: Option<Value>,
    when: String,
    manual: bool,
    allow_failure: bool,
    user: Value,
    runner: Value,
    artifacts_file: Value,
    environment: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GitlabJob {
    pub name: String,
    pub status: String,
    pub stage: String,
    pub durration: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GitlabWebhook {
    Pipeline(GitlabPipeline),
    Job(GitlabJob),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum WebHookData {
    GitHub(GitHubWebhook),
    Gitlab(GitlabWebhook),
}

async fn github(
    write: web::Data<Mutex<UnboundedSender<WebHookData>>>,
    _req: HttpRequest,
    _payload: web::Payload,
) -> Result<String, Error> {
    info!("got github webhooks");

    let _ = write
        .lock()
        .unwrap()
        .send(WebHookData::GitHub(GitHubWebhook {}));

    Ok(String::new())
}

async fn gitlab(
    write: web::Data<Mutex<UnboundedSender<WebHookData>>>,
    req: HttpRequest,
    mut payload: web::Payload,
) -> Result<String, Error> {
    info!("got gitlab webhooks");
    // println!("req => {req:?}");
    // println!("payload => {payload:?}");
    // TODO: validate secret

    // payload is a stream of Bytes objects
    let mut body = web::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        // limit max size of in-memory payload
        if (body.len() + chunk.len()) > MAX_SIZE {
            return Err(error::ErrorBadRequest("overflow"));
        }
        body.extend_from_slice(&chunk);
    }

    let Ok(body) = serde_json::from_str::<Value>(&String::from_utf8_lossy(&body)) else {
        return Err(error::ErrorBadRequest("malformed json"));
    };

    let data = if body["object_kind"] == "pipeline" {
        GitlabWebhook::Pipeline(GitlabPipeline {
            project_name: serde_json::from_value(body["project"]["name"].clone()).unwrap(),
            status: serde_json::from_value(body["object_attributes"]["status"].clone()).unwrap(),
            failed_jobs: serde_json::from_value::<Vec<GitlabBuild>>(body["builds"].clone())
                .unwrap()
                .into_iter()
                .filter_map(|job| {
                    if job.status != "success" {
                        Some(job)
                    } else {
                        None
                    }
                })
                .collect(),
            duration: serde_json::from_value(body["object_attributes"]["duration"].clone())
                .unwrap(),
        })
    } else if body["object_kind"] == "build" {
        GitlabWebhook::Job(GitlabJob {
            name: serde_json::from_value(body["build_name"].clone()).unwrap(),
            status: serde_json::from_value(body["build_status"].clone()).unwrap(),
            stage: serde_json::from_value(body["build_stage"].clone()).unwrap(),
            durration: serde_json::from_value(body["build_duration"].clone()).unwrap(),
        })
    } else {
        return Err(error::ErrorBadRequest("wrong webhook type"));
    };

    let _ = write.lock().unwrap().send(WebHookData::Gitlab(data));

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
                WebHookData::GitHub(data) => {
                    debug!("got github webhook data...");

                    serde_json::to_value(data)
                }
                WebHookData::Gitlab(data) => {
                    debug!("got gitlab webhook data...");
                    println!("{data:?}");

                    serde_json::to_value(data)
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
    let write = web::Data::new(Mutex::new(write));
    let config = web::Data::new(Configuration::get());

    HttpServer::new(move || {
        App::new()
            // .app_data(msg_event_addr.clone())
            .app_data(write.clone())
            .app_data(config.clone())
            .route("github", web::post().to(github))
            .route("gitlab", web::post().to(gitlab))
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
