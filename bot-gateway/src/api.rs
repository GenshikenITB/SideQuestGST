use axum::{
    routing::post,
    Router,
    Json,
    extract::State,
    http::StatusCode,
};
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::util::Timeout;
use std::net::SocketAddr;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
pub struct ApiState {
    pub producer: FutureProducer,
}

#[derive(Serialize, Deserialize)]
pub struct WebsiteQuestSubmission {
    pub title: String,
    pub description: String,
    // ini masi ditambahin.
}

// Handler for POST /submit
async fn submit_handler(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<WebsiteQuestSubmission>,
) -> Result<String, StatusCode> {
    
    // 1. Convert Web Payload to your Internal Event Payload
    // (You might need to map fields or create a specific event type for web submissions)
    let event_json = serde_json::to_string(&payload).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // 2. Send to Kafka
    let record = FutureRecord::to("quest.events")
        .payload(&event_json)
        .key("WEB_SUBMISSION"); // Use a specific key for web events

    match state.producer.send(record, Timeout::After(Duration::from_secs(5))).await {
        Ok(_) => Ok("Submission received".to_string()),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn start_server(producer: FutureProducer, addr: SocketAddr) {
    let shared_state = Arc::new(ApiState { producer });

    let app = Router::new()
        .route("/api/submit", post(submit_handler))
        .with_state(shared_state);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    println!("HTTP API listening on port {}", addr.port());
    axum::serve(listener, app).await.unwrap();
}