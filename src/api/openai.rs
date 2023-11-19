use std::time::Duration;

use anyhow::Context;
use itertools::Itertools;
use once_cell::sync::Lazy;
use reqwest::{
    header::HeaderMap,
    multipart::{Form, Part},
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::config::OpenAiConfig;

static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    let mut headers = HeaderMap::new();
    headers.insert(
        "user-agent",
        concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"))
            .parse()
            .expect("failed to parse header value"),
    );
    reqwest::Client::builder()
        .default_headers(headers)
        .timeout(Duration::from_secs(5))
        .build()
        .expect("failed to build HTTP client")
});

#[derive(Debug, Deserialize)]
struct TranscribeResp {
    text: String,
}

pub async fn transcribe(
    config: &OpenAiConfig,
    self_callsign: &str,
    callsigns: &[String],
    buf: Vec<u8>,
) -> anyhow::Result<String> {
    let form = Form::new()
        .part("file", Part::stream(buf).file_name("audio.wav"))
        .text("model", "whisper-1")
        .text("language", "en").text("prompt", format!(r#"Your callsign is {}. You are a military AWACS controller. You are going to listen a pilot's transmission.

Transmission usually looks like:

{{to callsign}}, {{from callsign}}, {{intent}}

Possible intents are:
- radio check
- request bogey dope

Possible callsigns are:

- {}
{}
"#,
    self_callsign,
    self_callsign,
    callsigns.iter().map(|callsign| format!("- {callsign}")).join("\n"),
));
    let resp = HTTP_CLIENT
        .post("https://api.openai.com/v1/audio/transcriptions")
        .bearer_auth(&config.api_key)
        .multipart(form)
        .send()
        .await
        .context("failed to request to OpenAI API")?
        .text()
        .await
        .context("failed to read from OpenAI API response")?;
    let resp = serde_json::from_str::<TranscribeResp>(&resp)
        .with_context(|| format!("failed to parse OpenAI API response: {}", resp))?;
    Ok(resp.text)
}

#[derive(Deserialize, Serialize)]
struct ChatCompletionMessage {
    content: String,
    role: String,
}

#[derive(Serialize)]
struct ChatCompletionReqResponseFormat {
    #[serde(rename = "type")]
    ty: &'static str,
}

#[derive(Serialize)]
struct ChatCompletionReq {
    messages: Vec<ChatCompletionMessage>,
    model: &'static str,
    max_tokens: usize,
    response_format: ChatCompletionReqResponseFormat,
    temperature: f64,
}

#[derive(Deserialize)]
struct ChatCompletionRespChoice {
    message: ChatCompletionMessage,
}

#[derive(Deserialize)]
struct ChatCompletionResp {
    choices: Vec<ChatCompletionRespChoice>,
}

pub async fn parse_transmission<T: DeserializeOwned>(
    config: &OpenAiConfig,
    self_callsign: &str,
    transmission: String,
) -> anyhow::Result<T> {
    let req = ChatCompletionReq {
        messages: vec![
            ChatCompletionMessage {
                content: format!(
                    r#"Your callsign is {}. You are a military AWACS controller. Parse the pilot's transmission to JSON.

Possible intents are:
- radio_check
- request_bogey_dope
- unknown

Input usually looks like:
{{to callsign}}, {{from callsign}}, {{intent}}

Output must be all lowercased and looks like:

{{
  "to_callsign": "{{to callsign}}",
  "from_callsign": "{{from callsign}}",
  "intent: "{{intent}}"
}}
"#,
                    self_callsign
                ),
                role: "system".to_string(),
            },
            ChatCompletionMessage {
                content: transmission,
                role: "user".to_string(),
            },
        ],
        model: "gpt-3.5-turbo-1106",
        max_tokens: 100,
        response_format: ChatCompletionReqResponseFormat { ty: "json_object" },
        temperature: 0.,
    };
    let resp_str = HTTP_CLIENT
        .post("https://api.openai.com/v1/chat/completions")
        .bearer_auth(&config.api_key)
        .json(&req)
        .send()
        .await
        .context("failed to request to OpenAI API")?
        .text()
        .await
        .context("failed to read from OpenAI API response")?;
    let resp = serde_json::from_str::<ChatCompletionResp>(&resp_str)
        .with_context(|| format!("failed to parse OpenAI API response: {}", resp_str))?;
    let choice = resp
        .choices
        .first()
        .with_context(|| format!("OpenAI returned empty choices, raw response: {}", resp_str))?;
    serde_json::from_str::<T>(&choice.message.content)
        .with_context(|| format!("failed to parse OpenAI API response: {}", resp_str))
}

#[derive(Debug, Serialize)]
struct SpeechReq<'a> {
    model: &'static str,
    input: &'a str,
    voice: &'a str,
    response_format: &'static str,
    speed: f64,
}

pub async fn speech(config: &OpenAiConfig, input: &str) -> anyhow::Result<Vec<u8>> {
    let req = SpeechReq {
        model: "tts-1",
        input,
        voice: &config.speech_voice,
        response_format: "opus",
        speed: config.speech_speed,
    };
    let resp = HTTP_CLIENT
        .post("https://api.openai.com/v1/audio/speech")
        .bearer_auth(&config.api_key)
        .json(&req)
        .send()
        .await
        .context("failed to request to OpenAI API")?
        .bytes()
        .await
        .context("failed to read from OpenAI API response")?;
    Ok(resp.to_vec())
}
