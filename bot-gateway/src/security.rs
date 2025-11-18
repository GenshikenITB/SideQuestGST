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