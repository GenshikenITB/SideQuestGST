use crate::{Data, Error};
use poise::serenity_prelude as serenity;
use poise::CreateReply;
use std::collections::HashMap;

type Context<'a> = poise::Context<'a, Data, Error>;

#[poise::command(slash_command, description_localized("en-US", "View your personal status"), check = "crate::security::check_guild")]
pub async fn stats(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let user_id = ctx.author().id.to_string();
    let hub = &ctx.data().sheets_hub;
    let sheet_id = &ctx.data().google_sheet_id;

    let result = hub.spreadsheets().values_batch_get(sheet_id)
        .add_ranges("Participants!A:D")
        .add_ranges("Quests!A:E")
        .doit()
        .await;

    match result {
        Ok((_, batch_response)) => {
            let value_ranges = batch_response.value_ranges.unwrap_or_default();
            
            if value_ranges.len() < 2 {
                ctx.say("❌ Failed to read sheets data.").await?;
                return Ok(());
            }

            let mut quest_map: HashMap<String, (String, String)> = HashMap::new();
            
            if let Some(q_rows) = &value_ranges[1].values {
                for row in q_rows {
                    if row.len() >= 5 {
                        let q_id = row[0].as_str().unwrap_or("").to_string();
                        let title = row[1].as_str().unwrap_or("Unknown Title").to_string();
                        let organizer = row[4].as_str().unwrap_or("Unknown").to_string();
                        quest_map.insert(q_id, (title, organizer));
                    }
                }
            }

            let mut active_count = 0;
            let mut completed_count = 0;
            let mut failed_count = 0;
            let mut quest_list_str = String::new();

            if let Some(p_rows) = &value_ranges[0].values {
                for row in p_rows.iter().skip(1) {
                    if row.len() >= 4 {
                        let row_user_id = row[1].as_str().unwrap_or("");

                        if row_user_id == user_id {
                            let q_id = row[0].as_str().unwrap_or("???");
                            let status = row[3].as_str().unwrap_or("UNKNOWN").to_uppercase();

                            let (title, organizer) = quest_map.get(q_id)
                                .map(|(t, o)| (t.as_str(), o.as_str()))
                                .unwrap_or(("Unknown Quest", "-"));

                            if status.contains("COMPLETED") || status.contains("VERIFIED") {
                                completed_count += 1;
                            } else if status.contains("FAILED") {
                                failed_count += 1;
                            } else if status.contains("ON_PROGRESS") {
                                active_count += 1;
                                quest_list_str.push_str(&format!(
                                    "**{}**\n└ 🆔 `{}` | 🛡️ {} | 📌 {}\n\n", 
                                    title, q_id, organizer, status
                                ));
                            }
                        }
                    }
                }
            }

            let dm_channel = ctx.author().create_dm_channel(&ctx).await?;
            
            let embed = serenity::CreateEmbed::new()
                .title(format!("📊 User Stats: {}", ctx.author().name))
                .field("🔥 Active Quests", format!("{}", active_count), true)
                .field("✅ Completed", format!("{}", completed_count), true)
                .field("❌ Failed", format!("{}", failed_count), true)
                .description(if quest_list_str.is_empty() { 
                    "No active quest at the moment.".to_string() 
                } else { 
                    if quest_list_str.len() > 2000 {
                        quest_list_str.truncate(1900);
                        quest_list_str.push_str("...(etc)");
                    }
                    format!("**Active quests list:**\n{}", quest_list_str) 
                })
                .color(0x3498DB);

            dm_channel.send_message(&ctx, serenity::CreateMessage::new().embed(embed)).await?;

            ctx.send(CreateReply::default()
                .content("✅ Stats has been send to your DM.")
                .ephemeral(true)
            ).await?;
        },
        Err(e) => {
            eprintln!("Error reading sheets: {:?}", e);
            ctx.say("❌ Failed fetching stats please contact admins.").await?;
        }
    }

    Ok(())
}