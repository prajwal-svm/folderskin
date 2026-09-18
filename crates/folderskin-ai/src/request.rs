//! Turning a [`GenerateRequest`] into each provider's request body, and reading its reply.
//!
//! Everything here is pure so it can be tested without a network: the body builders return
//! `serde_json::Value` (or a description of the multipart parts) and the readers take bytes.

use crate::{AiError, GenerateRequest};
use base64::Engine;
use serde_json::{json, Value};

/// Width and height from a "1024x1024" string, falling back to the model's first size.
pub fn dimensions(req: &GenerateRequest, default: &str) -> (u32, u32) {
    let s = req.size.as_deref().unwrap_or(default);
    let parsed = s
        .split_once('x')
        .and_then(|(w, h)| Some((w.trim().parse().ok()?, h.trim().parse().ok()?)));
    parsed.unwrap_or((1024, 1024))
}

pub fn size_string(req: &GenerateRequest, default: &str) -> String {
    let (w, h) = dimensions(req, default);
    format!("{w}x{h}")
}

// ---------------------------------------------------------------- request bodies

pub fn openai_body(req: &GenerateRequest, default_size: &str) -> Value {
    let mut body = json!({
        "model": req.model,
        "prompt": req.prompt,
        "n": 1,
        "size": size_string(req, default_size),
        "output_format": "png",
    });
    if req.want_alpha {
        body["background"] = json!("transparent");
    }
    body
}

pub fn xai_body(req: &GenerateRequest) -> Value {
    json!({ "model": req.model, "prompt": req.prompt, "n": 1, "response_format": "b64_json" })
}

pub fn recraft_body(req: &GenerateRequest, default_size: &str) -> Value {
    let mut body = json!({
        "model": req.model,
        "prompt": req.prompt,
        "size": size_string(req, default_size),
        "response_format": "b64_json",
    });
    if req.want_alpha {
        // Recraft returns a cut-out subject when asked for the transparent style.
        body["style"] = json!("digital_illustration");
        body["substyle"] = json!("2d_art_poster");
        body["transparent_background"] = json!(true);
    }
    body
}

pub fn google_body(req: &GenerateRequest) -> Value {
    let mut parts = vec![json!({ "text": req.prompt })];
    if let Some(png) = &req.reference_png {
        parts.push(json!({
            "inline_data": {
                "mime_type": "image/png",
                "data": base64::engine::general_purpose::STANDARD.encode(png),
            }
        }));
    }
    json!({ "contents": [{ "parts": parts }] })
}

pub fn bfl_body(req: &GenerateRequest, default_size: &str) -> Value {
    let (w, h) = dimensions(req, default_size);
    json!({ "prompt": req.prompt, "width": w, "height": h, "output_format": "png", "safety_tolerance": 2 })
}

pub fn ideogram_body(req: &GenerateRequest, default_size: &str) -> Value {
    json!({
        "prompt": req.prompt,
        "rendering_speed": "DEFAULT",
        "num_images": 1,
        "resolution": size_string(req, default_size).to_uppercase().replace('X', "x"),
    })
}

/// Stability takes multipart form fields rather than JSON.
pub fn stability_fields(req: &GenerateRequest) -> Vec<(&'static str, String)> {
    vec![
        ("prompt", req.prompt.clone()),
        ("output_format", "png".into()),
        ("aspect_ratio", "1:1".into()),
        ("mode", "text-to-image".into()),
    ]
}

// ---------------------------------------------------------------- reading replies

/// `data[0].b64_json` (OpenAI, xAI, Recraft) or `data[0].url` when the provider returns a link.
pub enum Payload {
    Bytes(Vec<u8>),
    Url(String),
}

pub fn read_data_array(provider: &str, body: &Value) -> Result<(Payload, Option<String>), AiError> {
    let first = body
        .get("data")
        .and_then(|d| d.as_array())
        .and_then(|a| a.first())
        .ok_or_else(|| AiError::Decode(format!("{provider} returned no image")))?;
    let revised = first
        .get("revised_prompt")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    if let Some(b64) = first.get("b64_json").and_then(|v| v.as_str()) {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .map_err(|_| {
                AiError::Decode(format!(
                    "{provider} returned an image FolderSkin could not read"
                ))
            })?;
        return Ok((Payload::Bytes(bytes), revised));
    }
    if let Some(url) = first.get("url").and_then(|v| v.as_str()) {
        return Ok((Payload::Url(url.to_string()), revised));
    }
    Err(AiError::Decode(format!("{provider} returned no image")))
}

/// Gemini puts the image in `candidates[0].content.parts[].inline_data.data`.
pub fn read_google(body: &Value) -> Result<Vec<u8>, AiError> {
    let parts = body
        .get("candidates")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.as_array())
        .ok_or_else(|| AiError::Decode("Gemini returned no image".into()))?;
    for part in parts {
        let data = part
            .get("inline_data")
            .or_else(|| part.get("inlineData"))
            .and_then(|d| d.get("data"))
            .and_then(|d| d.as_str());
        if let Some(b64) = data {
            return base64::engine::general_purpose::STANDARD
                .decode(b64)
                .map_err(|_| {
                    AiError::Decode("Gemini returned an image FolderSkin could not read".into())
                });
        }
    }
    Err(AiError::Decode(
        "Gemini replied with text instead of an image. Try rewording the prompt.".into(),
    ))
}

/// Ideogram returns `data[0].url`.
pub fn read_ideogram(body: &Value) -> Result<String, AiError> {
    body.get("data")
        .and_then(|d| d.as_array())
        .and_then(|a| a.first())
        .and_then(|i| i.get("url"))
        .and_then(|u| u.as_str())
        .map(str::to_string)
        .ok_or_else(|| AiError::Decode("Ideogram returned no image".into()))
}

/// BFL's submit call returns a polling URL; the poll returns `status` and eventually a sample URL.
pub fn read_bfl_submit(body: &Value) -> Result<String, AiError> {
    body.get("polling_url")
        .or_else(|| body.get("pollingUrl"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| {
            AiError::Decode("Black Forest Labs did not say where to collect the image".into())
        })
}

pub enum BflPoll {
    Pending,
    Ready(String),
    Failed(String),
}

pub fn read_bfl_poll(body: &Value) -> BflPoll {
    match body
        .get("status")
        .and_then(|s| s.as_str())
        .unwrap_or_default()
    {
        "Ready" | "ready" => match body
            .get("result")
            .and_then(|r| r.get("sample"))
            .and_then(|s| s.as_str())
        {
            Some(url) => BflPoll::Ready(url.to_string()),
            None => BflPoll::Failed("the finished image had no download link".into()),
        },
        "Error" | "Failed" | "error" | "failed" => BflPoll::Failed(
            body.get("details")
                .and_then(|d| d.as_str())
                .map(crate::error::trim_message)
                .unwrap_or_else(|| "the request failed".into()),
        ),
        "Content Moderated" | "Request Moderated" => {
            BflPoll::Failed("the prompt or the result was blocked by the provider's filter".into())
        }
        _ => BflPoll::Pending,
    }
}

/// Pulls a human-readable message out of whatever error shape a provider uses.
pub fn error_message(body: &str) -> String {
    let Ok(v) = serde_json::from_str::<Value>(body) else {
        return crate::error::trim_message(body);
    };
    for path in [
        vec!["error", "message"],
        vec!["error"],
        vec!["message"],
        vec!["detail"],
        vec!["errors", "0"],
        vec!["name"],
    ] {
        let mut node = &v;
        let mut ok = true;
        for key in &path {
            node = match key.parse::<usize>() {
                Ok(i) => match node.get(i) {
                    Some(n) => n,
                    None => {
                        ok = false;
                        break;
                    }
                },
                Err(_) => match node.get(key) {
                    Some(n) => n,
                    None => {
                        ok = false;
                        break;
                    }
                },
            };
        }
        if ok {
            if let Some(s) = node.as_str() {
                return crate::error::trim_message(s);
            }
        }
    }
    crate::error::trim_message(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req() -> GenerateRequest {
        GenerateRequest {
            provider: "openai".into(),
            model: "gpt-image-2.5-flare".into(),
            prompt: "a copper patina".into(),
            reference_png: None,
            size: Some("1536x1024".into()),
            want_alpha: false,
        }
    }

    #[test]
    fn sizes_parse_and_fall_back() {
        assert_eq!(dimensions(&req(), "1024x1024"), (1536, 1024));
        let mut r = req();
        r.size = None;
        assert_eq!(dimensions(&r, "1024x958"), (1024, 958));
        r.size = Some("nonsense".into());
        assert_eq!(dimensions(&r, "1024x1024"), (1024, 1024));
    }

    #[test]
    fn openai_only_asks_for_transparency_when_it_is_wanted() {
        let plain = openai_body(&req(), "1024x1024");
        assert!(plain.get("background").is_none());
        assert_eq!(plain["output_format"], "png");
        assert_eq!(plain["size"], "1536x1024");
        let mut r = req();
        r.want_alpha = true;
        assert_eq!(openai_body(&r, "1024x1024")["background"], "transparent");
    }

    #[test]
    fn recraft_sets_its_transparency_flag() {
        let mut r = req();
        r.want_alpha = true;
        let body = recraft_body(&r, "1024x1024");
        assert_eq!(body["transparent_background"], true);
        assert!(recraft_body(&req(), "1024x1024")
            .get("transparent_background")
            .is_none());
    }

    #[test]
    fn google_attaches_a_reference_picture_as_inline_data() {
        let mut r = req();
        r.reference_png = Some(vec![1, 2, 3]);
        let body = google_body(&r);
        let parts = body["contents"][0]["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[1]["inline_data"]["mime_type"], "image/png");
        assert_eq!(
            google_body(&req())["contents"][0]["parts"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn bfl_sends_pixel_dimensions() {
        let body = bfl_body(&req(), "1024x1024");
        assert_eq!(body["width"], 1536);
        assert_eq!(body["height"], 1024);
    }

    #[test]
    fn reading_a_data_array_handles_bytes_urls_and_nothing() {
        let b64 = base64::engine::general_purpose::STANDARD.encode([9u8, 9, 9]);
        let body = json!({ "data": [{ "b64_json": b64, "revised_prompt": "tweaked" }] });
        let (payload, revised) = read_data_array("OpenAI", &body).unwrap();
        assert!(matches!(payload, Payload::Bytes(b) if b == vec![9, 9, 9]));
        assert_eq!(revised.as_deref(), Some("tweaked"));

        let url = json!({ "data": [{ "url": "https://example.test/a.png" }] });
        assert!(
            matches!(read_data_array("xAI", &url).unwrap().0, Payload::Url(u) if u.ends_with("a.png"))
        );

        assert!(read_data_array("xAI", &json!({ "data": [] })).is_err());
    }

    #[test]
    fn google_replies_are_read_in_both_key_spellings() {
        let b64 = base64::engine::general_purpose::STANDARD.encode([7u8, 7]);
        let snake = json!({ "candidates": [{ "content": { "parts": [{ "inline_data": { "data": b64.clone() } }] } }] });
        assert_eq!(read_google(&snake).unwrap(), vec![7, 7]);
        let camel = json!({ "candidates": [{ "content": { "parts": [{ "inlineData": { "data": b64 } }] } }] });
        assert_eq!(read_google(&camel).unwrap(), vec![7, 7]);
        let text_only =
            json!({ "candidates": [{ "content": { "parts": [{ "text": "I can't" }] } }] });
        assert!(read_google(&text_only).is_err());
    }

    #[test]
    fn bfl_polling_states_are_recognised() {
        assert!(matches!(
            read_bfl_poll(&json!({ "status": "Pending" })),
            BflPoll::Pending
        ));
        assert!(matches!(
            read_bfl_poll(&json!({ "status": "Ready", "result": { "sample": "https://x.test/i.png" } })),
            BflPoll::Ready(u) if u.ends_with("i.png")
        ));
        assert!(
            matches!(read_bfl_poll(&json!({ "status": "Error", "details": "boom" })), BflPoll::Failed(m) if m == "boom")
        );
        assert!(matches!(
            read_bfl_poll(&json!({ "status": "Content Moderated" })),
            BflPoll::Failed(_)
        ));
        assert!(
            matches!(read_bfl_submit(&json!({ "polling_url": "https://x.test/p" })).unwrap(), u if u.ends_with("/p"))
        );
    }

    #[test]
    fn error_messages_are_pulled_from_each_provider_shape() {
        assert_eq!(
            error_message(r#"{"error":{"message":"bad key"}}"#),
            "bad key"
        );
        assert_eq!(error_message(r#"{"error":"nope"}"#), "nope");
        assert_eq!(
            error_message(r#"{"detail":"missing field"}"#),
            "missing field"
        );
        assert_eq!(error_message("plain text"), "plain text");
    }

    #[test]
    fn stability_fields_carry_the_prompt_and_format() {
        let fields = stability_fields(&req());
        assert!(fields
            .iter()
            .any(|(k, v)| *k == "prompt" && v == "a copper patina"));
        assert!(fields
            .iter()
            .any(|(k, v)| *k == "output_format" && v == "png"));
    }
}
