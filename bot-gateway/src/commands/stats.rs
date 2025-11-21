use crate::cache::get_cached_sheet_data;
use crate::{Data, Error};
use crate::models::StatsResult;
use poise::serenity_prelude as serenity;
use poise::CreateReply;
use std::collections::HashMap;

type Context<'a> = poise::Context<'a, Data, Error>;

pub fn calculate_stats(
    user_id: &str, 
    q_rows: &[Vec<String>], 
    p_rows: &[Vec<String>]
) -> StatsResult {
    let mut quest_map: HashMap<String, (String, String)> = HashMap::new();

    for row in q_rows {
        if row.len() >= 5 {
            let q_id = row[0].clone();
            let title = row[1].clone();
            let organizer = row[4].clone();
            quest_map.insert(q_id, (title, organizer));
        }
    }

    let mut active = 0;
    let mut completed = 0;
    let mut failed = 0;
    let mut list_str = String::new();

    for row in p_rows {
        if row.len() >= 4 {
            let row_user_id = &row[1];

            if row_user_id == user_id {
                let q_id = row[0].as_str();
                let mut status = row[3].to_uppercase();

                let (title, organizer) = quest_map.get(q_id)
                    .map(|(t, o)| (t.as_str(), o.as_str()))
                    .unwrap_or(("Unknown Quest", "-"));

                if status.contains("COMPLETED") || status.contains("VERIFIED") {
                    completed += 1;
                } else if status.contains("FAILED") {
                    failed += 1;
                } else if status.contains("ON_PROGRESS") {
                    status = "ON PROGRESS".to_string();
                    active += 1;
                    list_str.push_str(&format!(
                        "**{}**\n├ 🆔 ID: `{}`\n├ 🛡️ Organizer: {}\n└ 📌 Status: `{}`\n\n", 
                        title, q_id, organizer, status
                    ));
                }
            }
        }
    }

    StatsResult { active, completed, failed, list_str }
}

#[poise::command(slash_command, description_localized("en-US", "View your personal status"), check = "crate::security::check_guild")]
pub async fn stats(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let user_id = ctx.author().id.to_string();

    let result = get_cached_sheet_data(ctx).await;

    match result {
        Ok(data) => {
            let mut stats = calculate_stats(&user_id, &data.q_rows, &data.p_rows);

            let dm_channel = ctx.author().create_dm_channel(&ctx).await?;
            
            let embed = serenity::CreateEmbed::new()
                .title(format!("📊 User Stats: {}", ctx.author().name))
                .field("🔥 Active Quests", format!("{}", stats.active), true)
                .field("✅ Completed", format!("{}", stats.completed), true)
                .field("❌ Failed", format!("{}", stats.failed), true)
                .description(if stats.list_str.is_empty() { 
                    "No active quest at the moment.".to_string() 
                } else { 
                    if stats.list_str.len() > 2000 {
                        stats.list_str.truncate(1900);
                        stats.list_str.push_str("...(etc)");
                    }
                    format!("**Active quests list:**\n{}", stats.list_str) 
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_stats() {
        let user_id = "user123";
        
        let q_rows = vec![
            vec!["q1".into(), "Quest 1".into(), "Cat".into(), "5".into(), "Div A".into()],
            vec!["q2".into(), "Quest 2".into(), "Cat".into(), "5".into(), "Div B".into()],
        ];

        let p_rows = vec![
            vec!["q1".into(), "user123".into(), "tag".into(), "ON_PROGRESS".into()],
            vec!["q2".into(), "user123".into(), "tag".into(), "COMPLETED".into()],
            vec!["q1".into(), "other".into(), "tag".into(), "ON_PROGRESS".into()],
        ];

        let res = calculate_stats(user_id, &q_rows, &p_rows);

        assert_eq!(res.active, 1);
        assert_eq!(res.completed, 1);
        assert_eq!(res.failed, 0);
        assert!(res.list_str.contains("Quest 1"));
        assert!(!res.list_str.contains("Quest 2"));
    }
}