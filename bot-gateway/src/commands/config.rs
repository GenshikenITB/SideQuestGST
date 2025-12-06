use poise::CreateReply;
use serenity::all::{Channel, Role};

use crate::{Data, Error, cache::{get_guild_config, set_guild_config}, models::{ChannelConfigType, RoleConfigType}};

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

#[poise::command(slash_command, description_localized("en-US", "Set a channel for bot features"))]
pub async fn set_channel(
    ctx: Context<'_>,
    #[description = "Which channel to configure"] config_type: ChannelConfigType,
    #[description = "The channel to use"] channel: Channel,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?;

    let mut config = get_guild_config(ctx, guild_id.get()).await.unwrap_or_default();

    let label = match config_type {
        ChannelConfigType::Announcement => {
            config.announcement_channel_id = Some(channel.id().get());
            "Announcement channel"
        }
        ChannelConfigType::Proof => {
            config.proof_channel_id = Some(channel.id().get());
            "Proof submission channel"
        }
        ChannelConfigType::Log => {
            config.log_channel_id = Some(channel.id().get());
            "Log channel"
        }
    };
    
    set_guild_config(ctx, guild_id.get(), &config).await?;

    ctx.send(CreateReply::default()
        .content(format!("✅ {} set to <#{}>", label, channel.id()))
        .ephemeral(true)
    ).await?;
    Ok(())
}

#[poise::command(slash_command, description_localized("en-US", "Set a role for bot features"))]
pub async fn set_role(
    ctx: Context<'_>,
    #[description = "Which role to configure"] config_type: RoleConfigType,
    #[description = "The role to use"] role: Role,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?;

    let mut config = get_guild_config(ctx, guild_id.get()).await.unwrap_or_default();

    let label = match config_type {
        RoleConfigType::Ping => {
            config.ping_role_id = Some(role.id.get());
            "Ping role"
        }
        RoleConfigType::QuestGiver => {
            config.quest_giver_role_id = Some(role.id.get());
            "Quest Giver role"
        }
        RoleConfigType::Verifier => {
            config.verifier_role_id = Some(role.id.get());
            "Verifier role"
        }
    };
    
    set_guild_config(ctx, guild_id.get(), &config).await?;

    ctx.send(CreateReply::default()
        .content(format!("✅ {} set to <@&{}>", label, role.id))
        .ephemeral(true)
    ).await?;
    Ok(())
}

#[poise::command(slash_command, description_localized("en-US", "Clear a channel configuration"))]
pub async fn clear_channel(
    ctx: Context<'_>,
    #[description = "Which channel config to clear"] config_type: ChannelConfigType,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?;

    let mut config = get_guild_config(ctx, guild_id.get()).await.unwrap_or_default();

    let label = match config_type {
        ChannelConfigType::Announcement => {
            config.announcement_channel_id = None;
            "Announcement channel"
        }
        ChannelConfigType::Proof => {
            config.proof_channel_id = None;
            "Proof submission channel"
        }
        ChannelConfigType::Log => {
            config.log_channel_id = None;
            "Log channel"
        }
    };

    set_guild_config(ctx, guild_id.get(), &config).await?;

    ctx.send(CreateReply::default()
        .content(format!("✅ {} cleared (will use default/command channel)", label))
        .ephemeral(true)
    ).await?;

    Ok(())
}

#[poise::command(slash_command, description_localized("en-US", "Clear a role configuration"))]
pub async fn clear_role(
    ctx: Context<'_>,
    #[description = "Which role config to clear"] config_type: RoleConfigType,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?;

    let mut config = get_guild_config(ctx, guild_id.get()).await.unwrap_or_default();

    let label = match config_type {
        RoleConfigType::Ping => {
            config.ping_role_id = None;
            "Ping role"
        }
        RoleConfigType::QuestGiver => {
            config.quest_giver_role_id = None;
            "Quest Giver role"
        }
        RoleConfigType::Verifier => {
            config.verifier_role_id = None;
            "Verifier role"
        }
    };

    set_guild_config(ctx, guild_id.get(), &config).await?;

    ctx.send(CreateReply::default()
        .content(format!("✅ {} cleared (will use default role)", label))
        .ephemeral(true)
    ).await?;

    Ok(())
}

#[poise::command(slash_command, description_localized("en-US", "View current bot configuration"))]
pub async fn view(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must be used in a guild")?;
    
    let config = get_guild_config(ctx, guild_id.get()).await.unwrap_or_default();

    let fmt_channel = |opt: Option<u64>, default: &str| {
        opt.map(|id| format!("<#{}>", id)).unwrap_or_else(|| default.to_string())
    };

    let fmt_role = |opt: Option<u64>, default: &str| {
        opt.map(|id| format!("<@&{}>", id)).unwrap_or_else(|| default.to_string())
    };
    
    let content = format!(
        "**⚙️ Current Configuration**\n\n\
        **Channels**\n\
        📢 Announcement: {}\n\
        📝 Proof Submission: {}\n\
        📋 Log: {}\n\n\
        **Roles**\n\
        🔔 Ping Role: {}\n\
        🎖️ Quest Giver: {}\n\
        ✅ Verifier: {}",
        fmt_channel(config.announcement_channel_id, "Not set (command channel)"),
        fmt_channel(config.proof_channel_id, "Not set (command channel)"),
        fmt_channel(config.log_channel_id, "Not set (disabled)"),
        fmt_role(config.ping_role_id, "Not set (default participant)"),
        fmt_role(config.quest_giver_role_id, "Not set (env default)"),
        fmt_role(config.verifier_role_id, "Not set (admin only)"),
    );
    
    ctx.send(CreateReply::default()
        .content(content)
        .ephemeral(true)
    ).await?;
    
    Ok(())
}