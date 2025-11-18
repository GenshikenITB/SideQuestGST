mod models;
mod kafka;
mod commands {
    pub mod quest;
    pub mod admin;
}
mod security;

use poise::serenity_prelude as serenity;
use rdkafka::config::ClientConfig;
use rdkafka::producer::FutureProducer;
use std::env;

pub struct Data {
    pub kafka_producer: FutureProducer,
    pub target_guild_id: serenity::GuildId,
}

pub type Error = Box<dyn std::error::Error + Send + Sync>;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let token = env::var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN");
    let guild_id_str = env::var("TARGET_GUILD_ID").expect("missing TARGET_GUILD_ID");
    let guild_id = serenity::GuildId::new(guild_id_str.parse().expect("Invalid Guild ID"));
    let brokers = env::var("KAFKA_BROKERS").unwrap_or("kafka:9092".to_string());

    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &brokers)
        .set("message.timeout.ms", "5000")
        .create()
        .expect("Producer creation error");

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                commands::quest::create(), 
                commands::quest::take(),
                commands::admin::register_community(),
            ],
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_in_guild(ctx, &framework.options().commands, guild_id).await?;
                
                Ok(Data {
                    kafka_producer: producer,
                    target_guild_id: guild_id,
                })
            })
        })
        .build();

    let intents = serenity::GatewayIntents::non_privileged();
    let mut client = serenity::Client::builder(token, intents)
        .framework(framework)
        .await
        .expect("Error creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}