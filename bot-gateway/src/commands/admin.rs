use crate::{Data, Error};
use crate::models::NewCommunityPayload;
use crate::kafka::produce_event;

type Context<'a> = poise::Context<'a, Data, Error>;

#[poise::command(slash_command, check = "crate::security::check_admin")]
pub async fn register_community(
    ctx: Context<'_>,
    #[description = "Community Name"] name: String,
    #[description = "Community Leader (Mention user)"] leader: Option<poise::serenity_prelude::User>,
) -> Result<(), Error> {
    
    let leader_id = match leader {
        Some(u) => u.id.to_string(),
        None => "Unknown".to_string(),
    };

    let payload = NewCommunityPayload {
        community_name: name.clone(),
        leader_id,
    };

    produce_event(ctx, "REGISTER_COMMUNITY", &payload).await?;

    ctx.say(format!("✅ Community **{}** has successfully registered!", name)).await?;
    Ok(())
}