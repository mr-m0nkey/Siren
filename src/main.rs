use std::{fs, thread};
use serde::{Serialize, Deserialize};


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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Siren Check!");

    let app_config_yaml: String = fs::read_to_string("config/app.yml").expect("Failed to read config/app.yml");
    let app_config: AppConfig = serde_yaml::from_str(&app_config_yaml)?;


    //have a single thread watching the list of services and spawn a small health check thread (from a thread pool) to ping the service when it's scheduled time is reached.


    //maybe use a thread pool here instead of spawning a new thread for each service
    let mut ping_threads = Vec::new();


    for service in app_config.services.into_iter().filter(|service| service.enabled == true) {
        let ping_thread = thread::spawn(move || {
            match service.service_type {
                ServiceType::Http => {
                    handle_http_service(service);
                },
                _ => {
                    println!("Unsupported service type for service: {}", service.name);
                }
            };
             
        });
        ping_threads.push(ping_thread);
    }


    for ping_thread in ping_threads {
        ping_thread.join().expect("Thread panicked");
    }
   
    Ok(())
}

fn handle_http_service(service: Service)  {
    println!("Pinging HTTP service: {}", service.name);
    //use reqwest blocking client
    let response = reqwest::blocking::get(service.host.as_str());
    match response {
        Ok(_response) => {
            println!("{} is UP", service.name);
        },
        Err(_error) => {
            println!("{} is DOWN", service.name);
            
        }
    }
}

