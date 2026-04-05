use dotenv::dotenv;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::{env, fs, time::Duration};
use teloxide::{Bot, prelude::Requester};
use tokio::sync::RwLock;
use tokio::sync::mpsc::{self, Sender};
use tonic::client;
use tonic::{Request, Response, Status, transport::Server};

// Scheduler, trait for .seconds(), .minutes(), etc., and trait with job scheduling methods
use clokwerk::{AsyncScheduler, Job, Scheduler, TimeUnits};
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

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Client {
    host: String,
    name: String,
    online: Option<bool>,
    ping_interval_seconds: u32,
}

#[derive(Debug, Clone)]
struct AppContext {
    services: Arc<RwLock<Vec<Service>>>,
    clients: Arc<RwLock<Vec<Client>>>,
}

impl AppContext {
    fn new(clients: Vec<Client>, services: Vec<Service>) -> AppContext {
        AppContext {
            services: Arc::new(RwLock::new(services)),
            clients: Arc::new(RwLock::new(clients)),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct App {
    services: Vec<Service>,
    clients: Vec<Client>,
    online: Option<bool>,
}

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
    let app: App = serde_yaml::from_str(&app_config_yaml)?;

    let app_context = AppContext::new(app.clients, app.services);

    let telegram_notifier = TelegramNotifier::new();

    let mut uptime_check_sheduler = AsyncScheduler::new();

    let services: Vec<Service> = app_context
        .services
        .read()
        .await
        .iter()
        .filter(|s| s.enabled)
        .cloned()
        .collect();

    //TODO use Arc<Mutex>> for services so they can be updated
    for service in services {
        let bot_sender_channel = telegram_notifier.telegram_sender_channel.clone();
        uptime_check_sheduler
            .every(service.interval.seconds())
            .run(move || {
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

    let mut ping_scheduler = AsyncScheduler::new();

    for client in app_context.clients.read().await.iter() {
        let client_clone = client.clone();
        let clients_clone = app_context.clients.clone();

        ping_scheduler
            .every(client_clone.ping_interval_seconds.second())
            .run(move || {
                let is_client_up: bool = ping_client(&client_clone);
                let client_clone_2 = client_clone.clone();
                let clients_clone_2 = clients_clone.clone();

                async move {
                    let mut binding = clients_clone_2.write().await;
                    let mut client_option =
                        binding.iter_mut().find(|c| c.host == client_clone_2.host);
                    match client_option {
                        Some(found_client) => {
                            found_client.online = Some(is_client_up);
                        }

                        None => {
                        }
                    }
                }
            });
    }

    loop {
        ping_scheduler.run_pending().await;
        uptime_check_sheduler.run_pending().await;
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn ping_client(client: &Client) -> bool {
    todo!();
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
