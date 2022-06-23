use std::env;

use lazy_regex::regex_replace_all;
use serenity::async_trait;
use serenity::http::client::Http;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::model::id::UserId;
use serenity::prelude::*;
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
        let echo = msg.author.id == self.user_id;
        let dm = msg.guild_id.is_none();
        let mention = msg.mentions.iter().any(|user| user.id == self.user_id);
        if !echo && (dm || mention) {
            let content = regex_replace_all!(
                r#"<@\d+>"#i,
                & msg.content,
                |_| String::new(),
            );
            let tokens = Tokenizer::new().tokenize(&content);
            let commands = self.yozuk.get_commands(&tokens, &[]);
            if commands.is_empty() {
                if let Err(why) = msg
                    .reply(&ctx.http, "Sorry, I can't understand your request.")
                    .await
                {
                    println!("Error sending message: {:?}", why);
                }
                return;
            }

            let result = self.yozuk.run_commands(commands, &mut [], None);
            let outputs = match result {
                Ok(outputs) => outputs,
                Err(outputs) => outputs,
            };

            for output in outputs {
                for block in output.blocks {
                    match block {
                        Block::Comment(comment) => {
                            if let Err(why) = msg.reply(&ctx.http, comment.text).await {
                                println!("Error sending message: {:?}", why);
                            }
                        }
                        Block::Data(data) => {
                            if let Ok(text) = str::from_utf8(&data.data) {
                                if let Err(why) = msg.reply(&ctx.http, text).await {
                                    println!("Error sending message: {:?}", why);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() {
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::DIRECT_MESSAGES;

    let http = Http::new(&token);
    let user = http.get_current_user().await.unwrap();
    let yozuk = Arc::new(Yozuk::builder().build());

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler {
            user_id: user.id,
            yozuk,
        })
        .await
        .expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
