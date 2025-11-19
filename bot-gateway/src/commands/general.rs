use crate::{Data, Error};
use poise::CreateReply;
use serenity::all::{CreateEmbed, CreateEmbedFooter};

type Context<'a> = poise::Context<'a, Data, Error>;

#[poise::command(slash_command, check = "crate::security::check_guild")]
pub async fn help(
    ctx: Context<'_>,
) -> Result<(), Error> {
    
    ctx.send(CreateReply::default()
        .embed(CreateEmbed::default()
            .title("📜 Quest Bot Command Help")
            .description("Below is a list of all commands available for managing and participating in quests.")
            .field(
                "⚔️ Quest Management (Quest-role or Admins)",
                "`/create` - Open a modal to **create a new quest**.\n` /edit <id>` - Open a modal to **edit** an existing quest.\n` /delete <id>` - **Delete** an existing quest.",
                false,
            )
            .field(
                "🗺️ Participant Actions (Guild Members)",
                "`/take <id>` - **Register** yourself as a participant.\n` /drop <id>` - **Unregister** from a quest (before start).\n` /submit <id> <attachment:image>` - **Submit** image proof for a taken quest.",
                false,
            )
            .field(
                "📊 Information & Utilities (Guild Members)",
                "`/list` - Show the **quest board** in a paginated view.\n` /stats` - Get a DM with your **personal quest statistics** and active quests.\n` /help` - Display this **help page**.",
                false,
            )
            .field(
                "👑 Admin Command (Admins Only)",
                "`/register_community <name> [leader]` - **Register a new community**.",
                false,
            )
            .color(0x3498DB) // A suitable blue color for info/help
            .footer(CreateEmbedFooter::new("Use the slash commands directly in the chat."))
        )
    ).await?;

    Ok(())
}