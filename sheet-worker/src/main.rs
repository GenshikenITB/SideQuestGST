mod models;
mod sheets;

use google_sheets4::{hyper, hyper_rustls, oauth2, Sheets};
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use rdkafka::message::Message;
use std::env;
use crate::models::EventMessage;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    let sa_key_path = env::var("GOOGLE_APPLICATION_CREDENTIALS").unwrap_or("/app/credentials.json".to_string());
    let sheet_id = env::var("GOOGLE_SHEET_ID").expect("Missing GOOGLE_SHEET_ID");
    let kafka_brokers = env::var("KAFKA_BROKERS").unwrap_or("kafka:9092".to_string());

    println!("Starting Sheet Worker...");
    println!("Auth Key Path: {}", sa_key_path);

    let secret = oauth2::read_service_account_key(&sa_key_path)
        .await
        .expect("Failed to read credentials.json");
        
    let auth = oauth2::ServiceAccountAuthenticator::builder(secret)
        .build()
        .await
        .expect("Failed to create authenticator");

    // Membuat Client Hub Google Sheets
    let hub = Sheets::new(
        hyper::Client::builder().build(hyper_rustls::HttpsConnectorBuilder::new().with_native_roots().expect("REASON").https_or_http().enable_http1().build()),
        auth,
    );

    let consumer: StreamConsumer = ClientConfig::new()
        .set("group.id", "sheet_worker_group")
        .set("bootstrap.servers", &kafka_brokers)
        .set("enable.partition.eof", "false")
        .set("session.timeout.ms", "6000")
        .set("enable.auto.commit", "true")
        .set("auto.offset.reset", "earliest") 
        .create()
        .expect("Consumer creation failed");

    consumer.subscribe(&["quest.events"]).expect("Subscription failed");

    println!("Worker Ready. Listening for events on 'quest.events'...");

    loop {
        match consumer.recv().await {
            Err(e) => eprintln!("Kafka error: {}", e),
            Ok(m) => {
                if let Some(payload_result) = m.payload_view::<str>() {
                    if let Ok(text) = payload_result {
                        if let Ok(event) = serde_json::from_str::<EventMessage>(text) {
                             sheets::process_event(&hub, &sheet_id, event).await;
                        } else {
                            eprintln!("Malformed JSON received");
                        }
                    }
                }
                if let Err(e) = consumer.commit_message(&m, CommitMode::Async) {
                    eprintln!("Commit failed: {}", e);
                }
            }
        };
    }
}