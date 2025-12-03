use poise::CreateReply;
use serenity::all::{Channel, Role};

use crate::{Data, Error, cache::{get_guild_config, set_guild_config}};

type Context<'a> = poise::Context<'a, Data, Error>;

#[poise::command(
    slash_command, 
    subcommands("set_channel", "set_role", "view"),
    description_localized("en-US", "Configure bot settings"),
    check = "crate::security::check_admin"
)]
pub async fn config(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command, description_localized("en-US", "Set the announcement channel"))]
pub async fn set_channel(
    ctx: Context<'_>,
    #[description = "Channel for quest announcements"] channel: Channel,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?;

    let mut config = get_guild_config(ctx, guild_id.get()).await.unwrap_or_default();
    config.announcement_channel_id = Some(channel.id().get());
    set_guild_config(ctx, guild_id.get(), &config).await?;

    ctx.send(CreateReply::default()
        .content(format!("✅ Announcement channel set to <#{}>", channel.id()))
        .ephemeral(true)
    ).await?;
    Ok(())
}

#[poise::command(slash_command, description_localized("en-US", "Set the role to ping for announcements"))]
pub async fn set_role(
    ctx: Context<'_>,
    #[description = "Role to ping for quest announcements"] role: Role,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?;

    let mut config = get_guild_config(ctx, guild_id.get()).await.unwrap_or_default();
    config.ping_role_id = Some(role.id.get());
    set_guild_config(ctx, guild_id.get(), &config).await?;

    ctx.send(CreateReply::default()
        .content(format!("✅ Ping role set to <@&{}>", role.id))
        .ephemeral(true)
    ).await?;
    Ok(())
}

#[poise::command(slash_command, description_localized("en-US", "View current bot configuration"))]
pub async fn view(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?;
    
    let config = get_guild_config(ctx, guild_id.get()).await.unwrap_or_default();
    
    let channel_str = config.announcement_channel_id
        .map(|id| format!("<#{}>", id))
        .unwrap_or_else(|| "Not set (uses command channel)".to_string());
    
    let role_str = config.ping_role_id
        .map(|id| format!("<@&{}>", id))
        .unwrap_or_else(|| "Not set (uses default participant role)".to_string());
    
    ctx.send(CreateReply::default()
        .content(format!(
            "**⚙️ Current Configuration**\n\n📢 Announcement Channel: {}\n🔔 Ping Role: {}",
            channel_str, role_str
        ))
        .ephemeral(true)
    ).await?;
    
    Ok(())
}