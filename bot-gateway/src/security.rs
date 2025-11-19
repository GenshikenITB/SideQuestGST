use poise::CreateReply;

use crate::{Data, Error};

type Context<'a> = poise::Context<'a, Data, Error>;

pub async fn check_guild(ctx: Context<'_>) -> Result<bool, Error> {
    let guild_id = match ctx.guild_id() {
        Some(id) => id,
        None => {
            ctx.say("❌ This command doesnt work on DMs").await?;
            return Ok(false);
        }
    };

    let target_guild = ctx.data().target_guild_id;

    if guild_id != target_guild {
         ctx.send(CreateReply::default()
            .content("🚫 Access Denied: This command only works on GST Server.").ephemeral(true)
        ).await?;
        
        return Ok(false);
    }

    Ok(true)
}

pub async fn check_admin(ctx: Context<'_>) -> Result<bool, Error> {
    let is_admin = ctx.author_member().await
        .map(|m| m.permissions.map(|p| p.administrator()).unwrap_or(false))
        .unwrap_or(false);

    if !is_admin {
        ctx.send(CreateReply::default().content("⛔ You dont have admin access.").ephemeral(true)).await?;
        return Ok(false);
    }
    Ok(true)
}

pub async fn check_quest_role(ctx: Context<'_>) -> Result<bool, Error> {
    if !check_guild(ctx).await? {
        return Ok(false);
    }

    let quest_role = ctx.data().qg_role_id;
    let user = ctx.author_member().await.ok_or("Failed to get member")?;

    let has_role = user.roles.contains(&quest_role);

    let is_admin = user.permissions.map(|p| p.administrator()).unwrap_or(false);

    if has_role || is_admin {
        Ok(true)
    } else {
        ctx.send(poise::CreateReply::default()
            .content("⛔ Access Denied: Only Staff with QuestRole can use this command.")
            .ephemeral(true)
        ).await?;
        Ok(false)
    }
}

pub async fn check_participant_role(ctx: Context<'_>) -> Result<bool, Error> {
    if !check_guild(ctx).await? {
        return Ok(false);
    }

    let participant_role = ctx.data().participant_role_id;
    let user = ctx.author_member().await.ok_or("Failed to get member")?;

    let has_role = user.roles.contains(&participant_role);

    let is_admin = user.permissions.map(|p| p.administrator()).unwrap_or(false);

    if has_role || is_admin {
        Ok(true)
    } else {
        ctx.send(poise::CreateReply::default()
            .content("⛔ Access Denied: Only CaStaff can use this command.")
            .ephemeral(true)
        ).await?;
        Ok(false)
    }
}