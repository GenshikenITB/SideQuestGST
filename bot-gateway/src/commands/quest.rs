use crate::{Data, Error};
use crate::models::{QuestPayload, QuestCategory, Division, RegistrationPayload};
use crate::kafka::produce_event;
use poise::Modal as _;
use poise::CreateReply;
use serenity::all::{CreateEmbed, CreateEmbedFooter};
use chrono::{TimeZone, NaiveDateTime, FixedOffset};
use chrono::DateTime;

type Context<'a> = poise::Context<'a, Data, Error>;

fn parse_wib(input: &str) -> Result<String, String> {
    let naive = NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M")
        .map_err(|_| "Wrong time format! Use: YYYY-MM-DD HH:MM (E.g: 2025-11-25 19:30)".to_string())?;

    let wib_offset = FixedOffset::east_opt(7 * 3600).unwrap();
    let dt_wib = wib_offset.from_local_datetime(&naive).unwrap();
    
    Ok(dt_wib.to_rfc3339())
}

#[poise::command(slash_command, check = "crate::security::check_guild")] 
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
        
        #[name = "Description"]
        #[paragraph]
        description: String,

        #[name = "Start Time (YYYY-MM-DD HH:MM)"]
        #[placeholder = "E.g: 2025-11-25 19:00"]
        #[min_length = 16] 
        #[max_length = 16]
        schedule: String,

        #[name = "Deadline (YYYY-MM-DD HH:MM)"]
        #[placeholder = "Empty if same as start time"]
        deadline: Option<String>,

        #[name = "Platform / Location"]
        platform: String,
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

        let quest_id = uuid::Uuid::new_v4().to_string();

        let payload = QuestPayload {
            quest_id: quest_id.clone(),
            title: data.title.clone(),
            description: data.description,
            category: format!("{:?}", category),
            organizer_name: organizer_final,
            schedule: schedule_iso.clone(),
            platform: data.platform.clone(),
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
                 .footer(CreateEmbedFooter::new("Use /take_quest <id> to take the quest"))
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
    
    let payload = RegistrationPayload {
        quest_id: quest_id.clone(),
        user_id: ctx.author().id.to_string(),
        user_tag: ctx.author().tag(),
    };

    produce_event(ctx, "TAKE_QUEST", &payload).await?;
    ctx.say("✅ Request ambil quest terkirim.").await?;
    Ok(())
}