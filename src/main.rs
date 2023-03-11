mod commands;
mod libs;

use serenity::async_trait;
use serenity::framework::standard::macros::group;
use serenity::framework::standard::StandardFramework;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use std::env;

use crate::commands::ping::*;

struct OpenaiKey;

impl TypeMapKey for OpenaiKey {
    type Value = String;
}

#[group]
#[commands(ping)]
struct General;

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
            return;
        }

        // Retrieve the API key from the context data
        let api_key = {
            let data = ctx.data.read().await;
            match data.get::<OpenaiKey>() {
                Some(key) => key.to_owned(),
                None => {
                    println!("OpenAI API key not found");
                    return;
                }
            }
        };

        let prompt = format!("{} ", msg.content);

        if prompt.len() > 2048 {
            if let Err(why) = msg
                .channel_id
                .say(
                    &ctx.http,
                    "Error: prompt length exceeds maximum of 2048 tokens",
                )
                .await
            {
                println!("Error sending message: {:?}", why);
            }
            return;
        }

        // Generate a text response using the OpenAI API
        let response = match reqwest::Client::new()
            .post("https://api.openai.com/v1/chat/completions")
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&serde_json::json!({
                "model": "gpt-3.5-turbo",
                "messages": [
                    {"role": "system", "content": "You are a helpful assistant."},
                    {"role": "user", "content": prompt}
                ]
            }))
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                println!("Error sending request to OpenAI API: {:?}", e);
                return;
            }
        };

        let response_json = match response.json::<serde_json::Value>().await {
            Ok(json) => json,
            Err(e) => {
                println!("Error parsing response from OpenAI API: {:?}", e);
                return;
            }
        };

        let message = response_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("I'm sorry, I couldn't generate a response.");

        // Send the response back to the channel
        if let Err(why) = msg.channel_id.say(&ctx.http, message).await {
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
        data.insert::<OpenaiKey>(openai_key);
    }

    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }

    Ok(())
}
