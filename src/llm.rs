use serde::{Deserialize, Serialize};

/// LLM client supporting Heimdall (local) and Gemini (fallback)
pub struct LlmClient {
    http: reqwest::Client,
    heimdall_url: String,
    gemini_api_key: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

/// Gemini API structures
#[derive(Debug, Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiPart {
    text: String,
}

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: GeminiContent,
}

impl LlmClient {
    pub fn new(http: reqwest::Client, heimdall_url: &str, gemini_api_key: &str) -> Self {
        Self {
            http,
            heimdall_url: heimdall_url.to_string(),
            gemini_api_key: gemini_api_key.to_string(),
        }
    }

    /// Send a chat completion request — tries Heimdall first, falls back to Gemini
    pub async fn chat(&self, system_prompt: &str, user_message: &str) -> Result<String, String> {
        // Try Heimdall first (local, faster)
        if !self.heimdall_url.is_empty() {
            match self.chat_heimdall(system_prompt, user_message).await {
                Ok(response) => return Ok(response),
                Err(e) => tracing::warn!("Heimdall unavailable, falling back to Gemini: {}", e),
            }
        }

        // Fallback to Gemini
        if !self.gemini_api_key.is_empty() {
            return self.chat_gemini(system_prompt, user_message).await;
        }

        Err("No LLM provider configured (set HEIMDALL_URL or GEMINI_API_KEY)".to_string())
    }

    async fn chat_heimdall(&self, system_prompt: &str, user_message: &str) -> Result<String, String> {
        let request = ChatRequest {
            model: "default".to_string(),
            messages: vec![
                ChatMessage { role: "system".to_string(), content: system_prompt.to_string() },
                ChatMessage { role: "user".to_string(), content: user_message.to_string() },
            ],
            temperature: 0.2,
            max_tokens: 4096,
        };

        let response = self.http
            .post(format!("{}/v1/chat/completions", self.heimdall_url))
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Heimdall request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Heimdall returned {}", response.status()));
        }

        let chat_response: ChatResponse = response.json().await
            .map_err(|e| format!("Heimdall response parse error: {}", e))?;

        chat_response.choices.first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| "Heimdall returned no response".to_string())
    }

    async fn chat_gemini(&self, system_prompt: &str, user_message: &str) -> Result<String, String> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={}",
            self.gemini_api_key
        );

        let request = GeminiRequest {
            contents: vec![GeminiContent {
                parts: vec![GeminiPart {
                    text: format!("{}\n\n{}", system_prompt, user_message),
                }],
            }],
        };

        let response = self.http
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Gemini request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Gemini returned {}", response.status()));
        }

        let gemini_response: GeminiResponse = response.json().await
            .map_err(|e| format!("Gemini response parse error: {}", e))?;

        gemini_response.candidates.first()
            .and_then(|c| c.content.parts.first())
            .map(|p| p.text.clone())
            .ok_or_else(|| "Gemini returned no response".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_request_serialization() {
        let req = ChatRequest {
            model: "test".to_string(),
            messages: vec![
                ChatMessage { role: "system".to_string(), content: "You are a security expert.".to_string() },
                ChatMessage { role: "user".to_string(), content: "Fix this SQL injection".to_string() },
            ],
            temperature: 0.2,
            max_tokens: 4096,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("security expert"));
        assert!(json.contains("SQL injection"));
    }

    #[test]
    fn test_chat_response_deserialization() {
        let json = r#"{
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Use parameterized queries."
                }
            }]
        }"#;
        let resp: ChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.choices[0].message.content, "Use parameterized queries.");
    }

    #[test]
    fn test_gemini_response_deserialization() {
        let json = r#"{
            "candidates": [{
                "content": {
                    "parts": [{"text": "Here is the fix:\n```python\ncursor.execute('SELECT * FROM users WHERE id = ?', (user_id,))\n```"}]
                }
            }]
        }"#;
        let resp: GeminiResponse = serde_json::from_str(json).unwrap();
        assert!(resp.candidates[0].content.parts[0].text.contains("fix"));
    }

    #[test]
    fn test_llm_client_no_provider() {
        let client = LlmClient::new(reqwest::Client::new(), "", "");
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(client.chat("system", "user"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No LLM provider"));
    }
}
