use crate::cache::get_cached_sheet_data;
use crate::{Data, Error};
use common::{calculate_status, QuestStatus};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use poise::serenity_prelude as serenity;
use serenity::collector::ComponentInteractionCollector;
use serenity::all::{
    ButtonStyle, CreateActionRow, CreateButton, CreateInteractionResponse,
    CreateInteractionResponseMessage, EditMessage,
};
use futures_util::stream::StreamExt;

type Context<'a> = poise::Context<'a, Data, Error>;

async fn paginate_embeds(ctx: Context<'_>, embeds: Vec<serenity::CreateEmbed>) -> Result<(), Error> {
    if embeds.is_empty() {
        return Ok(());
    }

    let buttons = vec![
        CreateButton::new("prev")
            .label("◀ Prev")
            .style(ButtonStyle::Secondary),
        CreateButton::new("next")
            .label("Next ▶")
            .style(ButtonStyle::Primary),
    ];
    let action_row = CreateActionRow::Buttons(buttons);

    let reply = poise::CreateReply::default()
        .embed(embeds[0].clone())
        .components(vec![action_row.clone()]);

    let handle = ctx.send(reply).await?;
    let message = handle.message().await?;

    let author_id = ctx.author().id;
    let mut page_idx: usize = 0;
    let msg_id = message.id;
    let channel_id = message.channel_id;

    let ctx_serenity = ctx.serenity_context();
    let mut collector = ComponentInteractionCollector::new(ctx_serenity)
        .message_id(msg_id)
        .channel_id(channel_id)
        .author_id(author_id)
        .timeout(std::time::Duration::from_secs(120))
        .stream();

    while let Some(interaction) = collector.next().await {
        let custom_id = &interaction.data.custom_id;

        // Acknowledge interaction with deferred update
        let response = CreateInteractionResponse::Defer(
            CreateInteractionResponseMessage::new()
        );
        let _ = interaction.create_response(&ctx_serenity.http, response).await;

        match custom_id.as_str() {
            "next" => page_idx = (page_idx + 1) % embeds.len(),
            "prev" => page_idx = if page_idx == 0 { embeds.len() - 1 } else { page_idx - 1 },
            _ => {}
        }

        let edit = EditMessage::new()
            .embed(embeds[page_idx].clone())
            .components(vec![action_row.clone()]);

        if let Err(e) = channel_id.edit_message(&ctx_serenity.http, msg_id, edit).await {
            eprintln!("Failed to edit paginated message: {:?}", e);
        }
    }

    let edit_final = EditMessage::new()
        .embed(embeds[page_idx].clone())
        .components(Vec::new());
    let _ = channel_id.edit_message(&ctx_serenity.http, msg_id, edit_final).await;

    Ok(())
}

#[poise::command(slash_command, description_localized("en-US", "List all available quests"), check = "crate::security::check_guild")]
pub async fn list(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    let now = Utc::now().timestamp();

    let result = get_cached_sheet_data(ctx).await;

    match result {
        Ok(data) => {

            let mut participant_counts: HashMap<String, i8> = HashMap::new();
            
            for row in data.p_rows.iter().skip(1) {
                if row.len() >= 4 {
                    let q_id = row[0].clone();
                    let status = row[3].clone().to_uppercase();
                    
                    if !q_id.is_empty() && (status == "ON_PROGRESS" || status == "COMPLETED" || status == "VERIFIED") {
                        *participant_counts.entry(q_id.to_string()).or_insert(0) += 1;
                    }
                }
            }

            let mut display_quests: Vec<(String, String, String, i64, i64, i8, i8)> = Vec::new();

            for row in data.q_rows.iter().skip(1) {
                if row.len() >= 9 {
                    let q_id = row[0].clone();
                    let title = row.get(1).map(|s| s.as_str()).unwrap_or("No Title").to_string();
                    let organizer = row.get(4).map(|s| s.as_str()).unwrap_or("-").to_string();
                    let schedule_str = row[5].clone();
                
                    let deadline_str = row[8].clone(); 
                    
                    let max_slots = row[3].parse::<i8>().unwrap_or(0);

                    if q_id == "Quest ID" || q_id.is_empty() { continue; }

                    let current_filled = *participant_counts.get(&q_id).unwrap_or(&0);

                    // Parse Start Time
                    let schedule_ts = if let Ok(dt) = DateTime::parse_from_rfc3339(&schedule_str) {
                        dt.timestamp()
                    } else {
                        0
                    };

                    // Parse Deadline/End Time
                    let deadline_ts = if let Ok(dt) = DateTime::parse_from_rfc3339(&deadline_str) {
                        dt.timestamp()
                    } else {
                        0 // 0 implies no deadline provided
                    };

                    display_quests.push((q_id, title, organizer, schedule_ts, deadline_ts, max_slots, current_filled));
                }
            }

            if display_quests.is_empty() {
                ctx.say("📭 There're no active quest at the moment.").await?;
                return Ok(());
            }
            
            let items_per_page = 5;
            let chunks: Vec<_> = display_quests.chunks(items_per_page).collect();

            let mut embeds: Vec<serenity::CreateEmbed> = Vec::new();

            for (i, chunk) in chunks.iter().enumerate() {
                let mut embed = serenity::CreateEmbed::new()
                    .title(format!("📜 Quest Board — Page {} of {}", i + 1, chunks.len()))
                    .color(0x3498DB);

                for (q_id, title, organizer, schedule_ts, deadline_ts, max_slots, filled) in *chunk {
                    
                    let status = calculate_status(now, schedule_ts, deadline_ts);
                    
                    let title_display = if status == QuestStatus::Ended {format!("~~{}~~ (Ended)", title)} else {title.clone()};

                    let (status_icon, time_msg, is_active) = match status {
                        QuestStatus::Ended => (
                            "🏁", 
                            format!("Ended <t:{}:R>", deadline_ts), 
                            false
                        ),
                        QuestStatus::Ongoing => (
                            "🏃", 
                            format!("**HAPPENING NOW!**\nEnds <t:{}:R>", deadline_ts), 
                            true
                        ),
                        QuestStatus::Upcoming => (
                            "🟢", 
                            format!("<t:{}:f> (<t:{}:R>)", schedule_ts, schedule_ts), 
                            true
                        ),
                        QuestStatus::Tba => ("⚪", "Date TBA".to_string(), true),
                    };
                    // --- SLOT LOGIC ---
                    // Only show slot status if the quest hasn't ended
                    let slot_str = if !is_active {
                        "❌ **Closed**".to_string()
                    } else if *filled >= *max_slots {
                        format!("🔴 **FULL** ({}/{})", filled, max_slots)
                    } else {
                        format!("{} **Open** ({}/{})", status_icon, filled, max_slots)
                    };

                    // Construct Field
                    let field_name = format!("{} — {}", title_display, q_id);
                    let field_value = format!(
                        "• Status: {}\n• By: {}\n• Time: {}\n", 
                        slot_str, organizer, time_msg
                    );

                    embed = embed.field(field_name, field_value, false);
                }

                embeds.push(embed);
            }

            paginate_embeds(ctx, embeds).await?;
        },
        Err(e) => {
            eprintln!("Sheet Error: {:?}", e);
            ctx.say("❌ Internal server error.").await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    // Import everything from the parent module (so we can see calculate_status)
    use super::*;

    #[test]
    fn test_quest_upcoming() {
        let now = 1000;
        let start = 2000; // Start is in the future
        let end = 3000;

        let result = calculate_status(now, &start, &end);
        assert_eq!(result, QuestStatus::Upcoming);
    }

    #[test]
    fn test_quest_ongoing() {
        let now = 2500;
        let start = 2000; // Start is in the past
        let end = 3000;   // End is in the future

        let result = calculate_status(now, &start, &end);
        assert_eq!(result, QuestStatus::Ongoing);
    }

    #[test]
    fn test_quest_ended() {
        let now = 4000;
        let start = 2000;
        let end = 3000; // End is in the past

        let result = calculate_status(now, &start, &end);
        assert_eq!(result, QuestStatus::Ended);
    }

    #[test]
    fn test_quest_tba() {
        let now = 1000;
        let start = 0; // No start time
        let end = 0;

        let result = calculate_status(now, &start, &end);
        assert_eq!(result, QuestStatus::Tba);
    }

    // Edge case: Exactly on the start second
    #[test]
    fn test_quest_starts_exactly_now() {
        let now = 2000;
        let start = 2000;
        let end = 3000;

        let result = calculate_status(now, &start, &end);
        assert_eq!(result, QuestStatus::Ongoing);
    }
}