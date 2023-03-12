mod commands;
mod libs;
mod openai;

use libs::redis::release_lock;
use serenity::async_trait;
use serenity::framework::standard::macros::group;
use serenity::framework::standard::StandardFramework;
use serenity::model::prelude::Message;
use serenity::model::prelude::Ready;
use serenity::prelude::*;
use std::env;
use std::sync::Arc;

use crate::commands::clear::*;
use crate::commands::ping::*;
use crate::libs::redis::{acquire_lock, add_conversation, get_conn, get_conversations};
use crate::openai::request::{generate_response, Message as RequestMessage, PRIMER};

#[derive(Debug)]
struct RedisClient {}
impl TypeMapKey for RedisClient {
    type Value = Arc<redis::Client>;
}

#[derive(Debug)]
struct OpenAiKey {}
impl TypeMapKey for OpenAiKey {
    type Value = String;
}

#[group]
#[commands(ping, clear)]
struct General;

fn remove_mentions(msg: String) -> String {
    let mut message = msg;
    while let Some(start) = message.find("<@") {
        match message[start..].find('>') {
            Some(end) => {
                message.replace_range(start..start + end + 1, "");
            }
            None => break,
        }
    }
    message.trim().to_string()
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("Connected as {}", ready.user.name);
    }

    async fn message(&self, ctx: Context, msg: Message) {
        // Check if the bot was mentioned in the message
        let bot_id = ctx.cache.current_user_id();
        let mentioned = msg.mentions_user_id(bot_id);

        if !mentioned {
            return ();
        }

        let user_id = msg.author.id;
        let user_mention = msg.author.mention();
        let message_id = msg.id;

        let data = ctx.data.read().await;
        let api_key = data
            .get::<OpenAiKey>()
            .expect("There should be api_key here.");
        let redis_client = data
            .get::<RedisClient>()
            .expect("There should be redis_client here.");

        let mut redis_conn = get_conn(&mut &redis_client);

        let lock = acquire_lock(&mut redis_conn, message_id.to_string().as_str()).unwrap();

        // cant acquire lock means some other instance is already processing
        if !lock {
            return ();
        }

        let conversations = get_conversations(&mut redis_conn, user_id.to_string().as_str())
            .expect("Get conversations here.");

        let mut reversed_conversations = conversations.clone();
        if reversed_conversations.len() == 0 {
            reversed_conversations.push(RequestMessage {
                content: PRIMER.to_string(),
                role: "user".to_string(),
            });
        }

        let prompt = format!("{}", remove_mentions(msg.content.clone()));
        if prompt.len() > 2048 {
            if let Err(why) = msg
                .channel_id
                .say(
                    &ctx.http,
                    format!(
                        "Error: {}, your prompt length exceeds the maximum of 2048 tokens",
                        user_mention
                    ),
                )
                .await
            {
                println!("Error sending message: {:?}", why);
            }
            if let Err(why) = release_lock(&mut redis_conn, message_id.to_string().as_str()) {
                println!("Unable to release lock, {}", why);
            };
            return;
        }

        reversed_conversations.push(RequestMessage {
            content: prompt.clone(),
            role: "user".to_string(),
        });

        let message = generate_response(api_key.as_str(), reversed_conversations.clone())
            .await
            .unwrap();

        reversed_conversations.push(RequestMessage {
            role: "assistant".to_owned(),
            content: message.clone(),
        });

        let assistant_result = add_conversation(
            &mut redis_conn,
            user_id.to_string().as_str(),
            &reversed_conversations,
        );

        match assistant_result {
            Ok(()) => {}
            Err(why) => {
                eprintln!("Error add_conversation user: {}", why);
                if let Err(why) = release_lock(&mut redis_conn, message_id.to_string().as_str()) {
                    println!("Unable to release lock, {}", why);
                };
                return ();
            }
        }

        if let Err(why) = release_lock(&mut redis_conn, message_id.to_string().as_str()) {
            println!("Unable to release lock, {}", why);
        };

        // Send the response back to the channel
        if let Err(why) = msg.reply(&ctx.http, message).await {
            println!("Error sending message: {:?}", why);
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let token =
        env::var("DISCORD_TOKEN").map_err(|_| "DISCORD_TOKEN environment variable not set")?;
    let openai_key =
        env::var("OPENAI_API_KEY").map_err(|_| "OPENAI_API_KEY environment variable not set")?;
    let redis_url = env::var("REDIS_URL").map_err(|_| "REDIS_URL environment variable not set")?;

    let framework = StandardFramework::new()
        .configure(|c| c.prefix("!"))
        .group(&GENERAL_GROUP);

    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(token, intents)
        .event_handler(Handler)
        .framework(framework)
        .await?;

    {
        let mut data = client.data.write().await;
        data.insert::<OpenAiKey>(openai_key);

        match redis::Client::open(redis_url) {
            redis::RedisResult::Ok(client) => {
                println!("Redis client created");
                data.insert::<RedisClient>(Arc::new(client));
            }
            redis::RedisResult::Err(error) => {
                println!("Error connecting to Redis: {}", error);
            }
        }
    }

    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }

    Ok(())
}
