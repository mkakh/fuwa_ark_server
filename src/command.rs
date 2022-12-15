use serenity::model::channel::Message;
use serenity::prelude::*;
use tokio::process::Command;
use tracing::error;

pub async fn cmd_help(ctx: Context, msg: Message) {
    let mut m = "絶賛開発中！ by akiha\n".to_string();
    m.push_str(
        "・*/check_connection*：ポート公開用ソフト (playit.gg) が動作しているかを確認します\n",
    );
    m.push_str("・*/reload_connection*：ポート公開用ソフト (playit.gg) を再起動します\n");
    m.push_str(
        "・*/restart_server*：ARKサーバーを再起動します（再起動時に自動でアプデもされます）\n",
    );
    m.push_str("・*/stop_server*：ARKサーバーを終了します\n");
    m.push_str("・*/start_server*：ARKサーバーを起動します\n");
    m.push_str("・*/save*：ARKサーバーでセーブを実行します\n");
    if let Err(e) = msg.channel_id.say(&ctx.http, m).await {
        error!("Error sending message: {:?}", e);
    }
}

pub async fn cmd_say_hello(ctx: Context, msg: Message) {
    let output = Command::new("powershell")
        .arg("-NonInteractive")
        .arg("-File")
        .arg(r#"C:/Users/akh/Documents/ark-rcon.ps1"#)
        .arg(r#""BroadCast Hello. This is a test msg from Discord BOT""#)
        .output()
        .await
        .expect("failed to start `rcon`");
    let m = if output.stdout.is_empty() {
        "Succeeded".to_string()
    } else {
        String::from_utf8_lossy(&output.stdout).to_string()
    };
    if let Err(e) = msg.channel_id.say(&ctx.http, m).await {
        error!("Error sending message: {:?}", e);
    }
}

pub async fn cmd_save(ctx: Context, msg: Message) {
    let output = Command::new("powershell")
        .arg("-NonInteractive")
        .arg("-File")
        .arg(r#"C:/Users/akh/Documents/ark-rcon.ps1"#)
        .arg(r#""SaveWorld""#)
        .output()
        .await
        .expect("failed to start `rcon`");
    let m = if output.stdout.is_empty() {
        "Succeeded".to_string()
    } else {
        String::from_utf8_lossy(&output.stdout).to_string()
    };
    if let Err(e) = msg.channel_id.say(&ctx.http, m).await {
        error!("Error sending message: {:?}", e);
    }
}

pub async fn cmd_check_connection(ctx: Context, msg: Message) {
    let raw_output = Command::new("powershell")
        .arg(r#"scripts/check_connection.ps1"#)
        .output()
        .await
        .expect("failed to start `check_connection`");
    let output = String::from_utf8_lossy(&raw_output.stdout);
    let m = if output == "0" { "No" } else { "Yes" };
    if let Err(e) = msg.channel_id.say(&ctx.http, m).await {
        error!("Error sending message: {:?}", e);
    }
}

pub async fn cmd_reload_connection(ctx: Context, msg: Message) {
    Command::new("powershell")
        .arg(r#"C:/Users/akh/Documents/ark-playit-restart.ps1"#)
        .output()
        .await
        .expect("failed to start `reload_connection`");
    if let Err(e) = msg.channel_id.say(&ctx.http, "Reloaded").await {
        error!("Error sending message: {:?}", e);
    }
}
