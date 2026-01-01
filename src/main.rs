use std::{env, fs};
use serde::{Serialize, Deserialize};
use teloxide::{Bot, prelude::Requester};
use dotenv::dotenv;
use tokio::sync::mpsc;


#[derive(Serialize, Deserialize , Debug, Clone)]
struct Service {
    host: String,
    service_type: ServiceType,
    name: String,
    enabled: bool,
}

#[derive(Serialize, Deserialize , Debug, Clone)]
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok(); 

    let bot = Bot::from_env();
    let chat_id = env::var("CHAT_ID").expect("CHAT_ID must be set").parse::<i64>().expect("CHAT_ID must be a valid i64");


    let app_config_yaml: String = fs::read_to_string("config/services.yml").expect("Failed to read config/app.yml");
    let app_config: AppConfig = serde_yaml::from_str(&app_config_yaml)?;




    // We'll spawn Tokio tasks instead of OS threads and use a Tokio channel for async message passing
    let mut ping_tasks = Vec::new();

    let (bot_sender_channel, mut bot_receiver_channel) = mpsc::channel::<ServiceStatus>(32);

    let receiver_handle = tokio::spawn(async move {
        while let Some(service_status) = bot_receiver_channel.recv().await {
            if let Err(e) = bot.send_message(
                teloxide::types::ChatId(chat_id),
                service_status.service.name.clone() + if service_status.is_up { " is UP" } else { " is DOWN" }
            ).await {
                eprintln!("Failed to send message: {:?}", e);
            }
        }
    });

    for service in app_config.services.into_iter().filter(|service| service.enabled) {
        let bot_sender_channel_clone = bot_sender_channel.clone();
        let ping_task = tokio::spawn(async move {
            match service.service_type {
                ServiceType::Http => {
                    // perform blocking HTTP request in a blocking task
                    handle_http_service(service, bot_sender_channel_clone).await;
                },
                _ => {
                    println!("Unsupported service type for service: {}", service.name);
                }
            };
        });
        ping_tasks.push(ping_task);

    }

    // wait for all ping tasks to finish and then the receiver
    for ping_task in ping_tasks {
        if let Err(e) = ping_task.await {
            eprintln!("Task panicked: {:?}", e);
        }
    }

    drop(bot_sender_channel); // Close the sender to signal the receiver to finish

    if let Err(e) = receiver_handle.await {
        eprintln!("Receiver task panicked: {:?}", e);
    }
   
    Ok(())
}

async fn handle_http_service(service: Service, bot_sender: mpsc::Sender<ServiceStatus>) {
    println!("Pinging HTTP service: {}", service.name);
    let name = service.name.clone();
    let host = service.host.clone();

    let res = reqwest::get(host.as_str()).await;


    match res {
        Ok(_resp) => {
            println!("{} is UP", name);
            let service_status = ServiceStatus { service: service.clone(), is_up: true };
            if bot_sender.send(service_status).await.is_err() {
                eprintln!("Receiver dropped when sending UP for {}", name);
            }
        },
        _ => {
            println!("{} is DOWN", name);
            let service_status = ServiceStatus { service: service.clone(), is_up: true };
            if bot_sender.send(service_status).await.is_err() {
                eprintln!("Receiver dropped when sending UP for {}", name);
            }
        }

        
    }

    
}