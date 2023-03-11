use redis::{Client, Commands, RedisResult};

const LOCK_KEY: &str = "message_lock";

struct MessageProcessor {
    redis_client: Client,
}

impl MessageProcessor {
    fn new(redis_url: &str) -> MessageProcessor {
        let redis_client = Client::open(redis_url).unwrap();
        MessageProcessor { redis_client }
    }

    fn acquire_lock(&self, message_id: &str) -> RedisResult<bool> {
        let conn = &mut self.redis_client.get_connection()?;
        let result: bool = redis::cmd("SET")
            .arg(format!("{}_{}", LOCK_KEY, message_id))
            .arg("true")
            .arg("NX")
            .arg("EX")
            .arg(10)
            .query(conn)?;
        Ok(result)
    }

    fn release_lock(&self, message_id: &str) -> RedisResult<()> {
        let conn = &mut self.redis_client.get_connection()?;
        let _: () = redis::cmd("DEL")
            .arg(format!("{}_{}", LOCK_KEY, message_id))
            .query(conn)?;
        Ok(())
    }

    fn handle_message<F>(&self, message_id: &str, message: &str, process_fn: F) -> RedisResult<()>
    where
        F: FnOnce(&str) -> RedisResult<()> + Send,
    {
        // Try to acquire the lock
        if let Ok(true) = self.acquire_lock(message_id) {
            // We have the lock, so process the message
            process_fn(message)?;
            // Release the lock
            self.release_lock(message_id)?;
        } else {
            // Another instance has the lock, so skip processing the message
            println!("Skipping message: {}", message);
        }
        Ok(())
    }
}
