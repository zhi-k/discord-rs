use redis::{pipe, Client, Commands, Connection, RedisResult};

const CONVERSATIONS_KEY: &str = "Conversations";

struct RedisConnection {
    conn: Connection,
}

impl RedisConnection {
    fn new(redis_url: &str) -> RedisConnection {
        let client = Client::open(redis_url).unwrap();
        RedisConnection {
            conn: client.get_connection().unwrap(),
        }
    }

    fn ensure_conversation_exist(&mut self, user_id: &str) -> RedisResult<()> {
        let conversation_key = format!("{}:{}", CONVERSATIONS_KEY, user_id);
        let key_exists: bool = redis::cmd("EXISTS")
            .arg(&conversation_key)
            .query(&mut self.conn)?;

        if !key_exists {
            self.conn.lpush(&conversation_key, "")?;
        }

        Ok(())
    }

    fn add_conversation(&mut self, user_id: &str, message: &str) -> RedisResult<()> {
        let conversation_key = format!("{}:{}", CONVERSATIONS_KEY, user_id);

        let mut transaction = pipe();
        transaction
            .atomic()
            .lpush(&conversation_key, message)
            .ltrim(&conversation_key, 0, 4);

        let _: () = transaction.query(&mut self.conn)?;

        Ok(())
    }

    fn get_conversations(&mut self, user_id: &str) -> redis::RedisResult<Vec<String>> {
        let conversation_key = format!("{}:{}", CONVERSATIONS_KEY, user_id);
        self.conn.lrange(&conversation_key, 0, 4)
    }

    fn clear_conversations(&mut self, user_id: &str) -> redis::RedisResult<()> {
        let conversation_key = format!("{}:{}", CONVERSATIONS_KEY, user_id);
        self.conn.del(&conversation_key)?;
        Ok(())
    }
}
