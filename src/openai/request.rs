use redis::RedisResult;
use reqwest::header::{HeaderMap, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const API_URL: &str = "https://api.openai.com/v1";
pub const PRIMER: &'static str = "You are a extremely helpful assistant in a Discord server.";

#[derive(Debug, Serialize, Deserialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: i32,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAIChoice {
    text: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn to_redis_string(&self) -> String {
        format!("{}:{}", self.role, self.content)
    }

    pub fn from_redis_string(redis_string: &str) -> RedisResult<Self> {
        let parts: Vec<&str> = redis_string.splitn(2, ':').collect();

        if parts.len() != 2 {
            return Err(redis::RedisError::from((
                redis::ErrorKind::TypeError,
                "Invalid message format",
            )));
        }
        let role = parts[0].parse().map_err(|_| {
            redis::RedisError::from((redis::ErrorKind::TypeError, "Invalid message ID"))
        })?;
        let content = parts[1].to_owned();
        Ok(Self { role, content })
    }
}

pub async fn generate_response(
    api_key: &str,
    prompt_input: Vec<Message>,
) -> Result<String, Box<dyn std::error::Error>> {
    let prompt = prompt_input;

    let request = OpenAIRequest {
        model: "gpt-3.5-turbo".to_owned(),
        messages: prompt,
        temperature: 0.8,
        max_tokens: 50,
    };

    let client = reqwest::Client::new();
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());
    headers.insert(
        AUTHORIZATION,
        format!("Bearer {}", api_key).parse().unwrap(),
    );

    let url = format!("{}/chat/completions", API_URL);

    let response = client
        .post(&url)
        .headers(headers)
        .json(&request)
        .send()
        .await?
        .json::<Value>()
        .await?;

    let message = response["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or(
            "I'm sorry, I couldn't generate a response. Please check the length of your question.",
        )
        .to_owned();

    Ok(message)
}
