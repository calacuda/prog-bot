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
    pub duration: Option<f64>,
    pub status: String,
    pub failed_jobs: Vec<GitlabBuild>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GitlabBuild {
    id: usize,
    pub name: String,
    pub status: String,
    pub stage: String,
    pub duration: Option<f64>,
    created_at: Option<String>,
    started_at: Option<String>,
    finished_at: Option<String>,
    queued_duration: Option<f64>,
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
    pub duration: Option<f64>,
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

async fn parse_gitlab(body: String) -> Result<GitlabWebhook, Error> {
    let Ok(body) = serde_json::from_str::<Value>(&body) else {
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
            duration: serde_json::from_value(body["build_duration"].clone()).unwrap(),
        })
    } else {
        return Err(error::ErrorBadRequest("wrong webhook type"));
    };

    Ok(data)
}

async fn gitlab(
    write: web::Data<Mutex<UnboundedSender<WebHookData>>>,
    secret: web::Data<Arc<str>>,
    req: HttpRequest,
    mut payload: web::Payload,
) -> Result<String, Error> {
    debug!("got gitlab webhooks request");
    // validate secret
    if let Some(header_secret) = req.headers().get("X-Gitlab-Token") {
        if let Ok(header_secret) = header_secret.to_str() {
            if secret.to_string() != header_secret {
                return Err(error::ErrorBadRequest("bad/incorrect secret"));
            } else {
                debug!("gitlab webhooks request secret is verified");
            }
        } else {
            return Err(error::ErrorBadRequest(
                "a secret is required & it must be a string",
            ));
        }
    } else {
        return Err(error::ErrorBadRequest("a secret is required"));
    }

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

    let body = String::from_utf8_lossy(&body);

    let data = parse_gitlab(body.into()).await?;

    let _ = write.lock().unwrap().send(WebHookData::Gitlab(data));

    Ok(String::new())
}

fn on_ready() {
    debug!("webhook-intake node service started!");
}

fn on_stopping() {
    debug!("webhook-intake node is shutting down...");
}

/// takes a number of seconds and makes a human speech like string.
pub fn get_time_str(duration: f64) -> String {
    let n_minutes = (duration as i64) / 60;
    let n_hours = n_minutes / 60;
    let n_minutes = n_minutes - (n_hours * 60);
    let n_seconds = (duration.round() as i64) - (n_hours * (60 * 60)) - (n_minutes * 60);

    let hours = format!(
        "{}{}",
        if n_hours > 0 {
            format!("{n_hours} hour")
        } else {
            String::new()
        },
        if n_hours > 1 {
            "s, "
        } else if n_hours == 1 {
            ", "
        } else {
            ""
        },
    );
    let minuets = format!(
        "{}{}",
        if n_minutes > 0 {
            format!("{n_minutes} minuet")
        } else {
            String::new()
        },
        if n_minutes > 1 {
            "s, "
        } else if n_minutes == 1 {
            ", "
        } else {
            ""
        }
    );
    let seconds = format!(
        "{n_seconds} second{}",
        if n_seconds > 1 || n_seconds == 0 {
            "s"
        } else {
            ""
        }
    );
    let conjunction = if n_hours != 0 || n_minutes != 0 {
        "and "
    } else {
        ""
    };

    format!("{hours}{minuets}{conjunction}{seconds}")
}

pub async fn start(configs: Configuration) -> Result<()> {
    let (uuid, (mut writer, _reader)) = connect_to_messagebus(HashSet::new()).await?;
    let (write, mut read) = unbounded_channel::<WebHookData>();

    spawn(async move {
        let context = ProgBotMessageContext {
            sender: Some(uuid),
            response_to: None,
        };

        while let Some(raw_hook_data) = read.recv().await {
            let (data, phrase) = match raw_hook_data {
                WebHookData::GitHub(data) => {
                    debug!("got github webhook data...");

                    (
                        serde_json::to_value(data),
                        serde_json::to_value(&"GitHub web hook message received"),
                    )
                }
                WebHookData::Gitlab(data) => {
                    debug!("got gitlab webhook data...");
                    println!("{data:?}");

                    (
                        serde_json::to_value(data.clone()),
                        serde_json::to_value(match data {
                            GitlabWebhook::Pipeline(GitlabPipeline {
                                project_name,
                                duration,
                                status,
                                failed_jobs,
                            }) => {
                                let failed_jobs_msgs: Vec<String> = failed_jobs
                                    .into_iter()
                                    .map(|build| {
                                        format!(
                                            "job, {} of stage {}: {}",
                                            build.name, build.stage, build.status
                                        )
                                    })
                                    .collect();
                                // if status.to_lowercase() != "pending" {
                                format!(
                                    "the most recent pipeline for the project {project_name}, {} status: {status}.{}{}",
                                    if status.to_lowercase() == "pending" {
                                        "has"
                                    } else {
                                        "completed with"
                                    },
                                    if let Some(duration) = duration {
                                        format!(" it took {}. ", get_time_str(duration))
                                    } else {
                                        String::new()
                                    },
                                    if !failed_jobs_msgs.is_empty() {
                                        format!("the following jobs failed: {}", failed_jobs_msgs.join(", "))
                                    } else {
                                        String::new()
                                    })
                                // } else {
                                //     String::new()
                                // }
                            }
                            GitlabWebhook::Job(GitlabJob {
                                name,
                                status,
                                stage,
                                duration,
                            }) => {
                                // if status.to_lowercase() != "pending" {
                                format!(
                                    "the job {name} of stage {stage}. {} status: {status}. {}",
                                    if status.to_lowercase() == "pending" {
                                        "has"
                                    } else {
                                        "completed with"
                                    },
                                    if let Some(duration) = duration {
                                        format!(" it took {}. ", get_time_str(duration))
                                    } else {
                                        String::new()
                                    }
                                )
                                // } else {
                                //     String::new()
                                // }
                            }
                        }),
                    )
                }
            };

            if let (Ok(data), Ok(phrase)) = (data, phrase) {
                let alert_msg = ProgBotMessage {
                    msg_type: ProgBotMessageType::RecvWebHook,
                    data,
                    context,
                };

                let speak_msg = ProgBotMessage {
                    msg_type: ProgBotMessageType::Speak,
                    data: phrase,
                    context,
                };

                if let (Ok(alert_msg), Ok(speak_msg)) = (
                    serde_json::to_string(&alert_msg),
                    serde_json::to_string(&speak_msg),
                ) {
                    let _ = writer.send(Message::Text(alert_msg)).await;
                    let _ = writer.send(Message::Text(speak_msg)).await;
                } else {
                    error!("failed to serialize ProgBotMessage to string (either message to alert message bus, or the message to speak)")
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
        let secret: Arc<str> = configs.webhook.secret.clone().into();

        App::new()
            // .app_data(msg_event_addr.clone())
            .app_data(web::Data::new(secret))
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

#[cfg(test)]
mod test {
    use crate::{get_time_str, parse_gitlab, GitlabJob, GitlabPipeline, GitlabWebhook};

    #[actix::test]
    async fn gitlab_parer_pipeline() {
        let test_input = "{\"object_kind\":\"pipeline\",\"object_attributes\":{\"id\":201,\"iid\":49,\"ref\":\"dev\",\"tag\":false,\"sha\":\"fdf87ee5a725b9204cebeadf833157c88e1aa039 \",\"before_sha\":\"28265263cd54dc7ba07220da5905c1a71aae343f\",\"source\":\"push\",\"status\":\"success\",\"detailed_status\":\"passed\",\"stages\":[\"prebuild\",\"test\"],\"created_at\":\"2024-07-12T00:20:23.187Z\",\"finished_at\":\"2024-07-12T00:23:06.660Z\",\"duration\":152,\"queued_duration\":10,\"variables\":[]},\"merge_request\":null,\"user\":{\"id\":2,\"name\":\"Eoghan West\",\"username\":\"calacuda\",\"avatar_url\":\"https://gitlab.home/uploads/-/system/user/avatar/2/avatar.png\",\"email\":\"[REDACTED]\"},\"project\":{\"id\":8,\"name\":\"prog-bot\",\"description\":null,\"web_url\":\"https://gitlab.home/calacuda/prog-bot\",\"avatar_url\":null,\"git_ssh_url\":\"git@gitlab.home:calacuda/prog-bot.git\",\"git_http_url\":\"https://gitlab.home/calacuda/prog-bot.git\",\"namespace\":\"Eoghan West\",\"visibility_level\":0,\"path_with_namespace\":\"calacuda/prog-bot\",\"default_branch\":\"main\",\"ci_config_path\":null},\"commit\":{\"id\":\"fdf87ee5a725b9204cebeadf833157c88e1aa039\",\"message\":\"updated web-hook-intake\\n\",\"title\":\"updated web-hook-intake\",\"timestamp\":\"2024-07-11T20:20:18-04:00\",\"url\":\"https://gitlab.home/calacuda/prog-bot/-/commit/fdf87ee5a725b9204cebeadf833157c88e1aa039\",\"author\":{\"name\":\"Eoghan West\",\"email\":\"eowest@gmail.com\"}},\"builds\":[{\"id\":465,\"stage\":\"prebuild\",\"name\":\"build-message-bus\",\"status\":\"success\",\"created_at\":\"2024-07-12T00:20:23.192Z\",\"started_at\":\"2024-07-12T00:20:33.490Z\",\"finished_at\":\"2024-07-12T00:21:52.877Z\",\"duration\":79.386694,\"queued_duration\":9.869351,\"failure_reason\":null,\"when\":\"on_success\",\"manual\":false,\"allow_failure\":false,\"user\":{\"id\":2,\"name\":\"Eoghan West\",\"username\":\"calacuda\",\"avatar_url\":\"https://gitlab.home/uploads/-/system/user/avatar/2/avatar.png\",\"email\":\"[REDACTED]\"},\"runner\":{\"id\":142,\"description\":\"runner\",\"runner_type\":\"instance_type\",\"active\":true,\"is_shared\":true,\"tags\":[]},\"artifacts_file\":{\"filename\":null,\"size\":null},\"environment\":null},{\"id\":466,\"stage\":\"test\",\"name\":\"message-bus\",\"status\":\"success\",\"created_at\":\"2024-07-12T00:20:23.200Z\",\"started_at\":\"2024-07-12T00:21:53.501Z\",\"finished_at\":\"2024-07-12T00:23:06.572Z\",\"duration\":73.071522,\"queued_duration\":0.499341,\"failure_reason\":null,\"when\":\"on_success\",\"manual\":false,\"allow_failure\":false,\"user\":{\"id\":2,\"name\":\"Eoghan West\",\"username\":\"calacuda\",\"avatar_url\":\"https://gitlab.home/uploads/-/system/user/avatar/2/avatar.png\",\"email\":\"[REDACTED]\"},\"runner\":{\"id\":142,\"description\":\"runner\",\"runner_type\":\"instance_type\",\"active\":true,\"is_shared\":true,\"tags\":[]},\"artifacts_file\":{\"filename\":null,\"size\":null},\"environment\":null}]}";

        let output = parse_gitlab(test_input.into()).await;

        assert!(output.is_ok(), "parsing of gilab pipeline failed");

        assert!(
            match output {
                Ok(GitlabWebhook::Pipeline(GitlabPipeline {
                    project_name,
                    duration,
                    status,
                    failed_jobs,
                })) => {
                    assert_eq!(project_name, "prog-bot", "parsed wrong project name");
                    assert_eq!(duration, Some(152.0), "parsed wrong duration");
                    assert_eq!(status, "success", "parsed wrong status");
                    assert!(
                        failed_jobs.is_empty(),
                        "found failed jobs, expected all jobs to succeed"
                    );
                    true
                }
                _ => false,
            },
            "gitlab parsed output was found to be of type Job (Gitlab calls them builds), instead of expected Pipeline."
        )
    }

    #[actix::test]
    async fn gitlab_parer_job() {
        let test_input = "{\"object_kind\":\"build\",\"ref\":\"dev\",\"tag\":false,\"before_sha\":\"28265263cd54dc7ba07220da5905c1a71aae343f\",\"sha\":\"fdf87ee5a725b9204cebeadf833157c88e1aa039\",\"build_id\":466,\"build_name\":\"message-bus\",\"build_stage\":\"test\",\"build_status\":\"success\",\"build_created_at\":\"2024-07-12T00:20:23.200Z\",\"build_started_at\":\"2024-07-12T00:21:53.501Z\",\"build_finished_at\":\"2024-07-12T00:23:06.572Z\",\"build_duration\":73.071522,\"build_queued_duration\":0.499341,\"build_allow_failure\":false,\"build_failure_reason\":\"unknown_failure\",\"pipeline_id\":201,\"runner\":{\"id\":142,\"description\":\"runner\",\"runner_type\":\"instance_type\",\"active\":true,\"is_shared\":true,\"tags\":[]},\"project_id\":8,\"project_name\":\"Eoghan West / prog-bot\",\"user\":{\"id\":2,\"name\":\"Eoghan West\",\"username\":\"calacuda\",\"avatar_url\":\"https://gitlab.home/uploads/-/system/user/avatar/2/avatar.png\",\"email\":\"[REDACTED]\"},\"commit\":{\"id\":201,\"name\":null,\"sha\":\"fdf87ee5a725b9204cebeadf833157c88e1aa039\",\"message\":\"updated web-hook-intake\\n\",\"author_name\":\"Eoghan West\",\"author_email\":\"eowest@gmail.com\",\"author_url\":\"https://gitlab.home/calacuda\",\"status\":\"success\",\"duration\":152,\"started_at\":\"2024-07-12T00:20:33.673Z\",\"finished_at\":\"2024-07-12T00:23:06.660Z\"},\"repository\":{\"name\":\"prog-bot\",\"url\":\"git@gitlab.home:calacuda/prog-bot.git\",\"description\":null,\"homepage\":\"https://gitlab.home/calacuda/prog-bot\",\"git_http_url\":\"https://gitlab.home/calacuda/prog-bot.git\",\"git_ssh_url\":\"git@gitlab.home:calacuda/prog-bot.git\",\"visibility_level\":0},\"environment\":null}";

        let output = parse_gitlab(test_input.into()).await;

        assert!(output.is_ok(), "parsing of dilab job failed");

        assert!(
            match output {
                Ok(GitlabWebhook::Job(GitlabJob { name, status, stage, duration })) => {
                    assert_eq!(name, "message-bus", "parsed wrong build name");
                    assert_eq!(duration, Some(73.071522), "parsed wrong duration");
                    assert_eq!(status, "success", "parsed wrong status");
                    assert_eq!(stage, "test", "parsed wrong stage");
                    true
                }
                _ => false,
            },
            "gitlab parsed output was found to be of type Pipeline, instead of expected type Job (Gitlab calls them builds)."
        )
    }

    #[test]
    fn get_time_str_test() {
        let t_1 = 73.071522;
        let t_1_str = get_time_str(t_1);

        assert_eq!(
            t_1_str, "1 minuet, and 13 seconds",
            "{t_1} unexpectedly converted to {t_1_str}"
        );

        let t_2 = 7_384.5;
        let t_2_str = get_time_str(t_2);

        assert_eq!(
            t_2_str, "2 hours, 3 minuets, and 5 seconds",
            "{t_2} unexpectedly converted to {t_2_str}"
        );

        let t_3 = 0.0;
        let t_3_str = get_time_str(t_3);

        assert_eq!(
            t_3_str, "0 seconds",
            "{t_3} unexpectedly converted to {t_3_str}"
        );

        let t_4 = 60.0;
        let t_4_str = get_time_str(t_4);

        assert_eq!(
            t_4_str, "1 minuet, and 0 seconds",
            "{t_4} unexpectedly converted to {t_4_str}"
        );

        let t_5 = 3_600.0;
        let t_5_str = get_time_str(t_5);

        assert_eq!(
            t_5_str, "1 hour, and 0 seconds",
            "{t_5} unexpectedly converted to {t_5_str}"
        );
    }

    // #[actix::test]
    // async fn message_bus() {
    //     // TODO: connect to message-bus, send data over unbound channel, wait for speak message on
    //     // message-bus and verify correctness
    // }
}
