use std::sync::Arc;

use di_rs::{
    app_configs::{app_configs_subject, request::Action, ConfigItem, Request, Response},
    app_controls::{app_controls_subject, CommandType, EssAppStatus, EssControlRequest},
};
use futures::StreamExt;
use tokio::sync::Mutex;

const MRID: &str = "3bda2cb0-6e39-40ca-84de-d58b99e7e40e";
const REGION: &str = "290347ae-a0a6-4036-8d0c-0b45bd052376";

fn print_usage() {
    println!("Enter:");
    println!("  p:                  Print the status");
    println!("  soc_max:            Set SOC Max");
    println!("  soc_min:            Set SOC Min");
    println!("  get <key>:          Request config");
    println!("  set <key> <value>:  Set config");
    println!("  exit:               Quit");
}

async fn publish_control(
    nc: &async_nats::Client,
    commands: Vec<(CommandType, f32, String)>,
) -> Result<(), Box<dyn std::error::Error>> {
    let subject = app_controls_subject(REGION, "ess-manager");

    // request
    let request = EssControlRequest::create(commands);

    println!("Sending request: {:?}", request);

    let bytes: Vec<u8> = request.into();

    if let Err(e) = nc.publish(subject, bytes.into()).await {
        println!("Error: {}", e);
    }

    Ok(())
}

async fn publish_config(
    nc: &async_nats::Client,
    key: &str,
    value: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let subject = app_configs_subject(REGION, "ess-manager");

    let mut config = vec![];

    // add string value
    config.push(ConfigItem {
        key: format!("ess-manager.{}", key),
        value: value.to_string(),
    });

    let request = Request::from((config.clone(), Action::Set));

    println!("Sending request: {:?}", request);

    let bytes: Vec<u8> = request.into();

    if let Err(e) = nc.publish(subject, bytes.into()).await {
        println!("Error: {}", e);
    }

    Ok(())
}

async fn request_config(
    nc: &async_nats::Client,
    key: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let subject = app_configs_subject(REGION, "ess-manager");

    let mut config = vec![];

    // add string value
    config.push(ConfigItem {
        key: format!("ess-manager.{}", key),
        value: "".to_string(),
    });

    let request = Request::from((config.clone(), Action::Get));

    println!("Sending request: {:?}", request);

    let bytes: Vec<u8> = request.into();

    let response = nc.request(subject, bytes.into()).await?;

    // get response from payload
    let response = Response::try_from(response.payload.to_vec()).unwrap();

    // print
    println!("{:#?}", response);

    Ok(())
}

#[derive(Debug, Clone, Default)]
struct MyStatus {
    status: EssAppStatus,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let topic = format!("opendso.{}.EssAppStatus.app-status", REGION);

    let client = async_nats::connect("nats://127.0.0.1:4222").await?;
    let mut subscriber = client.subscribe(topic).await?;

    let status = Arc::new(Mutex::new(MyStatus::default()));

    let status_clone = status.clone();

    tokio::spawn(async move {
        while let Some(message) = subscriber.next().await {
            let message = message;
            let payload = message.payload.to_vec();

            let st: EssAppStatus = payload.try_into().unwrap();

            let mut status = status_clone.lock().await;
            status.status = st;
        }
    });

    loop {
        print_usage();
        // wait for input from keyboard
        let mut input = String::new();

        let mrid = MRID.to_string();

        if let Ok(_c) = std::io::stdin().read_line(&mut input) {
            if input.trim() == "p" {
                let status = status.lock().await;
                println!("Status: {:#?}", status.status);
            } else if input.trim() == "soc_max" {
                let mut value = String::new();
                println!("Enter SOC Max value:");
                if let Ok(_c) = std::io::stdin().read_line(&mut value) {
                    if let Ok(value) = value.trim().parse::<f32>() {
                        publish_control(
                            &client,
                            vec![(CommandType::SetMaxSoc, value, mrid.clone())],
                        )
                        .await?;
                    } else {
                        println!("Invalid value: {}", value);
                    }
                }
            } else if input.trim() == "soc_min" {
                let mut value = String::new();
                println!("Enter SOC Min value:");
                if let Ok(_c) = std::io::stdin().read_line(&mut value) {
                    if let Ok(value) = value.trim().parse::<f32>() {
                        publish_control(
                            &client,
                            vec![(CommandType::SetMinSoc, value, mrid.clone())],
                        )
                        .await?;
                    } else {
                        println!("Invalid value: {}", value);
                    }
                }
            } else if input.trim().starts_with("get ") {
                let key = input.split_whitespace().collect::<Vec<&str>>()[1];
                request_config(&client, key).await?;
            } else if input.trim().starts_with("set ") {
                let mut parts = input.split_whitespace();
                let key = parts.nth(1).unwrap();
                let value = parts.next().unwrap();
                publish_config(&client, key, value).await?;
            } else if input.trim() == "exit" {
                println!("Quitting...");
                break;
            } else {
                println!("Unknown command: {}", input.trim());
                print_usage();
            }

            input.clear();
        }
    }

    Ok(())
}
