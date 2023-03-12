use std::time::Duration;

use crate::openai::request::Message;
use redis::{pipe, Client, Commands, Connection, RedisResult};

const CONVERSATIONS_KEY: &str = "Conversations";
const LOCK_KEY: &str = "message_lock";

pub fn get_conn(redis_client: &Client) -> Connection {
    redis_client
        .get_connection_with_timeout(Duration::from_secs(1))
        .expect("There should be a redis_client here.")
}

pub fn add_conversation(
    conn: &mut Connection,
    user_id: &str,
    messages: &Vec<Message>,
) -> RedisResult<()> {
    let conversation_key = format!("{}:{}", CONVERSATIONS_KEY, user_id);
    let redis_strings: Vec<String> = messages
        .iter()
        .map(|message| message.to_redis_string())
        .collect();

    let mut transaction = pipe();
    transaction
        .atomic()
        .lpush(&conversation_key, &redis_strings)
        .ltrim(&conversation_key, 0, 9);

    let _: () = transaction.query(conn)?;

    Ok(())
}

pub fn get_conversations(conn: &mut Connection, user_id: &str) -> redis::RedisResult<Vec<Message>> {
    let conversation_key = format!("{}:{}", CONVERSATIONS_KEY, user_id);
    let messages: Vec<String> = conn.lrange(&conversation_key, 0, 9)?;

    let mut conversation_messages: Vec<Message> = Vec::new();
    for message_str in messages.iter().rev() {
        if message_str.is_empty() {
            continue;
        }
        let message = Message::from_redis_string(message_str)?;
        conversation_messages.push(message);
    }

    Ok(conversation_messages)
}

pub fn clear_conversations(conn: &mut Connection, user_id: &str) -> redis::RedisResult<()> {
    let conversation_key = format!("{}:{}", CONVERSATIONS_KEY, user_id);
    conn.del(&conversation_key)?;
    Ok(())
}

pub fn acquire_lock(conn: &mut Connection, message_id: &str) -> RedisResult<bool> {
    let result: bool = redis::cmd("SET")
        .arg(format!("{}_{}", LOCK_KEY, message_id))
        .arg("true")
        .arg("NX")
        .arg("EX")
        .arg(10)
        .query(conn)?;
    Ok(result)
}

pub fn release_lock(conn: &mut Connection, message_id: &str) -> RedisResult<()> {
    let _: () = redis::cmd("DEL")
        .arg(format!("{}_{}", LOCK_KEY, message_id))
        .query(conn)?;
    Ok(())
}
