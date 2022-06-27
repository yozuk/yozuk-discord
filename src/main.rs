use anyhow::Result;
use clap::Parser;
use futures::future::join_all;
use lazy_regex::regex_replace_all;
use mediatype::{media_type, MediaType};
use serenity::async_trait;
use serenity::http::client::Http;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::model::id::UserId;
use serenity::prelude::*;
use std::io::Cursor;
use std::str;
use std::sync::Arc;
use yozuk::Yozuk;
use yozuk_sdk::prelude::*;

struct Handler {
    user_id: UserId,
    yozuk: Arc<Yozuk>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if let Err(err) = handle_message(self, ctx, msg).await {
            println!("{err}");
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

async fn handle_message(handler: &Handler, ctx: Context, msg: Message) -> Result<()> {
    let echo = msg.author.id == handler.user_id;
    let dm = msg.guild_id.is_none();
    let mention = msg.mentions.iter().any(|user| user.id == handler.user_id);
    if !echo && (dm || mention) {
        let content = regex_replace_all!(
            r#"<@\d+>"#i,
            & msg.content,
            |_| String::new(),
        );

        let attachments = join_all(msg.attachments.iter().map(|att| att.download())).await;
        let mut attachments = attachments
            .into_iter()
            .zip(&msg.attachments)
            .filter_map(|(data, att)| data.ok().map(|data| (data, att.content_type.as_ref())))
            .map(|(data, content_type)| {
                InputStream::new(
                    Cursor::new(data),
                    content_type
                        .and_then(|ty| MediaType::parse(ty).ok())
                        .unwrap_or(media_type!(APPLICATION / OCTET_STREAM)),
                )
            })
            .collect::<Vec<_>>();

        let tokens = Tokenizer::new().tokenize(&content);
        let commands = handler.yozuk.get_commands(&tokens, &attachments);
        if commands.is_empty() {
            msg.reply(&ctx.http, "Sorry, I can't understand your request.")
                .await?;
            return Ok(());
        }

        let result = handler.yozuk.run_commands(commands, &mut attachments, None);
        let outputs = match result {
            Ok(outputs) => outputs,
            Err(outputs) => outputs,
        };

        for output in outputs {
            for block in output.blocks {
                match block {
                    Block::Comment(comment) => {
                        msg.reply(&ctx.http, comment.text).await?;
                    }
                    Block::Data(data) => {
                        if let Ok(text) = str::from_utf8(&data.data) {
                            msg.reply(&ctx.http, text).await?;
                        } else {
                            let files = vec![(data.data.as_ref(), data.file_name.as_str())];
                            msg.channel_id.send_files(&ctx.http, files, |m| m).await?;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

#[derive(Parser)]
#[clap(author, version, about)]
pub struct Args {
    #[clap(long, env("DISCORD_TOKEN"), hide_env_values = true)]
    pub token: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::try_parse()?;
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::DIRECT_MESSAGES;

    let http = Http::new(&args.token);
    let user = http.get_current_user().await?;
    let yozuk = Arc::new(Yozuk::builder().build());

    let mut client = Client::builder(&args.token, intents)
        .event_handler(Handler {
            user_id: user.id,
            yozuk,
        })
        .await?;

    client.start().await?;
    Ok(())
}
