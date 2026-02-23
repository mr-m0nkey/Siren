use dotenv::dotenv;
use serde::{Deserialize, Serialize};
use std::{env, fs, time::Duration};
use teloxide::{Bot, prelude::Requester};
use tokio::sync::mpsc::{self, Sender};
use tonic::{transport::Server, Request, Response, Status};


// Scheduler, trait for .seconds(), .minutes(), etc., and trait with job scheduling methods
use clokwerk::{AsyncScheduler, Job, TimeUnits};
// Import week days and WeekDay
use clokwerk::Interval::*;

pub mod gossip {
    tonic::include_proto!("gossip");
}

//TODO find a way to refresh configs without restarts
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Service {
    host: String,
    service_type: ServiceType,
    name: String,
    enabled: bool,
    interval: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ServiceStatus {
    service: Service,
    is_up: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
enum ServiceType {
    Http,
    Tcp,
}

#[derive(Serialize, Deserialize, Debug)]
struct AppConfig {
    services: Vec<Service>,
}

//TODO add a way to implement gossip protocol
struct Gossip {}

struct TelegramNotifier {
    telegram_sender_channel: Sender<ServiceStatus>,
}

impl TelegramNotifier {
    pub fn new() -> TelegramNotifier {
        let (bot_sender_channel, mut bot_receiver_channel) = mpsc::channel::<ServiceStatus>(32);

        tokio::spawn(async move {
            let bot = Bot::from_env();
            let chat_id = env::var("CHAT_ID")
                .expect("CHAT_ID must be set")
                .parse::<i64>()
                .expect("CHAT_ID must be a valid i64");
            let receiver_handle: tokio::task::JoinHandle<()> = tokio::spawn(async move {
                while let Some(service_status) = bot_receiver_channel.recv().await {
                    if let Err(e) = bot
                        .send_message(
                            teloxide::types::ChatId(chat_id),
                            service_status.service.name.clone()
                                + if service_status.is_up {
                                    " is UP"
                                } else {
                                    " is DOWN"
                                },
                        )
                        .await
                    {
                        eprintln!("Failed to send message: {:?}", e);
                    }
                }
            });

            if let Err(e) = receiver_handle.await {
                eprintln!("Receiver task panicked: {:?}", e);
            }
        });

        TelegramNotifier {
            telegram_sender_channel: bot_sender_channel,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    tonic_build::compile_protos("proto/gossip.proto")?;

    let app_config_yaml: String =
        fs::read_to_string("config/services.yml").expect("Failed to read config/app.yml");
    let app_config: AppConfig = serde_yaml::from_str(&app_config_yaml)?;

    let telegram_notifier = TelegramNotifier::new();

    let mut scheduler = AsyncScheduler::new();

    for service in app_config
        .services
        .into_iter()
        .filter(|service| service.enabled)
    {
        let bot_sender_channel = telegram_notifier.telegram_sender_channel.clone();
        scheduler.every(service.interval.seconds()).run(move || {
            let bot_sender_channel = bot_sender_channel.clone();
            let service = service.clone();
            async move {
                match service.service_type {
                    ServiceType::Http => {
                        // perform blocking HTTP request in a blocking task
                        handle_http_service(service, bot_sender_channel).await;
                    }
                    _ => {
                        println!("Unsupported service type for service: {}", service.name);
                    }
                };
            }
        });
    }

    loop {
        scheduler.run_pending().await;
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn handle_http_service(service: Service, bot_sender: mpsc::Sender<ServiceStatus>) {
    println!("Pinging HTTP service: {}", service.name);
    let name = service.name.clone();
    let host = service.host.clone();

    let res = reqwest::get(host.as_str()).await;

    //TODO don't send notifications until there's a new state
    match res {
        Ok(_resp) => {
            println!("{} is UP", name);
            let service_status = ServiceStatus {
                service: service.clone(),
                is_up: true,
            };
            if bot_sender.send(service_status).await.is_err() {
                eprintln!("Receiver dropped when sending UP for {}", name);
            }
        }
        _ => {
            println!("{} is DOWN", name);
            let service_status = ServiceStatus {
                service: service.clone(),
                is_up: false,
            };
            if bot_sender.send(service_status).await.is_err() {
                eprintln!("Receiver dropped when sending UP for {}", name);
            }
        }
    }
}
