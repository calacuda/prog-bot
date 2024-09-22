use actix::{spawn, Actor, Addr, Context, Handler, Message, StreamHandler};
use actix_broker::{BrokerIssue, BrokerSubscribe, SystemBroker};
use actix_web::{
    error::ErrorBadRequest, post, web, App, Error, HttpRequest, HttpResponse, HttpServer,
};
use actix_web_actors::ws;
use anyhow::Result;
// use futures_util::stream::stream::StreamExt;
// use actix_web::{error, post, web, App, Error, HttpResponse};
use futures_util::StreamExt;
use prog_bot_common::start_logging;
use prog_bot_data_types::{
    get_new_uuid, Configuration, ProgBotMessage, ProgBotMessageContext, ProgBotMessageType,
    SubscribeTo, Uuid,
};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::time::{sleep, Duration};
use tracing::*;

#[cfg(test)]
mod test;

const MAX_SIZE: usize = 1_440; // max payload size is 256k

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MessageEvent;

#[derive(Clone, Debug, Message)]
#[rtype(result = "()")]
pub struct MessageInternalWrapper {
    id: Uuid,
    message: ProgBotMessage,
}

impl Actor for MessageEvent {
    type Context = Context<Self>;
}

impl Handler<MessageInternalWrapper> for MessageEvent {
    type Result = ();

    fn handle(&mut self, msg: MessageInternalWrapper, _ctx: &mut Self::Context) -> Self::Result {
        // log::trace!("SherlockMessageEvent recv message => {:?}", msg);
        self.issue_async::<SystemBroker, _>(msg);
    }
}

fn on_ready() {
    debug!("Message bus service started!")
}

fn on_stopping() {
    debug!("Message bus is shutting down...");
}

/// Define HTTP actor
#[derive(Clone)]
struct MessageBus {
    event: web::Data<Addr<MessageEvent>>,
    id: Uuid,
    subscribed_to: SubscribeTo,
}

impl Actor for MessageBus {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.subscribe_async::<SystemBroker, MessageInternalWrapper>(ctx);
    }
}

impl Handler<MessageInternalWrapper> for MessageBus {
    type Result = ();

    fn handle(&mut self, item: MessageInternalWrapper, ctx: &mut Self::Context) {
        if item.id != self.id && self.subscribed_to.contains(&item.message.msg_type) {
            debug!(
                "connection {} recv'ed a message {:?}",
                self.id, item.message
            );
            let mut mesg = item.message;

            mesg.context.sender = Some(item.id);

            if let Ok(json) = serde_json::to_string(&mesg) {
                ctx.text(json);
            } else {
                warn!("could not serialize message to json string.");
            }
        }
    }
}

/// Handler for ws::Message message
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for MessageBus {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(text)) => {
                // ctx.text(text.clone());
                if let Ok(message) = serde_json::from_str::<ProgBotMessage>(&text.to_string()) {
                    debug!("connection {} sent a message", self.id);

                    match message.msg_type {
                        ProgBotMessageType::Syn => {
                            if let Ok(sub_to) = serde_json::from_value::<SubscribeTo>(message.data)
                            {
                                self.subscribed_to.extend(sub_to);
                            }

                            let res_mesg = ProgBotMessage {
                                msg_type: ProgBotMessageType::Ack,
                                data: serde_json::to_value(self.id).unwrap(),
                                context: ProgBotMessageContext::default(),
                            };

                            if let Ok(res) = serde_json::to_string(&res_mesg) {
                                ctx.text(res);
                            } else {
                                error!("unknown error while serializing response to {}", self.id);
                            }
                        }
                        ProgBotMessageType::Subscribe => {
                            if let Ok(sub_to) = serde_json::from_value::<SubscribeTo>(message.data)
                            {
                                self.subscribed_to.extend(sub_to);
                            } else {
                                error!("malformed Json, can not subscribe to.");
                            }
                        }
                        ProgBotMessageType::Unsubscribe => {
                            if let Ok(sub_to) = serde_json::from_value::<SubscribeTo>(message.data)
                            {
                                self.subscribed_to.retain(|elm| !sub_to.contains(elm));
                            } else {
                                error!("malformed Json, can not subscribe to.");
                            }
                        }
                        ProgBotMessageType::Log => {
                            // TODO: log the sent message.
                        }
                        _ => self.event.do_send(MessageInternalWrapper {
                            id: self.id,
                            message,
                        }),
                    };
                } else {
                    warn!("received a message that doesn't follow the Message specifications. Did you serialize it using from a ProgBotMessage struct?");
                    ctx.text("{\"response\":\"malformed JSON message.\"}")
                }
            }
            Ok(ws::Message::Binary(_bin)) => {
                ctx.text("{\"response\":\"binary messages/responces are not yet implemented\"}")
            } // ctx.binary(bin),
            _ => (),
        }
    }
}

async fn index(
    data: web::Data<Addr<MessageEvent>>,
    req: HttpRequest,
    stream: web::Payload,
) -> Result<HttpResponse, Error> {
    let id = get_new_uuid();
    let resp = ws::start(
        MessageBus {
            event: data,
            id,
            subscribed_to: SubscribeTo::new(),
        },
        &req,
        stream,
    );

    resp
}

#[post("/open-file")]
async fn open_file(
    data: web::Data<Addr<MessageEvent>>,
    req: HttpRequest,
    mut payload: web::Payload,
    stream: web::Payload,
) -> Result<String, Error> {
    let mut body = web::BytesMut::new();

    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        // limit max size of in-memory payload
        if (body.len() + chunk.len()) > MAX_SIZE {
            return Err(ErrorBadRequest("overflow"));
        }
        body.extend_from_slice(&chunk);
    }

    let Ok(raw_body_data) = String::from_utf8(body.to_vec()) else {
        return Err(ErrorBadRequest("invalid utf8"));
    };
    let raw_body_data = raw_body_data.replace("\n", "");
    let id = get_new_uuid();

    if !raw_body_data.starts_with("/home/") {
        return Err(ErrorBadRequest("invalid directory"));
    }

    let tmp_file_path = PathBuf::from(&raw_body_data);

    let mut file_path = PathBuf::from("/home/");
    let mut prev_slash: bool = false;

    for dir in tmp_file_path.as_path() {
        // println!("dir: {dir:?} - {prev_slash}");
        if dir == "/" && prev_slash {
            return Err(ErrorBadRequest("file must be in the users home directory"));
        } else if dir == "/" {
            prev_slash = true;
        } else {
            prev_slash = false;
        }

        if dir == ".." {
            file_path.pop();
        } else {
            file_path.push(dir)
        }
        // println!("prev_slash {prev_slash}");
    }

    if !file_path.starts_with("/home/") {
        return Err(ErrorBadRequest("file must be in the users home directory"));
    }

    data.do_send(MessageInternalWrapper {
        id,
        message: ProgBotMessage {
            msg_type: ProgBotMessageType::FileOpened,
            data: serde_json::to_value(&file_path).unwrap(),
            context: ProgBotMessageContext {
                sender: None,
                response_to: None,
            },
        },
    });

    info!("opened file {}", file_path.as_path().to_str().unwrap());

    Ok("Success".into())
}

#[post("/close-file")]
async fn close_file(
    data: web::Data<Addr<MessageEvent>>,
    req: HttpRequest,
    mut payload: web::Payload,
    stream: web::Payload,
) -> Result<String, Error> {
    let mut body = web::BytesMut::new();

    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        // limit max size of in-memory payload
        if (body.len() + chunk.len()) > MAX_SIZE {
            return Err(ErrorBadRequest("overflow"));
        }
        body.extend_from_slice(&chunk);
    }

    let Ok(raw_body_data) = String::from_utf8(body.to_vec()) else {
        return Err(ErrorBadRequest("invalid utf8"));
    };
    let raw_body_data = raw_body_data.replace("\n", "");

    if !raw_body_data.starts_with("/home/") {
        return Err(ErrorBadRequest("invalid directory"));
    }

    let tmp_file_path = PathBuf::from(&raw_body_data);

    let mut file_path = PathBuf::from("/home/");
    let mut prev_slash: bool = false;

    for dir in tmp_file_path.as_path() {
        // println!("dir: {dir:?} - {prev_slash}");
        if dir == "/" && prev_slash {
            return Err(ErrorBadRequest("file must be in the users home directory"));
        } else if dir == "/" {
            prev_slash = true;
        } else {
            prev_slash = false;
        }

        if dir == ".." {
            file_path.pop();
        } else {
            file_path.push(dir)
        }
        // println!("prev_slash {prev_slash}");
    }

    if !file_path.starts_with("/home/") {
        return Err(ErrorBadRequest("file must be in the users home directory"));
    }

    let id = get_new_uuid();
    data.do_send(MessageInternalWrapper {
        id,
        message: ProgBotMessage {
            msg_type: ProgBotMessageType::FileClosed,
            data: serde_json::to_value(&file_path).unwrap(),
            context: ProgBotMessageContext {
                sender: None,
                response_to: None,
            },
        },
    });

    info!("closed file {}", file_path.as_path().to_str().unwrap());

    Ok("Success".into())
}

#[post("/save-file")]
async fn save_file(
    data: web::Data<Addr<MessageEvent>>,
    req: HttpRequest,
    mut payload: web::Payload,
    stream: web::Payload,
) -> Result<String, Error> {
    let mut body = web::BytesMut::new();

    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        // limit max size of in-memory payload
        if (body.len() + chunk.len()) > MAX_SIZE {
            return Err(ErrorBadRequest("overflow"));
        }
        body.extend_from_slice(&chunk);
    }

    let Ok(raw_body_data) = String::from_utf8(body.to_vec()) else {
        return Err(ErrorBadRequest("invalid utf8"));
    };
    let raw_body_data = raw_body_data.replace("\n", "");

    if !raw_body_data.starts_with("/home/") {
        return Err(ErrorBadRequest("invalid directory"));
    }

    let tmp_file_path = PathBuf::from(&raw_body_data);

    let mut file_path = PathBuf::from("/home/");
    let mut prev_slash: bool = false;

    for dir in tmp_file_path.as_path() {
        // println!("dir: {dir:?} - {prev_slash}");
        if dir == "/" && prev_slash {
            return Err(ErrorBadRequest("file must be in the users home directory"));
        } else if dir == "/" {
            prev_slash = true;
        } else {
            prev_slash = false;
        }

        if dir == ".." {
            file_path.pop();
        } else {
            file_path.push(dir)
        }
        // println!("prev_slash {prev_slash}");
    }

    if !file_path.starts_with("/home/") {
        return Err(ErrorBadRequest("file must be in the users home directory"));
    }

    let id = get_new_uuid();
    data.do_send(MessageInternalWrapper {
        id,
        message: ProgBotMessage {
            msg_type: ProgBotMessageType::FileSaved,
            data: serde_json::to_value(&file_path).unwrap(),
            context: ProgBotMessageContext {
                sender: None,
                response_to: None,
            },
        },
    });

    info!("saved file {}", file_path.as_path().to_str().unwrap());

    Ok("Success".into())
}

pub async fn start(configs: Configuration) -> Result<()> {
    let msg_event_addr = web::Data::new(MessageEvent.start());

    // TODO: add end points for file save and load

    HttpServer::new(move || {
        App::new()
            .app_data(msg_event_addr.clone())
            .route(&configs.websocket.route, web::get().to(index))
            .service(open_file)
            .service(close_file)
    })
    .bind((configs.websocket.host, configs.websocket.port))?
    .run()
    .await?;

    Ok(())
}

#[actix_web::main]
async fn main() {
    let _ = start_logging();

    debug!("Loading message bus configs");

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
            error!("message-bus failed to start: {e}");
            r.store(false, Ordering::SeqCst);
        }
    });

    on_ready();

    while running.load(Ordering::SeqCst) {
        // debug!("waiting...");
        sleep(Duration::from_secs(1)).await;
    }
}
