use serenity::framework::standard::macros::command;
use serenity::framework::standard::CommandResult;
use serenity::model::channel::Message;
use serenity::prelude::Context;

use crate::libs::redis::{clear_conversations, get_conn};
use crate::RedisClient;

#[command]
async fn clear(ctx: &Context, msg: &Message) -> CommandResult {
    let user_id = msg.author.id;

    let data = ctx.data.read().await;
    let redis_client = data
        .get::<RedisClient>()
        .expect("There is a redis_client here.");
    let mut redis_conn = get_conn(redis_client);
    if let Err(why) = clear_conversations(&mut redis_conn, user_id.to_string().as_str()) {
        println!("Error clearing conversation, {}", why);
    };

    msg.reply(ctx, "Conversation cleared. You can now start a new topic!")
        .await?;

    Ok(())
}
