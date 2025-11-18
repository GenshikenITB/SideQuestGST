use crate::{Data, Error, HubType};
use crate::models::{QuestPayload, QuestCategory, Division, RegistrationPayload, ProofPayload};
use crate::kafka::produce_event;
use poise::Modal as _;
use poise::CreateReply;
use serenity::all::{CreateEmbed, CreateEmbedFooter, Attachment};
use chrono::{TimeZone, NaiveDateTime, FixedOffset, DateTime};

type Context<'a> = poise::Context<'a, Data, Error>;

fn parse_wib(input: &str) -> Result<String, String> {
    let naive = NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M")
        .map_err(|_| "Wrong time format! Use: YYYY-MM-DD HH:MM (E.g: 2025-11-25 19:30)".to_string())?;

    let wib_offset = FixedOffset::east_opt(7 * 3600).unwrap();
    let dt_wib = wib_offset.from_local_datetime(&naive).unwrap();
    
    Ok(dt_wib.to_rfc3339())
}

async fn get_quest_and_participant_data(hub: &HubType, sheet_id: &str, quest_id: &str) -> Result<(i8, i8, Option<String>, String), Error> {
    let result = hub.spreadsheets().values_batch_get(sheet_id)
        .add_ranges("Quests!A:I")
        .add_ranges("Participants!A:B")
        .doit()
        .await?;
    
    let value_ranges = result.1.value_ranges.unwrap_or_default();
    if value_ranges.len() < 2 { return Err("Failed to fetch necessary sheet ranges.".into()); }

    let mut max_slots: i8 = 0;
    let mut schedule_iso: Option<String> = None;
    let mut quest_title = "Unknown Quest".to_string();
    let mut found = false;

    if let Some(q_rows) = &value_ranges[0].values {
        for row in q_rows {
            if row.len() >= 9 && row[0].as_str().unwrap_or("") == quest_id {
                quest_title = row[1].as_str().unwrap_or("Unknown").to_string();
                max_slots = row[8].as_str().unwrap_or("0").parse::<i8>().unwrap_or(0);
                schedule_iso = Some(row[4].as_str().unwrap_or("").to_string());
                found = true;
                break;
            }
        }
    }

    if !found {
        return Err(format!("Quest ID `{}` not found or slots not defined.", quest_id).into());
    }

    let mut current_participants: i8 = 0;
    if let Some(p_rows) = &value_ranges[1].values {
        for row in p_rows {
            if row.len() >= 2 && row[0].as_str().unwrap_or("") == quest_id {
                current_participants += 1;
            }
        }
    }

    current_participants = current_participants.saturating_sub(1); 

    Ok((max_slots, current_participants, schedule_iso, quest_title))
}

#[poise::command(slash_command, check = "crate::security::check_quest_role")] 
pub async fn create(
    ctx: Context<'_>,
    
    #[description = "Select Quest Category"]
    category: QuestCategory,

    #[description = "Select Division ('None' if Community)"]
    division: Division,

    #[description = "Community Name (Fill only if Community)"]
    community_name: Option<String>,

) -> Result<(), Error> {
    let organizer_final = match category {
        QuestCategory::CreativeArts => {
            if let Division::None = division {
                ctx.say("❌ Error: Expected Division Name.").await?;
                return Ok(());
            }
            format!("{:?}", division) 
        },
        QuestCategory::Community => {
            match community_name {
                Some(name) => name,
                None => {
                    ctx.say("❌ Error: Expected Community Name.").await?;
                    return Ok(());
                }
            }
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
            deadline: deadline_iso,
            creator_id: ctx.author().id.to_string(),
        };
        let display_ts = DateTime::parse_from_rfc3339(&schedule_iso)
            .unwrap()
            .timestamp();

        ctx.send(CreateReply::default()
            .embed(CreateEmbed::default()
                .title(format!("⚔️ New Quest: {}", payload.title))
                 .description(&payload.description)
                 .field("📁 Category", &payload.category, true)
                 .field("🛡️ By", &payload.organizer_name, true)
                 .field("📅 Start Time", format!("<t:{}:f>", display_ts), true)
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

#[poise::command(slash_command, check = "crate::security::check_guild")]
pub async fn take(
    ctx: Context<'_>,
    #[description = "Quest ID"] quest_id: String
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let hub = &ctx.data().sheets_hub;
    let sheet_id = &ctx.data().google_sheet_id;
    let user_id = ctx.author().id.to_string();
    
    match get_quest_and_participant_data(hub, sheet_id, &quest_id).await {
        Ok((max_slots, current_participants, _, quest_title)) => {
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

#[poise::command(slash_command, check = "crate::security::check_guild")]
pub async fn drop(
    ctx: Context<'_>,
    #[description = "Quest ID to drop"] quest_id: String
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let hub = &ctx.data().sheets_hub;
    let sheet_id = &ctx.data().google_sheet_id;
    let user_id = ctx.author().id.to_string();

    let (_, _, schedule_opt, quest_title) = match get_quest_and_participant_data(hub, sheet_id, &quest_id).await {
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

#[poise::command(slash_command, check = "crate::security::check_guild")]
pub async fn submit(
    ctx: Context<'_>,
    #[description = "Taken Quest ID"] quest_id: String,
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