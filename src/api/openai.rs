use anyhow::Context;
use once_cell::sync::Lazy;
use reqwest::{
    header::HeaderMap,
    multipart::{Form, Part},
};
use serde::{Deserialize, Serialize};

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
        .build()
        .expect("failed to build HTTP client")
});

#[derive(Debug, Deserialize)]
struct TranscribeResp {
    text: String,
}

pub async fn transcribe(config: &OpenAiConfig, buf: Vec<u8>) -> anyhow::Result<String> {
    let form = Form::new()
        .part("file", Part::stream(buf).file_name("audio.wav"))
        .text("model", "whisper-1")
        .text("language", "en");
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
