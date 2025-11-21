mod models;
mod kafka;
mod commands {
    pub mod quest;
    pub mod admin;
    pub mod stats;
    pub mod list;
    pub mod general;
}
mod security;
mod api;

use poise::serenity_prelude as serenity;
use rdkafka::config::ClientConfig;
use rdkafka::producer::FutureProducer;
use tokio::spawn;
use std::{env, net::{SocketAddr}};
use google_sheets4::{hyper, hyper_rustls, oauth2, Sheets};
use serenity::{GuildId, RoleId};

use crate::api::start_server;

pub type HubType = Sheets<hyper_rustls::HttpsConnector<hyper::client::HttpConnector>>;

pub struct Data {
    pub kafka_producer: FutureProducer,
    pub target_guild_id: GuildId,
    pub sheets_hub: HubType,
    pub google_sheet_id: String,
    pub qg_role_id: RoleId,
    pub participant_role_id: RoleId
}

pub type Error = Box<dyn std::error::Error + Send + Sync>;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let addr_str = env::var("API_ADDRESS").expect("missing API ADDRESS");
    let addr: SocketAddr = addr_str.parse().expect("API_ADDRESS invalid (use host:port)");
    let token = env::var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN");
    let guild_id_str = env::var("TARGET_GUILD_ID").expect("missing TARGET_GUILD_ID");
    let guild_id = GuildId::new(guild_id_str.parse().expect("Invalid Guild ID"));
    let quest_giver_str = env::var("QUEST_GIVER_ID").expect("missing QUEST_GIVER_ID");
    let quest_giver_id = serenity::RoleId::new(quest_giver_str.parse().expect("Invalid Quest Giver ID"));
    let quest_participant_str = env::var("QUEST_PARTICIPANT_ID").expect("missing QUEST_PARTICIPANT_ID");
    let quest_participant_id = serenity::RoleId::new(quest_participant_str.parse().expect("Invalid Quest Participant ID"));
    let brokers = env::var("KAFKA_BROKERS").unwrap_or("kafka:9092".to_string());

    let sa_key_path = env::var("GOOGLE_APPLICATION_CREDENTIALS").unwrap_or("/app/credentials.json".to_string());
    let sheet_id = env::var("GOOGLE_SHEET_ID").expect("Missing GOOGLE_SHEET_ID");

    let secret = oauth2::read_service_account_key(&sa_key_path)
        .await
        .expect("Failed to read credentials.json");
        
    let auth = oauth2::ServiceAccountAuthenticator::builder(secret)
        .build()
        .await
        .expect("Failed to create authenticator");

    let hub = Sheets::new(
        hyper::Client::builder().build(hyper_rustls::HttpsConnectorBuilder::new().with_native_roots().expect("REASON").https_or_http().enable_http1().build()),
        auth,
    );

    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &brokers)
        .set("message.timeout.ms", "5000")
        .create()
        .expect("Producer creation error");

    let producer_clone = producer.clone(); 

    spawn(async move {
        start_server(producer_clone, addr).await;
    });

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                commands::quest::create(),
                commands::quest::edit(),
                commands::quest::delete(), 
                commands::quest::take(),
                commands::quest::drop(),
                commands::quest::submit(),
                commands::stats::stats(),
                commands::list::list(),
                commands::admin::register_community(),
                commands::general::help(),
            ],
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_in_guild(ctx, &framework.options().commands, guild_id).await?;
                
                Ok(Data {
                    kafka_producer: producer,
                    target_guild_id: guild_id,
                    sheets_hub: hub,
                    google_sheet_id: sheet_id,
                    qg_role_id: quest_giver_id,
                    participant_role_id: quest_participant_id
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