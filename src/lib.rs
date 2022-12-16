use anyhow::anyhow;
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use shuttle_secrets::SecretStore;
use tracing::{error, info};
mod command;
pub use crate::command::*;

struct Bot;
#[async_trait]
impl EventHandler for Bot {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "/help" {
            cmd_help(ctx, msg).await;
        } else if msg.content == "/save" {
            cmd_save(ctx, msg).await;
        } else if msg.content == "/say_hello" {
            cmd_say_hello(ctx, msg).await;
        } else if msg.content == "/check_connection" {
            cmd_check_connection(ctx, msg).await;
        } else if msg.content == "/listplayers" {
            cmd_listplayers(ctx, msg).await;
        } else if msg.content == "/reload_connection" {
            cmd_reload_connection(ctx, msg).await;
        } else if msg.content == "/restart_server" {
            // steamcmd？
            // RCON?
            if let Err(e) = msg.channel_id.say(&ctx.http, "未実装").await {
                error!("Error sending message: {:?}", e);
            }
        } else if msg.content == "/stop_server" {
            // steamcmd？
            // RCON?
            if let Err(e) = msg.channel_id.say(&ctx.http, "未実装").await {
                error!("Error sending message: {:?}", e);
            }
        } else if msg.content == "/start_server" {
            // steamcmd
            if let Err(e) = msg.channel_id.say(&ctx.http, "未実装").await {
                error!("Error sending message: {:?}", e);
            }
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
    }
}

#[shuttle_service::main]
async fn serenity(
    #[shuttle_secrets::Secrets] secret_store: SecretStore,
) -> shuttle_service::ShuttleSerenity {
    // Get the discord token set in `Secrets.toml`
    let token = if let Some(token) = secret_store.get("DISCORD_TOKEN") {
        token
    } else {
        return Err(anyhow!("'DISCORD_TOKEN' was not found").into());
    };

    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

    let client = Client::builder(&token, intents)
        .event_handler(Bot)
        .await
        .expect("Err creating client");

    Ok(client)
}
