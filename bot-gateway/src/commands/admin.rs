use crate::{Data, Error};
use crate::models::NewCommunityPayload;
use crate::kafka::produce_event;
use common::normalize_name;

type Context<'a> = poise::Context<'a, Data, Error>;

pub fn determine_leader_id(leader: Option<&poise::serenity_prelude::User>) -> String {
    match leader {
        Some(u) => u.id.to_string(),
        None => "Unknown".to_string(),
    }
}

#[poise::command(slash_command, description_localized("en-US", "Register a new community"), check = "crate::security::check_admin")]
pub async fn register_community(
    ctx: Context<'_>,
    #[description = "Community Name"] name: String,
    #[description = "Community Leader (Mention user)"] leader: Option<poise::serenity_prelude::User>,
) -> Result<(), Error> {
    
    let leader_id = determine_leader_id(leader.as_ref());

    let hub = &ctx.data().sheets_hub;
    let sheet_id = &ctx.data().google_sheet_id;
    match hub.spreadsheets().values_get(sheet_id, "Communities!A:A").doit().await {
        Ok((_, range)) => {
            if let Some(rows) = range.values {
                let target = normalize_name(&name);
                for row in rows.iter().skip(1) {
                    if let Some(cell) = row.get(0).and_then(|v| v.as_str()) {
                        if normalize_name(cell) == target {
                            ctx.say(format!("❌ Community `{}` already registered.", name)).await?;
                            return Ok(());
                        }
                    }
                }
            }
        }
        Err(e) => {
            ctx.say(format!("❌ Failed to check existing communities: {}", e)).await?;
            return Ok(());
        }
    }

    let payload = NewCommunityPayload {
        community_name: name.clone(),
        leader_id,
    };

    produce_event(ctx, "REGISTER_COMMUNITY", &payload).await?;

    ctx.say(format!("✅ Community **{}** has successfully registered!", name)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use poise::serenity_prelude::{User, UserId};

    #[test]
    fn test_determine_leader_id_unknown() {
        let res = determine_leader_id(None);
        assert_eq!(res, "Unknown");
    }

    #[test]
    fn test_determine_leader_id_some() {
        let mut user = User::default();
        user.id = UserId::new(12345);
        
        let res = determine_leader_id(Some(&user));
        assert_eq!(res, "12345");
    }
}