use crate::{Data, Error};
use std::collections::HashMap;
use chrono::DateTime;

type Context<'a> = poise::Context<'a, Data, Error>;

#[poise::command(slash_command, description_localized("en-US", "**List** all available quests"), check = "crate::security::check_guild")]
pub async fn list(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    let hub = &ctx.data().sheets_hub;
    let sheet_id = &ctx.data().google_sheet_id;

    let result = hub.spreadsheets().values_batch_get(sheet_id)
        .add_ranges("Quests!A:I")
        .add_ranges("Participants!A:D")
        .doit()
        .await;

    match result {
        Ok((_, batch_res)) => {
            let value_ranges = batch_res.value_ranges.unwrap_or_default();
            if value_ranges.len() < 2 {
                ctx.say("❌ Failed to read data.").await?;
                return Ok(());
            }

            let q_rows = value_ranges[0].values.clone().unwrap_or_default();
            let p_rows = value_ranges[1].values.clone().unwrap_or_default();

            let mut participant_counts: HashMap<String, i8> = HashMap::new();
            
            for row in p_rows.iter().skip(1) {
                if row.len() >= 4 {
                    let q_id = row[0].as_str().unwrap_or("");
                    let status = row[3].as_str().unwrap_or("").to_uppercase();
                    
                    if !q_id.is_empty() && (status == "ON_PROGRESS" || status == "COMPLETED" || status == "VERIFIED") {
                        *participant_counts.entry(q_id.to_string()).or_insert(0) += 1;
                    }
                }
            }

            let mut display_quests: Vec<(String, String, String, i64, i8, i8)> = Vec::new();

            for row in q_rows.iter().skip(1) {
                if row.len() >= 9 {
                    let q_id = row[0].as_str().unwrap_or("").to_string();
                    let title = row[1].as_str().unwrap_or("No Title").to_string();
                    let organizer = row[4].as_str().unwrap_or("-").to_string();
                    let schedule_str = row[5].as_str().unwrap_or("");
                    let max_slots = row[3].as_str().unwrap_or("0").parse::<i8>().unwrap_or(0);

                    if q_id == "Quest ID" || q_id.is_empty() { continue; }

                    let current_filled = *participant_counts.get(&q_id).unwrap_or(&0);

                    let schedule_ts = if let Ok(dt) = DateTime::parse_from_rfc3339(schedule_str) {
                        dt.timestamp()
                    } else {
                        0
                    };

                    display_quests.push((q_id, title, organizer, schedule_ts, max_slots, current_filled));
                }
            }

            if display_quests.is_empty() {
                ctx.say("📭 There're no active quest at the moment.").await?;
                return Ok(());
            }
            
            let items_per_page = 5;
            let chunks: Vec<_> = display_quests.chunks(items_per_page).collect();
            let mut pages_string: Vec<String> = Vec::new();
            let total_pages = chunks.len();

            for (i, chunk) in chunks.iter().enumerate() {
                let mut page_content = format!("📜 **Quest Board** | Page {} of {}\n\n", i + 1, total_pages);
                
                for (q_id, title, organizer, schedule_ts, max_slots, filled) in *chunk {
                    // Indikator Slot Penuh
                    let slot_str = if *filled >= *max_slots {
                        format!("🔴 FULL ({}/{})", filled, max_slots)
                    } else {
                        format!("🟢 Available ({}/{})", filled, max_slots)
                    };

                    let time_str = if *schedule_ts > 0 {
                        format!("<t:{}:f> (<t:{}:R>)", schedule_ts, schedule_ts)
                    } else {
                        "Invalid time".to_string()
                    };

                    page_content.push_str(&format!(
                        "**{}**\n`{}`\n Slot: {} | By: {} | Start Time: {}\n\n", 
                        title, q_id, slot_str, organizer, time_str
                    ));
                }
                pages_string.push(page_content);
            }

            let pages_ref: Vec<&str> = pages_string.iter().map(|s| s.as_str()).collect();

            poise::builtins::paginate(ctx, pages_ref.as_slice()).await?;
        },
        Err(e) => {
            eprintln!("Sheet Error: {:?}", e);
            ctx.say("❌ Internal server error.").await?;
        }
    }

    Ok(())
}