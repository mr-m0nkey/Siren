use std::{env, fs};
use serde::{Serialize, Deserialize};
use teloxide::{Bot, prelude::Requester};
use dotenv::dotenv;
use tokio::sync::mpsc;
use tokio::task;


#[derive(Serialize, Deserialize , Debug, Clone)]
struct Service {
    host: String,
    service_type: ServiceType,
    name: String,
    enabled: bool,
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
    dotenv().ok(); // Load .env file


    let bot = Bot::from_env();
    let chat_id = env::var("CHAT_ID").expect("CHAT_ID must be set").parse::<i64>().expect("CHAT_ID must be a valid i64");


    //move to a new thread
    // bot.send_message(
    //     teloxide::types::ChatId(chat_id), 
    //     "Siren Bot Started!"
    // ).await?;


    let app_config_yaml: String = fs::read_to_string("config/app.yml").expect("Failed to read config/app.yml");
    let app_config: AppConfig = serde_yaml::from_str(&app_config_yaml)?;


    //have a single thread watching the list of services and spawn a small health check thread (from a thread pool) to ping the service when it's scheduled time is reached.


    // We'll spawn Tokio tasks instead of OS threads and use a Tokio channel for async message passing
    let mut ping_tasks = Vec::new();

    let (tx, mut rx) = mpsc::channel::<String>(32);

    let bot_clone_for_receiver = bot.clone();
    let chat_id_for_receiver = chat_id;
    let receiver_handle = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if let Err(e) = bot_clone_for_receiver.send_message(
                teloxide::types::ChatId(chat_id_for_receiver),
                message
            ).await {
                eprintln!("Failed to send message: {:?}", e);
            }
        }
    });

    for service in app_config.services.into_iter().filter(|service| service.enabled) {
        let tx_clone = tx.clone();
        let handle = tokio::spawn(async move {
            match service.service_type {
                ServiceType::Http => {
                    // perform blocking HTTP request in a blocking task
                    handle_http_service(service, tx_clone).await;
                },
                _ => {
                    println!("Unsupported service type for service: {}", service.name);
                }
            };
        });
        ping_tasks.push(handle);

    }

    // wait for all ping tasks to finish and then the receiver
    for ping_task in ping_tasks {
        if let Err(e) = ping_task.await {
            eprintln!("Task panicked: {:?}", e);
        }
    }

    drop(tx); // Close the sender to signal the receiver to finish

    if let Err(e) = receiver_handle.await {
        eprintln!("Receiver task panicked: {:?}", e);
    }
   
    Ok(())
}

async fn handle_http_service(service: Service, bot_sender: mpsc::Sender<String>) {
    println!("Pinging HTTP service: {}", service.name);
    let name = service.name.clone();
    let host = service.host.clone();

    // run the blocking reqwest call in a blocking thread
    let res = tokio::task::spawn_blocking(move || reqwest::blocking::get(host.as_str())).await;

    match res {
        Ok(Ok(_resp)) => {
            println!("{} is UP", name);
            if bot_sender.send(service.name + " is UP [" + service.host.as_str() + "]").await.is_err() {
                eprintln!("Receiver dropped when sending UP for {}", name);
            }
        },
        _ => {
            println!("{} is DOWN", name);
            if bot_sender.send(service.name + " is DOWN [" + service.host.as_str() + "]").await.is_err() {
                eprintln!("Receiver dropped when sending DOWN for {}", name);
            }
        }
    }
}

