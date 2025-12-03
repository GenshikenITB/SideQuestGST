use serde::{Deserialize, Serialize};
use redis::AsyncCommands;
use serde_json::from_str;
use crate::{Data, Error, models::GuildConfig};

type Context<'a> = poise::Context<'a, Data, Error>;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CachedQuestData {
    pub q_rows: Vec<Vec<String>>,
    pub p_rows: Vec<Vec<String>>,
    pub c_rows: Vec<Vec<String>>,
}

pub async fn get_cached_sheet_data(ctx: Context<'_>) -> Result<CachedQuestData, Error> {
    let redis_client = &ctx.data().redis_client;
    let mut con = redis_client.get_multiplexed_async_connection().await?;
    let cache_key = "sheet_data_cache";

    let cached_json: Option<String> = con.get(cache_key).await.ok();

    if let Some(json) = cached_json {
        if let Ok(data) = from_str::<CachedQuestData>(&json) {
            return Ok(data);
        }
    }

    let hub = &ctx.data().sheets_hub;
    let sheet_id = &ctx.data().google_sheet_id;

    let result = hub.spreadsheets().values_batch_get(sheet_id)
        .add_ranges("Quests!A:I")
        .add_ranges("Participants!A:D")
        .add_ranges("Communities!A:B")
        .doit()
        .await?;

    let value_ranges = result.1.value_ranges.unwrap_or_default();

    let extract_rows = |idx: usize| -> Vec<Vec<String>> {
        if let Some(v) = value_ranges.get(idx) {
            if let Some(rows) = &v.values {
                return rows.iter().map(|row| {
                    row.iter().map(|cell| cell.as_str().unwrap_or("").to_string()).collect()
                }).collect();
            }
        }
        vec![]
    };

    let data = CachedQuestData {
        q_rows: extract_rows(0),
        p_rows: extract_rows(1),
        c_rows: extract_rows(2),
    };

    let json_str = serde_json::to_string(&data)?;
    let _: () = con.set_ex(cache_key, json_str, 60).await?;

    Ok(data)
}

pub async fn get_guild_config(ctx: Context<'_>, guild_id: u64) -> Result<GuildConfig, Error> {
    let redis_client = &ctx.data().redis_client;
    let mut con = redis_client.get_multiplexed_async_connection().await?;
    let cache_key = format!("guild_config:{}", guild_id);
    let cached: Option<String> = con.get(&cache_key).await.ok();

    if let Some(json) = cached {
        if let Ok(config) = serde_json::from_str::<GuildConfig>(&json) {
            return Ok(config);
        }
    }
    Ok(GuildConfig::default())
}

pub async fn set_guild_config(ctx: Context<'_>, guild_id: u64, config: &GuildConfig) -> Result<(), Error> {
    let redis_client = &ctx.data().redis_client;
    let mut con = redis_client.get_multiplexed_async_connection().await?;
    let cache_key = format!("guild_config:{}", guild_id);
    let json = serde_json::to_string(config)?;
    let _: () = con.set(&cache_key, json).await?;
    Ok(())
}