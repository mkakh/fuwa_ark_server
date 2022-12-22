use std::collections::{HashMap, HashSet};
use std::io::{copy, Read, Write};
use std::sync::Arc;

use serenity::async_trait;
use serenity::client::bridge::gateway::ShardManager;
use serenity::framework::standard::macros::{command, group, help, hook};
use serenity::framework::standard::{
    help_commands, Args, CommandGroup, CommandResult, DispatchError, HelpOptions, StandardFramework,
};

use rcon::{AsyncStdStream, Connection, Error};
use serenity::http::Http;
use serenity::model::channel::Message;
use serenity::model::gateway::{GatewayIntents, Ready};
use serenity::model::id::UserId;
use serenity::prelude::*;
use serenity::utils::{content_safe, ContentSafeOptions};
use std::fs::File;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use walkdir::WalkDir;
use zip::write::FileOptions;

const BACKUP_DIR_PATH: &str = "C:/asmdata/akhBackups";
const ARK_SAVEDATA_PATH: &str = "C:/asmdata/Servers/Server2/ShooterGame/Saved/SavedArks";

// A container type is created for inserting into the Client's `data`, which
// allows for data to be accessible across all events and framework commands, or
// anywhere else that has a copy of the `data` Arc.
struct ShardManagerContainer;

impl TypeMapKey for ShardManagerContainer {
    type Value = Arc<Mutex<ShardManager>>;
}

struct CommandCounter;

impl TypeMapKey for CommandCounter {
    type Value = HashMap<String, u64>;
}
struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[group]
#[commands(
    broadcast,
    listplayers,
    save,
    listbackups,
    rollback,
    check_connection,
    reload_connection,
    check_server,
    start_server,
    shutdown_server
)]
struct General;

#[help]
#[individual_command_tip = "ふわふわARK BOT"]
#[command_not_found_text = "Could not find: `{}`."]
// Define the maximum Levenshtein-distance between a searched command-name
// and commands. If the distance is lower than or equal the set distance,
// it will be displayed as a suggestion.
// Setting the distance to 0 will disable suggestions.
#[max_levenshtein_distance(3)]
// On another note, you can set up the help-menu-filter-behaviour.
// Here are all possible settings shown on all possible options.
// First case is if a user lacks permissions for a command, we can hide the command.
#[lacking_permissions = "Hide"]
// If the user is nothing but lacking a certain role, we just display it hence our variant is `Nothing`.
#[lacking_role = "Nothing"]
// The last `enum`-variant is `Strike`, which ~~strikes~~ a command.
#[wrong_channel = "Strike"]
// Serenity will automatically analyse and generate a hint/tip explaining the possible
// cases of ~~strikethrough-commands~~, but only if
// `strikethrough_commands_tip_in_{dm, guild}` aren't specified.
// If you pass in a value, it will be displayed instead.
async fn my_help(
    context: &Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>,
) -> CommandResult {
    let _ = help_commands::with_embeds(context, msg, args, help_options, groups, owners).await;
    Ok(())
}

#[hook]
async fn before(ctx: &Context, msg: &Message, command_name: &str) -> bool {
    println!(
        "Got command '{}' by user '{}'",
        command_name, msg.author.name
    );

    // Increment the number of times this command has been run once. If
    // the command's name does not exist in the counter, add a default
    // value of 0.
    let mut data = ctx.data.write().await;
    let counter = data
        .get_mut::<CommandCounter>()
        .expect("Expected CommandCounter in TypeMap.");
    let entry = counter.entry(command_name.to_string()).or_insert(0);
    *entry += 1;

    true // if `before` returns false, command processing doesn't happen.
}

#[hook]
async fn after(_ctx: &Context, _msg: &Message, command_name: &str, command_result: CommandResult) {
    match command_result {
        Ok(()) => println!("Processed command '{}'", command_name),
        Err(why) => println!("Command '{}' returned error {:?}", command_name, why),
    }
}

#[hook]
async fn unknown_command(_ctx: &Context, _msg: &Message, unknown_command_name: &str) {
    println!("Could not find command named '{}'", unknown_command_name);
}

#[hook]
async fn normal_message(_ctx: &Context, msg: &Message) {
    println!("Message is not a command '{}'", msg.content);
}

#[hook]
async fn delay_action(ctx: &Context, msg: &Message) {
    // You may want to handle a Discord rate limit if this fails.
    let _ = msg.react(ctx, '⏱').await;
}

#[hook]
async fn dispatch_error(ctx: &Context, msg: &Message, error: DispatchError, _command_name: &str) {
    if let DispatchError::Ratelimited(info) = error {
        // We notify them only once.
        if info.is_first_try {
            let _ = msg
                .channel_id
                .say(
                    &ctx.http,
                    &format!("Try this again in {} seconds.", info.as_secs()),
                )
                .await;
        }
    }
}

#[tokio::main]
async fn main() {
    let token = std::fs::read_to_string("discord_token").expect("could not read DISCORD_TOKEN");

    std::thread::spawn(|| loop {
        let rt = tokio::runtime::Runtime::new().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(3600000));
        rt.block_on(async {
            let output = rcon("SaveWorld").await.expect("failed to run `rcon`");

            if output.is_empty() {
                println!("backup failed");
            } else {
                create_backup().await.expect("failed to create a backup");
            }
        });
    });

    let http = Http::new(&token);

    // fetch bot's owners and id
    let (owners, bot_id) = match http.get_current_application_info().await {
        Ok(info) => {
            let mut owners = HashSet::new();
            if let Some(team) = info.team {
                owners.insert(team.owner_user_id);
            } else {
                owners.insert(info.owner.id);
            }
            match http.get_current_user().await {
                Ok(bot_id) => (owners, bot_id.id),
                Err(why) => panic!("Could not access the bot id: {:?}", why),
            }
        }
        Err(why) => panic!("Could not access application info: {:?}", why),
    };

    let framework = StandardFramework::new()
        .configure(|c| {
            c.with_whitespace(true)
                .on_mention(Some(bot_id))
                .prefix("/")
                // In this case, if "," would be first, a message would never
                // be delimited at ", ", forcing you to trim your arguments if you
                // want to avoid whitespaces at the start of each.
                .delimiters(vec![", ", ","])
                // Sets the bot's owners. These will be used for commands that
                // are owners only.
                .owners(owners)
        })
        .before(before)
        .after(after)
        .unrecognised_command(unknown_command)
        .normal_message(normal_message)
        .on_dispatch_error(dispatch_error)
        .help(&MY_HELP)
        .group(&GENERAL_GROUP);

    // To run properly, the "Presence Intent" and "Server Members Intent" options need to be enabled.
    // These are needed so the `required_permissions` macro works on the commands that need to use it.
    // You will need to enable these 2 options on the bot application, and possibly wait up to 5 minutes.
    let intents = GatewayIntents::all();
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .framework(framework)
        .type_map_insert::<CommandCounter>(HashMap::default())
        .await
        .expect("Err creating client");

    {
        let mut data = client.data.write().await;
        data.insert::<ShardManagerContainer>(Arc::clone(&client.shard_manager));
    }

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}

// say something to Discord channel while ensuring that user and role mentions are replaced with a safe textual alternative.
#[command]
async fn say(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    match args.single_quoted::<String>() {
        Ok(x) => {
            let settings = if let Some(guild_id) = msg.guild_id {
                // By default roles, users, and channel mentions are cleaned.
                ContentSafeOptions::default()
                    // We do not want to clean channal mentions as they
                    // do not ping users.
                    .clean_channel(false)
                    // If it's a guild channel, we want mentioned users to be displayed
                    // as their display name.
                    .display_as_member_from(guild_id)
            } else {
                ContentSafeOptions::default()
                    .clean_channel(false)
                    .clean_role(false)
            };

            let content = content_safe(&ctx.cache, x, &settings, &msg.mentions);

            msg.channel_id.say(&ctx.http, &content).await?;

            return Ok(());
        }
        Err(_) => {
            msg.reply(ctx, "An argument is required to run this command.")
                .await?;
            return Ok(());
        }
    };
}

#[command]
#[description = "ポート公開用ソフト (playit.gg) が動作しているかを確認します"]
async fn check_connection(ctx: &Context, msg: &Message) -> CommandResult {
    let raw_output = Command::new("powershell")
        .arg(r#"scripts/check_connection.ps1"#)
        .output()
        .await
        .expect("failed to start `check_connection`");
    let output = String::from_utf8_lossy(&raw_output.stdout);
    let m = if output == "0" {
        "playit.ggが起動されていません．\n*/reload_connection*を実行してplayit.ggを起動してください．"
    } else {
        "playit.ggは実行中です．\n回線に問題がある場合は*/reload_connection*を実行してください．"
    };
    msg.reply(&ctx.http, m).await?;
    Ok(())
}

async fn num_listplayers() -> usize {
    if let Ok(output) = rcon("listplayers").await {
        output.chars().filter(|x| *x == ',').count()
    } else {
        1001001001
    }
}

#[command]
#[allowed_roles("ARK Server Admin")]
#[description = "ポート公開用ソフト (playit.gg) を再起動します"]
async fn reload_connection(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    if (num_listplayers().await == 0 || num_listplayers().await == 1001001001)
        || (!args.is_empty() && args.rest() == "force")
    {
        let result = Command::new("powershell")
            .arg(r#"C:/Users/akh/Documents/ark-playit-restart.ps1"#)
            .output()
            .await;
        if result.is_err() {
            msg.reply(
                &ctx.http,
                "reload_connectionの実行に失敗しました\n再実行してください",
            )
            .await?;
        } else {
            msg.reply(&ctx.http, "Connection Reloaded").await?;
        }
    } else {
        msg.reply(&ctx.http, "ゲームにプレイヤーが残っていたため，再起動を中止しました．\n強制再起動をする場合は*/reload_connection force*を実行してください．").await?;
    }
    Ok(())
}

#[command]
#[description = "ゲームをセーブします"]
async fn save(ctx: &Context, msg: &Message) -> CommandResult {
    let output = rcon("SaveWorld").await.expect("failed to run `rcon`");

    if output.is_empty() {
        msg.reply(&ctx.http, "No output was returned").await?;
    } else {
        msg.reply(&ctx.http, "セーブとバックアップを開始しました")
            .await?;
        create_backup().await?;
        msg.reply(&ctx.http, output).await?;
    };
    Ok(())
}

async fn rcon(cmd: &str) -> Result<String, Error> {
    fn trim_newline(s: &str) -> String {
        let mut str = s.to_owned();
        if str.ends_with('\n') {
            str.pop();
            if str.ends_with('\r') {
                str.pop();
            }
        }
        str
    }

    let pass = trim_newline(
        &std::fs::read_to_string("rcon_password").expect("could not read RCON Password"),
    );
    let mut conn = <Connection<AsyncStdStream>>::builder()
        .enable_factorio_quirks(true)
        .connect("127.0.0.1:32330", &pass)
        .await?;
    let resp = conn.cmd(cmd).await?;
    Ok(resp)
}

#[command]
#[description = "ゲーム内に文字列を表示します"]
#[allowed_roles("ARK Server Admin")]
async fn broadcast(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    if !args.is_empty() {
        msg.reply(&ctx.http, &format!("[Broadcast]\n{}", args.rest()))
            .await?;
        rcon(&format!("Broadcast {}", args.rest()))
            .await
            .expect("failed to run `rcon`");
    } else {
        msg.reply(&ctx.http, "An argument is required").await?;
    }
    Ok(())
}

#[command]
#[description = "サーバーを起動します"]
#[allowed_roles("ARK Server Admin")]
async fn start_server(ctx: &Context, msg: &Message) -> CommandResult {
    let output = rcon("listplayers").await.expect("failed to run `rcon`");
    if output.is_empty() {
        let raw_output = Command::new("powershell")
            .arg(r#"scripts/start_ark_server.ps1"#)
            .output()
            .await
            .expect("failed to start `check_connection`");
        let output = String::from_utf8_lossy(&raw_output.stdout);
        if output.is_empty() {
            msg.reply(&ctx.http, "No output was returned").await?;
        } else {
            msg.reply(&ctx.http, output).await?;
        }
    } else {
        msg.reply(&ctx.http, "ARKサーバーは既に動作中です").await?;
    }
    Ok(())
}

#[command]
#[description = "サーバーを再起動します"]
#[allowed_roles("ARK Server Admin")]
async fn restart_server(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    if num_listplayers().await == 0 || (!args.is_empty() && args.rest() == "force") {
        msg.reply(&ctx.http, "ゲームをセーブします").await?;
        let mut save_succeeded_flag = false;
        for i in 0..3 {
            let output = rcon("SaveWorld").await.expect("failed to run `rcon`");

            if output.is_empty() {
                if i != 2 {
                    msg.reply(&ctx.http, "セーブに失敗しました．再試行します．")
                        .await?;
                    sleep(Duration::from_millis(1000)).await;
                } else {
                    msg.reply(&ctx.http, "セーブに失敗しました．").await?;
                }
            } else {
                msg.reply(&ctx.http, "セーブとバックアップを開始しました")
                    .await?;
                create_backup().await?;
                msg.reply(&ctx.http, output).await?;
                save_succeeded_flag = true;
                break;
            };
        }
        if save_succeeded_flag {
            msg.reply(&ctx.http, "シャットダウンを開始します").await?;
            let output = rcon("DoExit").await.expect("failed to run `rcon`");

            if output.is_empty() {
                msg.reply(&ctx.http, "シャットダウンが確認できませんでした．*/check_server*などのコマンドを使用してサーバーが正常終了しているかを確認してください．サーバーの起動は*/start_server*で行えます．").await?;
            } else {
                msg.reply(&ctx.http, output).await?;
                let raw_output = Command::new("powershell")
                    .arg(r#"scripts/start_ark_server.ps1"#)
                    .output()
                    .await
                    .expect("failed to start `check_connection`");
                let output = String::from_utf8_lossy(&raw_output.stdout);
                if output.is_empty() {
                    msg.reply(&ctx.http, "No output was returned").await?;
                } else {
                    msg.reply(&ctx.http, output).await?;
                }
            };
        } else {
            msg.reply(
                &ctx.http,
                "サーバーがコマンドを受け付けていません．シャットダウンを中断します．",
            )
            .await?;
        }
    } else {
        msg.reply(&ctx.http, "ゲームにプレイヤーが残っていたため，シャットダウンを中止しました．\n強制シャットダウンをする場合は*/shutdown force*を実行してください．").await?;
    }
    Ok(())
}

#[command]
#[description = "サーバーをセーブしてシャットダウンします"]
#[allowed_roles("ARK Server Admin")]
async fn shutdown_server(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    if num_listplayers().await == 0 || (!args.is_empty() && args.rest() == "force") {
        msg.reply(&ctx.http, "ゲームをセーブします").await?;
        let mut save_succeeded_flag = false;
        for i in 0..3 {
            let output = rcon("SaveWorld").await.expect("failed to run `rcon`");

            if output.is_empty() {
                if i != 2 {
                    msg.reply(&ctx.http, "セーブに失敗しました．再試行します．")
                        .await?;
                    sleep(Duration::from_millis(1000)).await;
                } else {
                    msg.reply(&ctx.http, "セーブに失敗しました．").await?;
                }
            } else {
                msg.reply(&ctx.http, "セーブとバックアップを開始しました")
                    .await?;
                create_backup().await?;
                msg.reply(&ctx.http, output).await?;
                save_succeeded_flag = true;
                break;
            };
        }
        if save_succeeded_flag {
            msg.reply(&ctx.http, "シャットダウンを開始します").await?;
            let output = rcon("DoExit").await.expect("failed to run `rcon`");

            if output.is_empty() {
                msg.reply(&ctx.http, "No output was returned").await?;
            } else {
                msg.reply(&ctx.http, output).await?;
            };
        } else {
            msg.reply(
                &ctx.http,
                "サーバーがコマンドを受け付けていません．シャットダウンを中断します．",
            )
            .await?;
        }
    } else {
        msg.reply(&ctx.http, "ゲームにプレイヤーが残っていたため，シャットダウンを中止しました．\n強制シャットダウンをする場合は*/shutdown force*を実行してください．").await?;
    }
    Ok(())
}

#[command]
#[description = "ARKサーバーが起動しているかを確認します"]
async fn check_server(ctx: &Context, msg: &Message) -> CommandResult {
    let output = rcon("listplayers").await;
    if output.is_err() || output.unwrap().is_empty() {
        msg.reply(&ctx.http, "ARKサーバーは動作停止中です").await?;
    } else {
        msg.reply(&ctx.http, "ARKサーバーは動作中です").await?;
    }
    Ok(())
}

#[command]
#[description = "ロールバック可能なバックアップリストを表示します"]
#[allowed_roles("ARK Server Admin")]
async fn listbackups(ctx: &Context, msg: &Message) -> CommandResult {
    let mut list: String =
        String::from("表記説明：\n`2022-12-21_(16-11-21).zip` 2022/12/21 16:11のバックアップ\n\n");
    let paths = std::fs::read_dir(BACKUP_DIR_PATH)?;
    for (i, path) in paths.into_iter().enumerate() {
        list.push_str(&format!(
            "{}: `{}`\n",
            i,
            path?
                .file_name()
                .to_str()
                .expect("failed to get file names")
        ));
        //msg.channel_id
        //   .say(&ctx.http, &path?.file_name().to_str().expect("failed to get file names"))
        //  .await?;
    }
    msg.reply(&ctx.http, list).await?;
    Ok(())
}

#[command]
#[description = "指定されたセーブデータを使ってロールバックします"]
#[allowed_roles("ARK Server Admin")]
async fn rollback(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    // サーバーが起動中かをチェック
    let output = rcon("listplayers").await;
    if output.is_err() || output.unwrap().is_empty() {
        // バックアップファイルが指定されているかをチェック
        if !args.is_empty() {
            // forceオプションの有無をチェック
            if !args.rest().contains("force") {
                msg.reply(&ctx.http, "ロールバックを開始します").await?;
                let zip_fullpath = format!(
                    "{}/{}.zip",
                    BACKUP_DIR_PATH,
                    args.rest()
                        .strip_prefix("force")
                        .expect("failed to get the zip fullpath")
                        .trim()
                );
                let fname = std::path::Path::new(&zip_fullpath);
                let file = File::open(fname).unwrap();

                let mut archive = zip::ZipArchive::new(file).unwrap();

                for i in 0..archive.len() {
                    let mut file = archive.by_index(i).expect("failed to open zip file");
                    let outpath = match file.enclosed_name() {
                        Some(path) => std::path::Path::new(ARK_SAVEDATA_PATH).join(path),
                        None => continue,
                    };

                    {
                        let comment = file.comment();
                        if !comment.is_empty() {
                            println!("File {} comment: {}", i, comment);
                        }
                    }

                    if (*file.name()).ends_with('/') {
                        println!("File {} extracted to \"{}\"", i, outpath.display());
                        std::fs::create_dir_all(&outpath).unwrap();
                    } else {
                        println!(
                            "File {} extracted to \"{}\" ({} bytes)",
                            i,
                            outpath.display(),
                            file.size()
                        );
                        if let Some(p) = outpath.parent() {
                            if !p.exists() {
                                std::fs::create_dir_all(p).unwrap();
                            }
                        }
                        let mut outfile = File::create(&outpath).unwrap();
                        copy(&mut file, &mut outfile).unwrap();
                    }
                }
                msg.reply(&ctx.http, "ロールバックを正常に終了しました")
                    .await?;
            } else {
                msg.reply(&ctx.http, "ロールバックを行うと現在のデータは失われます．確認のため*/rollback force ファイル名*を実行してください").await?;
            }
        } else {
            msg.reply(&ctx.http, "セーブデータ名を指定してください．利用可能なセーブデータは*/listbackups*で確認できます．").await?;
        }
    } else {
        msg.reply(
            &ctx.http,
            "ARKサーバーが動作中です．ロールバックを行う前にサーバーを停止してください．",
        )
        .await?;
    }
    Ok(())
}

async fn create_backup() -> zip::result::ZipResult<()> {
    println!("backup started");
    let date = chrono::Local::now()
        .format("%Y-%m-%d_(%H-%M-%S)")
        .to_string();
    let dest = format!("{}/{}.zip", BACKUP_DIR_PATH, date);
    let path = std::path::Path::new(&dest);
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path)?);
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Bzip2);

    let walkdir = WalkDir::new(ARK_SAVEDATA_PATH);
    let it = walkdir.into_iter().filter_map(|e| e.ok());

    let mut buffer = Vec::new();
    for entry in it {
        let path = entry.path();
        let name = path
            .strip_prefix(ARK_SAVEDATA_PATH)
            .unwrap()
            .to_str()
            .unwrap();
        if path.is_file()
            && (!(path
                .extension()
                .unwrap()
                .to_str()
                .expect("failed to get a file extension")
                .to_string()
                .contains("bak")
                || name != "Fjordur.ark"
                    && name.contains("Fjordur")
                    && path.extension().unwrap() == "ark"))
        {
            println!("Add: {}", name);
            zip.start_file(name, options)?;
            let mut f = File::open(path)?;

            f.read_to_end(&mut buffer)?;
            zip.write_all(&buffer)?;
            buffer.clear();
        } else if path.is_dir() {
            zip.add_directory(name, options)?;
        }
    }
    zip.finish()?;

    // if the num of file is greater than 10, delete the oldest backup
    if 10
        < std::fs::read_dir(BACKUP_DIR_PATH)
            .expect("failed to read the backup directory")
            .count()
    {
        let paths =
            std::fs::read_dir(BACKUP_DIR_PATH).expect("failed to read the backup directory");
        let mut old_path = std::path::PathBuf::from(BACKUP_DIR_PATH);
        let mut old_time = std::time::SystemTime::now();

        for result_path in paths {
            let entry = result_path.expect("failed to read a file (backup)");
            let metadata = std::fs::metadata(BACKUP_DIR_PATH)?;
            let created_time = metadata.created()?;

            if created_time < old_time {
                old_time = created_time;
                old_path = entry.path();
            }
        }
        if old_path != std::path::PathBuf::from(BACKUP_DIR_PATH) {
            std::fs::remove_file(&old_path)?;
            println!(
                "Deleted {}",
                old_path
                    .file_name()
                    .unwrap()
                    .to_str()
                    .expect("failed to get a file name")
            );
        }
    }
    println!("backup finished");
    Ok(())
}

#[command]
#[description = "オンラインのプレイヤーリストを表示します"]
#[allowed_roles("ARK Server Admin")]
async fn listplayers(ctx: &Context, msg: &Message) -> CommandResult {
    let output = rcon("listplayers").await.expect("failed to run `rcon`");
    if output.is_empty() {
        msg.reply(&ctx.http, "Failed to get the player list")
            .await?;
    } else {
        let mut v = vec![];
        for li in output.lines() {
            let splitted = li.split(',').collect::<Vec<&str>>();
            v.push(splitted[0]);
        }
        msg.reply(&ctx.http, v.join("\n")).await?;
    }
    Ok(())
}
