use rdkafka::producer::{FutureRecord};
use rdkafka::util::Timeout;
use std::time::Duration;
use crate::models::EventMessage;

use crate::Data; 
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

pub async fn produce_event(ctx: Context<'_>, event_type: &str, payload: &impl serde::Serialize) -> Result<(), Error> {
    let payload_json = serde_json::to_string(payload)?;
    let event = EventMessage {
        event_type: event_type.to_string(),
        payload: payload_json,
    };
    let msg_json = serde_json::to_string(&event)?;

    let producer = &ctx.data().kafka_producer;
    let record = FutureRecord::to("quest.events")
        .payload(&msg_json)
        .key(event_type);

    producer.send(record, Timeout::After(Duration::from_secs(5))).await
        .map_err(|(e, _)| e)?;

    Ok(())
}