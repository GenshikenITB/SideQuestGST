use crate::{Data, Error};
use poise::serenity_prelude as serenity;
use poise::CreateReply;

type Context<'a> = poise::Context<'a, Data, Error>;

#[poise::command(slash_command, check = "crate::security::check_guild")]
pub async fn stats(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let user_id = ctx.author().id.to_string();
    let hub = &ctx.data().sheets_hub;
    let sheet_id = &ctx.data().google_sheet_id;

    let result = hub.spreadsheets().values_get(sheet_id, "Participants!A:D")
        .doit()
        .await;

    match result {
        Ok((_, value_range)) => {
            let rows = value_range.values.unwrap_or_default();
            
            let mut active_count = 0;
            let mut completed_count = 0;
            let mut failed_count = 0;
            let mut quest_list = String::new();

            for row in rows {
                if row.len() >= 4 {
                    let row_user_id = row[1].as_str().unwrap_or("");
                    
                    if row_user_id == user_id {
                        let q_id = row[0].as_str().unwrap_or("???");
                        let status = row[3].as_str().unwrap_or("UNKNOWN").to_uppercase();

                        if status.contains("COMPLETED") || status.contains("VERIFIED") {
                            completed_count += 1;
                        } else if status.contains("FAILED") {
                            failed_count += 1;
                        } else {
                            active_count += 1;
                            quest_list.push_str(&format!("• `{}` (Status: {})\n", q_id, status));
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
                .description(if quest_list.is_empty() { 
                    "No active quest at the moment.".to_string() 
                } else { 
                    format!("**Active quests list:**\n{}", quest_list) 
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