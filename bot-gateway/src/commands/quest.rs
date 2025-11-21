use std::collections::HashMap;

use crate::{Data, Error, HubType};
use crate::models::{CachedQuestData, DeletePayload, Division, EditPayload, ProofPayload, QuestCategory, QuestCompleteMode, QuestPayload, RegistrationPayload
                    };
use crate::kafka::produce_event;
use common::{parse_wib, calculate_status, QuestStatus};
use futures_util::{stream, Stream};
use futures_util::StreamExt;
use poise::Modal as _;
use poise::CreateReply;
use redis::AsyncCommands;
use serde_json::from_str;
use serenity::all::{Attachment, AutocompleteChoice, CreateEmbed, CreateEmbedFooter};
use chrono::{DateTime, Utc};

type Context<'a> = poise::Context<'a, Data, Error>;

async fn get_quest_and_participant_data(ctx: Context<'_>, quest_id: &str) -> Result<(i8, i8, Option<String>, Option<String>, String), Error> {
    let data = get_cached_sheet_data(ctx).await?;

    let mut max_slots: i8 = 0;
    let mut schedule_iso: Option<String> = None;
    let mut deadline_iso: Option<String> = None;
    let mut quest_title = "Unknown Quest".to_string();
    let mut found = false;

    
    for row in data.q_rows.iter().skip(1) {
        if row.len() >= 9 && row[0].as_str().unwrap_or("") == quest_id {
            quest_title = row[1].as_str().unwrap_or("Unknown").to_string();
            max_slots = row[3].as_str().unwrap_or("0").parse::<i8>().unwrap_or(0);
            schedule_iso = Some(row[5].as_str().unwrap_or("").to_string());
            deadline_iso = Some(row[8].as_str().unwrap_or("").to_string());
            found = true;
            break;
        }
    }

    if !found {
        return Err(format!("Quest ID `{}` not found or slots not defined.", quest_id).into());
    }

    let mut current_participants: i8 = 0;
    for row in data.p_rows.iter().skip(1) {
        if row.len() >= 2 && row[0].as_str().unwrap_or("") == quest_id {
            current_participants += 1;
        }
    } 

    Ok((max_slots, current_participants, schedule_iso, deadline_iso, quest_title))
}

pub fn determine_organizer(category: QuestCategory, division: Division, community_name: Option<String>) -> Result<String, String> {
    match category {
        QuestCategory::CreativeArts => {
            if let Division::None = division {
                return Err("Error: Expected Division Name.".to_string());
            }
            Ok(format!("{:?}", division))
        },
        QuestCategory::Community => {
            match community_name {
                Some(name) if !name.trim().is_empty() => Ok(name),
                _ => Err("Error: Expected Community Name.".to_string()),
                }
            }
        }
}

async fn autocomplete_quest_id<'a>(
    ctx: Context<'_>,
    partial: &'a str,
) -> impl Stream<Item = AutocompleteChoice> + 'a {

    let now = Utc::now().timestamp();

    let mode = match ctx.command().name.as_str() {
        "take" => QuestCompleteMode::Take,
        _ => QuestCompleteMode::Submit,
    };

    let data = get_cached_sheet_data(ctx).await;

    let mut choices = Vec::new();

    match mode {
        QuestCompleteMode::Take => {
            let mut counts = HashMap::new();
            for row in data.p_rows.iter().skip(1) {
                if let Some(id) = row.get(0) {
                    *counts.entry(id.as_str().unwrap_or("")).or_insert(0) += 1;
                }
            }

            for row in data.q_rows.iter().skip(1) {
                if row.len() >= 9 {
                    let id = row[0].as_str().unwrap_or("");
                    let title = row[1].as_str().unwrap_or("No Title");
                    let slots = row[3].as_str().unwrap_or("0").parse::<i32>().unwrap_or(0);
                    let schedule_str = row[5].as_str().unwrap_or("");
                    let deadline_str = row[8].as_str().unwrap_or(""); 
                    let filled = *counts.get(id).unwrap_or(&0);

                    let start = if let Ok(dt) = DateTime::parse_from_rfc3339(schedule_str) {
                        dt.timestamp()
                    } else {
                        0
                    };

                    // Parse Deadline/End Time
                    let end = if let Ok(dt) = DateTime::parse_from_rfc3339(deadline_str) {
                        dt.timestamp()
                    } else {
                        0 // 0 implies no deadline provided
                    };

                    let status = calculate_status(now, &start, &end);

                    if status == QuestStatus::Upcoming && filled < slots {
                        let name = format!("{} ({} left) - {}", title, slots - filled, id);
                        let name = if name.len() > 100 {
                            name[0..100].to_string()
                        } else {
                            name
                        };

                        if name.to_lowercase().contains(&partial.to_lowercase()) {
                            choices.push(AutocompleteChoice::new(
                                name, id.to_string()
                            ));
                        }
                    }
                }
            }
        },
        QuestCompleteMode::Submit => {
            // Logic: Iterate Participants (User specific), lookup Quest Titles
            let user_id = ctx.author().id.to_string();
            
            let mut titles = HashMap::new();
            for row in data.q_rows.iter().skip(1) {
                if row.len() >= 2 {
                    titles.insert(
                        row[0].as_str().unwrap_or(""), 
                        row[1].as_str().unwrap_or("Unknown")
                    );
                }
            }

            for row in data.p_rows.iter().skip(1) {
                if row.len() >= 4 {
                    let q_id = row[0].as_str().unwrap_or("");
                    let u_id = row[1].as_str().unwrap_or("");
                    let status = row[3].as_str().unwrap_or("");

                    if u_id == user_id && status == "ON_PROGRESS" {
                        let title = titles.get(q_id).unwrap_or(&"Unknown");
                        
                        // What the user SEES
                        let name = format!("{} - {}", title, q_id);
                        
                        if name.to_lowercase().contains(&partial.to_lowercase()) {
                            choices.push(AutocompleteChoice::new(
                                name, q_id.to_string()
                            ));
                        }
                    }
                }
            }
        }
    }
    stream::iter(choices).take(25)
}

async fn get_cached_sheet_data(ctx: Context<'_>) -> Result<CachedQuestData, Error> {
    let redis_client = &ctx.data().redis_client;
    let mut con = redis_client.get_multiplexed_async_connection().await?;
    let cache_key = "sheet_data_cache";

    let cached_json: Option<String> = con.get(cache_key).await.ok();

    if let Some(json) = cached_json {
        if let Ok(data) = from_str<CachedQuestData>(&json) {
            return Ok(data);
        }
    }

    let hub = &ctx.data().sheets_hub;
    let sheet_id = &ctx.data().google_sheet_id;

    let result = hub.spreadsheets().values_batch_get(sheet_id)
        .add_ranges("Quests!A:I")
        .add_ranges("Participants!A:B")
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
    };

    let json_str = serde_json::to_string(&data)?;
    let _: () = con.set_ex(cache_key, json_str, 60).await?;

    Ok(data)
}

#[poise::command(slash_command, description_localized("en-US", "Create a new quest"), check = "crate::security::check_quest_role")] 
pub async fn create(
    ctx: Context<'_>,
    
    #[description = "Select Quest Category"]
    category: QuestCategory,

    #[description = "Select Division ('None' if Community)"]
    division: Division,

    #[description = "Community Name (Fill only if Community)"]
    community_name: Option<String>,

) -> Result<(), Error> {
    let organizer_final = match determine_organizer(category, division, community_name) {
        Ok(org) => org,
        Err(msg) => {
            ctx.send(CreateReply::default().content(format!("❌ {}", msg)).ephemeral(true)).await?;
            return Ok(());
        }
    };

    #[derive(Debug, poise::Modal)]
    #[name = "Side Quest Details"]
    struct QuestModal {
        #[name = "Quest Name"]
        #[placeholder = "Example: 5v5 MLBB Fun Match / KSICK"]
        title: String,
        
        #[name = "Description & Platform / Location"]
        #[paragraph]
        #[placeholder = "Row 1: [Platform/Location - Required\nRow 2+: [Quest Description]"]
        description_and_platform: String,

        #[name = "Participant Slots"]
        #[placeholder = "Example: 5"]
        slots: String,

        #[name = "Start Time (YYYY-MM-DD HH:MM)"]
        #[placeholder = "E.g: 2025-11-25 19:00"]
        #[min_length = 16] 
        #[max_length = 16]
        schedule: String,

        #[name = "Deadline (YYYY-MM-DD HH:MM)"]
        #[placeholder = "Empty if same as start time"]
        deadline: Option<String>,
    }

    let app_ctx = match ctx {
        poise::Context::Application(app_ctx) => app_ctx,
        _ => {
            ctx.say("❌ Error: This command must be run as slash command.").await?;
            return Ok(());
        }
    };

    let modal_data = QuestModal::execute(app_ctx).await?;
    
    if let Some(data) = modal_data {
        let schedule_iso = match parse_wib(&data.schedule) {
            Ok(iso) => iso,
            Err(err_msg) => {
                ctx.say(format!("❌ {}", err_msg)).await?;
                return Ok(());
            }
        };

        let deadline_iso = match data.deadline {
            Some(d) if !d.trim().is_empty() => {
                match parse_wib(&d) {
                    Ok(iso) => iso,
                    Err(err_msg) => {
                        ctx.say(format!("❌ Deadline Error: {}", err_msg)).await?;
                        return Ok(());
                    }
                }
            },
            _ => schedule_iso.clone(),
        };

        let (description, platform) = {
            let parts: Vec<&str> = data.description_and_platform.splitn(2, '\n').collect();
            let platform = parts[0].trim().to_string();
            let description = parts.get(1).map(|s| s.trim().to_string()).unwrap_or_default();
            (description, platform)
        };

        let quest_id = uuid::Uuid::new_v4().to_string();

        let payload = QuestPayload {
            quest_id: quest_id.clone(),
            title: data.title.clone(),
            description: description,
            slots: data.slots.clone().parse::<i8>().unwrap(),
            category: format!("{:?}", category),
            organizer_name: organizer_final,
            schedule: schedule_iso.clone(),
            platform: platform,
            deadline: deadline_iso.clone(),
            creator_id: ctx.author().id.to_string(),
        };
        let display_ts = DateTime::parse_from_rfc3339(&schedule_iso)
            .unwrap()
            .timestamp();

        let display_dl = DateTime::parse_from_rfc3339(&deadline_iso)
            .unwrap()
            .timestamp();

        ctx.send(CreateReply::default()
            .embed(CreateEmbed::default()
                .title(format!("⚔️ New Quest: {}", payload.title))
                 .description(&payload.description)
                 .field("📁 Category", &payload.category, true)
                 .field("🛡️ By", &payload.organizer_name, true)
                 .field("👥 Slots", &data.slots, true)
                 .field("📅 Start Time", format!("<t:{}:f>", display_ts), true)
                 .field("⏰ Deadline", format!("<t:{}:f>", display_dl), true)
                 .field("📍 Location", &payload.platform, true)
                 .field("ID", &quest_id, false)
                 .color(0xF1C40F)
                 .footer(CreateEmbedFooter::new("Use /take <id> to take the quest"))
            )
        ).await?;

        produce_event(ctx, "CREATE_QUEST", &payload).await?;
    }

    Ok(())
}

#[poise::command(slash_command, description_localized("en-US", "Edit an existing quest"), check = "crate::security::check_quest_role")] 
pub async fn edit(
    ctx: Context<'_>,
    
    #[description = "Quest ID to edit"]
    quest_id: String,
) -> Result<(), Error> {
    let data = get_cached_sheet_data(ctx).await;
    let mut existing_title = String::new();
    let mut existing_slots = String::new();
    let mut existing_platform = String::new();
    let mut existing_schedule = String::new();
    let mut existing_deadline = String::new();
    let mut existing_description = String::new();
    let mut found = false;

    for row in data.q_rows.iter().skip(1) {
        if row.len() >= 1 && row[0].as_str().unwrap_or("") == quest_id {
            existing_title = row.get(1).and_then(|v| v.as_str()).unwrap_or("").to_string();
            existing_slots = row.get(3).and_then(|v| v.as_str()).unwrap_or("").to_string();
            existing_platform = row.get(6).and_then(|v| v.as_str()).unwrap_or("").to_string();
            existing_schedule = row.get(5).and_then(|v| v.as_str()).unwrap_or("").to_string();
            existing_deadline = row.get(8).and_then(|v| v.as_str()).unwrap_or("").to_string();
            existing_description = row.get(7).and_then(|v| v.as_str()).unwrap_or("").to_string();
            found = true;
            break;
        }
    }

    if !found {
        ctx.send(CreateReply::default()
                .content(format!("❌ Quest ID `{}` not found.", quest_id))
                .ephemeral(true)).await?;
        return Ok(());
    }

    // Modal for editing (leave fields empty to keep existing)
    #[derive(Debug, poise::Modal)]
    #[name = "Edit Quest Details"]
    struct EditModal {
        #[name = "New Title (optional)"]
        title: Option<String>,

        #[name = "Description & Platform / Location (optional)"]
        #[paragraph]
        #[placeholder = "Line 1: Platform (optional)\nLine 2+: Description (optional)"]
        description_and_platform: Option<String>,

        #[name = "Participant Slots (optional)"]
        slots: Option<String>,

        #[name = "Start Time (YYYY-MM-DD HH:MM) (optional)"]
        schedule: Option<String>,

        #[name = "Deadline (YYYY-MM-DD HH:MM) (optional)"]
        deadline: Option<String>,
    }

    let app_ctx = match ctx {
        poise::Context::Application(a) => a,
        _ => {
            ctx.send(CreateReply::default()
                .content("❌ Error: This command must be run as slash command.")
                .ephemeral(true)).await?;
            return Ok(());
        }
    };

    let modal_data = EditModal::execute(app_ctx).await?;

    if let Some(data) = modal_data {
        let title_opt = data.title.and_then(|s| {
            let t = s.trim();
            if t.is_empty() { None } else { Some(t.to_string()) }
        });

        let (description_opt, platform_opt) = match data.description_and_platform {
            Some(s) => {
                let raw = s.trim();
                if raw.is_empty() {
                    (None, None)
                } else {
                    let parts: Vec<&str> = raw.splitn(2, '\n').collect();
                    let plat = parts.get(0).map(|p| p.trim()).unwrap_or("").to_string();
                    let desc = parts.get(1).map(|d| d.trim()).unwrap_or("").to_string();
                    let plat_opt = if plat.is_empty() { None } else { Some(plat) };
                    let desc_opt = if desc.is_empty() { None } else { Some(desc) };
                    (desc_opt, plat_opt)
                }
            }
            None => (None, None),
        };

        let slots_opt = match data.slots {
            Some(s) => {
                let t = s.trim();
                if t.is_empty() {
                    None
                } else {
                    match t.parse::<i8>() {
                        Ok(v) => Some(v),
                        Err(_) => {
                            ctx.send(CreateReply::default()
                                .content("❌ Invalid slots number.")
                                .ephemeral(true)).await?;
                            return Ok(());
                        }
                    }
                }
            }
            None => None,
        };

        let schedule_opt = match data.schedule {
            Some(s) => {
                let t = s.trim();
                if t.is_empty() {
                    None
                } else {
                    match parse_wib(t) {
                        Ok(iso) => Some(iso),
                        Err(msg) => {
                            ctx.send(CreateReply::default()
                                .content(format!("❌ Schedule Error: {}", msg))
                                .ephemeral(true)).await?;
                            return Ok(());
                        }
                    }
                }
            }
            None => None,
        };

        let deadline_opt = match data.deadline {
            Some(d) => {
                let t = d.trim();
                if t.is_empty() {
                    None
                } else {
                    match parse_wib(t) {
                        Ok(iso) => Some(iso),
                        Err(msg) => {
                            ctx.send(CreateReply::default()
                                .content(format!("❌ Deadline Error: {}", msg))
                                .ephemeral(true)).await?;
                            return Ok(());
                        }
                    }
                }
            }
            None => None,
        };

        let final_title = title_opt.unwrap_or_else(|| existing_title.clone());
        let final_platform = platform_opt.unwrap_or_else(|| existing_platform.clone());
        let final_description = description_opt.unwrap_or_else(|| existing_description.clone());
        let final_slots = slots_opt.unwrap_or_else(|| existing_slots.parse::<i8>().unwrap_or(0));
        let final_schedule = schedule_opt.unwrap_or_else(|| existing_schedule.clone());
        let final_deadline = deadline_opt.unwrap_or_else(|| {
            if existing_deadline.is_empty() { final_schedule.clone() } else { existing_deadline.clone() }
        });

        let edit_payload = EditPayload {
            quest_id: quest_id.clone(),
            title: final_title.clone(),
            description: final_description.clone(),
            slots: final_slots,
            schedule: final_schedule.clone(),
            deadline: final_deadline.clone(),
            platform: final_platform.clone(),
        };

        produce_event(ctx, "EDIT_QUEST", &edit_payload).await?;

        let display_ts = DateTime::parse_from_rfc3339(&final_schedule)
            .map(|dt| dt.timestamp())
            .unwrap_or(0);

        let display_dl = DateTime::parse_from_rfc3339(&final_deadline)
            .map(|dt| dt.timestamp())
            .unwrap_or(0);

        ctx.send(CreateReply::default()
            .embed(CreateEmbed::default()
                .title(format!("✏️ Quest Edited: {}", final_title))
                .description(&edit_payload.description)
                .field("👥 Slots", format!("{}", final_slots), true)
                .field("📅 Start Time", format!("<t:{}:f>", display_ts), true)
                .field("⏰ Deadline", format!("<t:{}:f>", display_dl), true)
                .field("📍 Location", &edit_payload.platform, true)
                .field("ID", &quest_id, false)
                .color(0x3498DB)
                .footer(CreateEmbedFooter::new("Use /take <id> to take the quest"))
            )
        ).await?;
    }

    Ok(())
}

#[poise::command(slash_command, description_localized("en-US", "Delete a quest"), check = "crate::security::check_quest_role")]
pub async fn delete(
    ctx: Context<'_>,
    #[description = "Quest ID to delete"] quest_id: String,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;
    
    let data = get_cached_sheet_data(ctx).await;

    let mut found = false;
        for row in data.q_rows.iter().skip(1) {
            if row.len() >= 1 && row[0].as_str().unwrap_or("") == quest_id {
                found = true;
                break;
            }
        }

    if !found {
        ctx.say(format!("❌ Quest ID `{}` not found.", quest_id)).await?;
        return Ok(());
    }
    

    let payload = DeletePayload {
        quest_id: quest_id.clone(),
    };

    if let Err(e) = produce_event(ctx, "DELETE_QUEST", &payload).await {
        ctx.say(format!("❌ Failed to send delete request: {}", e)).await?;
        return Ok(());
    }

    ctx.say(format!("✅ Delete request for quest `{}` sent.", quest_id)).await?;
    Ok(())
}

#[poise::command(slash_command, description_localized("en-US", "Take a quest from available quests"),
                 check = "crate::security::check_guild", check = "crate::security::check_participant_role")]
pub async fn take(
    ctx: Context<'_>,
    #[description = "Select a Quest"]
    #[autocomplete = "autocomplete_quest_id"]
    quest_id: String
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let now = chrono::Utc::now().timestamp();
    
    let hub = &ctx.data().sheets_hub;
    let sheet_id = &ctx.data().google_sheet_id;
    let user_id = ctx.author().id.to_string();

    match get_quest_and_participant_data(ctx, &quest_id).await {
        Ok((max_slots, current_participants, schedule_iso, deadline_iso, quest_title)) => {
            let schedule_str = schedule_iso.unwrap_or_default();
            let deadline_str = deadline_iso.unwrap_or_default();
            
            let quest_schedule = if !schedule_str.is_empty() {
                chrono::DateTime::parse_from_rfc3339(&schedule_str).map(|dt| dt.timestamp()).unwrap_or(0)
            } else { 0 };

            let quest_deadline = if !deadline_str.is_empty() {
                chrono::DateTime::parse_from_rfc3339(&deadline_str).map(|dt| dt.timestamp()).unwrap_or(0)
            } else { 0 };

            let status = calculate_status(now, &quest_schedule, &quest_deadline);

            if status == QuestStatus::Ended {
                ctx.say("❌ This quest has already ended (deadline passed).").await?;
                return Ok(());
            }

            if quest_schedule > 0 && now >= quest_schedule {
                ctx.say("❌ This quest has already started and cannot be taken.").await?;
                return Ok(());
            }

            let participants_res = hub.spreadsheets().values_get(sheet_id, "Participants!A:B").doit().await;
            if let Ok((_, part_range)) = participants_res {
                if let Some(rows) = part_range.values {
                    for row in rows {
                        if row.len() >= 2 && row[0].as_str().unwrap_or("") == quest_id && row[1].as_str().unwrap_or("") == user_id {
                            ctx.say("❌ You've taken this quest.").await?;
                            return Ok(());
                        }
                    }
                }
            }

            if current_participants >= max_slots {
                ctx.say(format!("❌ Quest `{}` is full. Available slots: {} of {}.", quest_title, max_slots - current_participants, max_slots)).await?;
                return Ok(());
            }

            let payload = RegistrationPayload {
                quest_id: quest_id.clone(),
                user_id: user_id.clone(),
                user_tag: ctx.author().tag(),
            };
            produce_event(ctx, "TAKE_QUEST", &payload).await?;
            ctx.say(format!("✅ Successfully taken the quest `{}`. Available slots: {} of {}.", quest_title, current_participants + 1, max_slots)).await?;
        },
        Err(e) => {
            ctx.say(format!("❌ Failed to take quest: {}", e)).await?;
        }
    }
    Ok(())
}

#[poise::command(slash_command, description_localized("en-US", "Drop a taken quest"),
                 check = "crate::security::check_guild", check = "crate::security::check_participant_role")]
pub async fn drop(
    ctx: Context<'_>,
    #[description = "Quest to drop"]
    #[autocomplete = "autocomplete_quest_id"]
     quest_id: String
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let hub = &ctx.data().sheets_hub;
    let sheet_id = &ctx.data().google_sheet_id;
    let user_id = ctx.author().id.to_string();

    let (_, _, schedule_opt, _, quest_title) = match get_quest_and_participant_data(ctx, &quest_id).await {
        Ok(data) => data,
        Err(e) => {
            ctx.say(format!("❌ Failed to fetch quest detail: {}", e)).await?;
            return Ok(());
        }
    };

    let schedule_iso = schedule_opt.ok_or_else(|| "Quest schedule not found.")?;

    let schedule_time = DateTime::parse_from_rfc3339(&schedule_iso).unwrap().timestamp();
    let now = chrono::Utc::now().timestamp();

    if now >= schedule_time {
        ctx.say("❌ Couldn't drop quest that has been started.").await?;
        return Ok(());
    }

    let participants_res = hub.spreadsheets().values_get(sheet_id, "Participants!A:D").doit().await;
    let mut found_on_progress = false;

    if let Ok((_, part_range)) = participants_res {
        if let Some(rows) = part_range.values {
            for row in rows.iter().skip(1) {
                if row.len() >= 4 {
                    let q_id = row[0].as_str().unwrap_or("");
                    let u_id = row[1].as_str().unwrap_or("");
                    let status = row[3].as_str().unwrap_or("");

                    if q_id == quest_id && u_id == user_id {
                        if status == "ON_PROGRESS" {
                            found_on_progress = true;
                            break;
                        } else {
                            ctx.say(format!("❌ Quest **{}** already: {}.", quest_title, status)).await?;
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    if !found_on_progress {
        ctx.say(format!("❌ This quest **{}** isn't taken or the status is invalid.", quest_title)).await?;
        return Ok(());
    }

    let payload = RegistrationPayload {
        quest_id: quest_id.clone(),
        user_id: user_id.clone(),
        user_tag: ctx.author().tag(),
    };

    produce_event(ctx, "DROP_QUEST", &payload).await?;

    ctx.say(format!("✅ Request to drop quest **{}** sucessfully sent. Slot will be returned.", quest_title)).await?;

    Ok(())
}

#[poise::command(slash_command, description_localized("en-US", "Submit a completed quest"),
                 check = "crate::security::check_guild", check = "crate::security::check_participant_role")]
pub async fn submit(
    ctx: Context<'_>,
    #[description = "Taken Quest"]
    #[autocomplete = "autocomplete_quest_id"]
    quest_id: String,
    #[description = "Upload Proof"] proof_image: Attachment,
) -> Result<(), Error> {

    if let Some(ctype) = &proof_image.content_type {
        if !ctype.starts_with("image/") {
            ctx.say("❌ Please upload an image (jpg/png).").await?;
            return Ok(());
        }
    } else {
        ctx.say("❌ Invalid file.").await?;
        return Ok(());
    }

    let payload = ProofPayload {
        quest_id: quest_id.clone(),
        user_id: ctx.author().id.to_string(),
        proof_url: proof_image.url.clone(),
    };

    produce_event(ctx, "SUBMIT_PROOF", &payload).await?;

    ctx.say(format!("✅ Proof for quest `{}` has been successfully submitted.", quest_id)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_wib_valid() {
        let input = "2025-11-20 19:00";
        let result = parse_wib(input);
        assert!(result.is_ok());
        let iso = result.unwrap();
        assert!(iso.contains("2025-11-20T19:00:00+07:00"));
    }

    #[test]
    fn test_parse_wib_invalid_format() {
        let input = "2025/11/20 19:00";
        let result = parse_wib(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_determine_organizer_creative_valid() {
        let res = determine_organizer(QuestCategory::CreativeArts, Division::Illust, None);
        assert_eq!(res.unwrap(), "Illust");
    }

    #[test]
    fn test_determine_organizer_creative_invalid() {
        let res = determine_organizer(QuestCategory::CreativeArts, Division::None, None);
        assert!(res.is_err());
    }

    #[test]
    fn test_determine_organizer_community_valid() {
        let res = determine_organizer(QuestCategory::Community, Division::None, Some("GenBalok".to_string()));
        assert_eq!(res.unwrap(), "GenBalok");
    }

    #[test]
    fn test_determine_organizer_community_invalid() {
        let res = determine_organizer(QuestCategory::Community, Division::None, None);
        assert!(res.is_err());
    }
}