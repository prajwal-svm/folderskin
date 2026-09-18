//! Bring-your-own-key image generation for FolderSkin.
//!
//! The user pastes a key from a provider they already have an account with; it is stored in the
//! operating system's keychain by [`keys`], read at the moment of a request, and never written
//! anywhere else. There is no FolderSkin server: every call goes straight from the user's
//! machine to the provider they chose.
//!
//! [`catalogue`] lists what is on offer, [`request`] builds each provider's body and reads its
//! reply (pure, so it is all unit-tested), [`prompts`] composes the prompt, and [`generate`]
//! performs the one HTTP call.

pub mod catalogue;
pub mod error;
pub mod keys;
pub mod prompts;
pub mod request;

pub use catalogue::{model, provider, providers, ModelInfo, ProviderInfo};
pub use error::AiError;

use request::{BflPoll, Payload};
use std::sync::OnceLock;
use std::time::Duration;

/// What the caller wants generated. `prompt` is already composed; providers get it verbatim.
#[derive(Clone, Debug)]
pub struct GenerateRequest {
    pub provider: String,
    pub model: String,
    pub prompt: String,
    /// A picture to work from, for models that accept one.
    pub reference_png: Option<Vec<u8>>,
    /// "1024x1024"; `None` uses the model's first listed size.
    pub size: Option<String>,
    /// Ask for a transparent background. Only set for models that really support it.
    pub want_alpha: bool,
}

/// One generated image.
#[derive(Clone, Debug)]
pub struct GenerateResult {
    pub image: Vec<u8>,
    pub media_type: String,
    /// True when the bytes really carry a transparent background.
    pub native_alpha: bool,
    pub model_used: String,
    pub revised_prompt: Option<String>,
}

/// Generation can take a while; a minute and a half is generous for every provider here.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(90);
/// Black Forest Labs is asynchronous: submit, then poll.
const POLL_INTERVAL: Duration = Duration::from_secs(1);
const POLL_LIMIT: u32 = 90;

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent(concat!("folderskin/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("a default reqwest client always builds")
    })
}

fn net(provider: &str, e: reqwest::Error) -> AiError {
    if e.is_timeout() {
        AiError::Timeout {
            provider: provider.to_string(),
        }
    } else {
        AiError::Network {
            provider: provider.to_string(),
            detail: error::trim_message(&e.to_string()),
        }
    }
}

/// Reads a response, turning any non-success status into a friendly error.
async fn json_or_error(
    provider: &str,
    res: reqwest::Response,
) -> Result<serde_json::Value, AiError> {
    let status = res.status();
    let text = res.text().await.map_err(|e| net(provider, e))?;
    if !status.is_success() {
        return Err(AiError::from_status(
            provider,
            status.as_u16(),
            request::error_message(&text),
        ));
    }
    serde_json::from_str(&text).map_err(|_| {
        AiError::Decode(format!(
            "{provider} replied with something FolderSkin could not read"
        ))
    })
}

/// Downloads an image the provider left at a URL.
async fn fetch_image(provider: &str, url: &str) -> Result<(Vec<u8>, String), AiError> {
    let res = client()
        .get(url)
        .send()
        .await
        .map_err(|e| net(provider, e))?;
    let status = res.status();
    if !status.is_success() {
        return Err(AiError::Provider {
            provider: provider.to_string(),
            status: status.as_u16(),
            message: "the image link the provider gave could not be downloaded".into(),
        });
    }
    let media = res
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/png")
        .split(';')
        .next()
        .unwrap_or("image/png")
        .to_string();
    let bytes = res.bytes().await.map_err(|e| net(provider, e))?;
    Ok((bytes.to_vec(), media))
}

/// Generates one image with the user's key.
pub async fn generate(req: &GenerateRequest, api_key: &str) -> Result<GenerateResult, AiError> {
    let provider =
        provider(&req.provider).ok_or_else(|| AiError::UnknownProvider(req.provider.clone()))?;
    let model = model(&req.provider, &req.model).ok_or_else(|| AiError::UnknownModel {
        provider: req.provider.clone(),
        model: req.model.clone(),
    })?;
    if api_key.trim().is_empty() {
        return Err(AiError::MissingKey(provider.label.to_string()));
    }
    if req.reference_png.is_some() && !model.accepts_reference {
        return Err(AiError::Unsupported(format!(
            "{} cannot work from a reference picture. Clear it, or pick another model.",
            model.label
        )));
    }
    let default_size = model.sizes[0];
    let label = provider.label;

    let (image, media_type, revised) = match provider.id {
        "openai" => openai(req, api_key, default_size, label).await?,
        "xai" => xai(req, api_key, label).await?,
        "recraft" => recraft(req, api_key, default_size, label).await?,
        "google" => google(req, api_key, label).await?,
        "bfl" => bfl(req, api_key, default_size, label).await?,
        "stability" => stability(req, api_key, label).await?,
        "ideogram" => ideogram(req, api_key, default_size, label).await?,
        other => return Err(AiError::UnknownProvider(other.to_string())),
    };

    Ok(GenerateResult {
        image,
        media_type,
        native_alpha: req.want_alpha && model.native_alpha,
        model_used: model.id.to_string(),
        revised_prompt: revised,
    })
}

/// A cheap authenticated call that proves the key works without generating anything.
pub async fn test_key(provider_id: &str, api_key: &str) -> Result<(), AiError> {
    let provider =
        provider(provider_id).ok_or_else(|| AiError::UnknownProvider(provider_id.to_string()))?;
    if api_key.trim().is_empty() {
        return Err(AiError::MissingKey(provider.label.to_string()));
    }
    let label = provider.label;
    let (url, req) = match provider.id {
        "openai" => (
            "https://api.openai.com/v1/models",
            client()
                .get("https://api.openai.com/v1/models")
                .bearer_auth(api_key),
        ),
        "xai" => (
            "https://api.x.ai/v1/models",
            client()
                .get("https://api.x.ai/v1/models")
                .bearer_auth(api_key),
        ),
        "recraft" => (
            "https://external.api.recraft.ai/v1/users/me",
            client()
                .get("https://external.api.recraft.ai/v1/users/me")
                .bearer_auth(api_key),
        ),
        "google" => (
            "https://generativelanguage.googleapis.com/v1beta/models",
            client()
                .get("https://generativelanguage.googleapis.com/v1beta/models")
                .header("x-goog-api-key", api_key),
        ),
        "stability" => (
            "https://api.stability.ai/v1/user/account",
            client()
                .get("https://api.stability.ai/v1/user/account")
                .bearer_auth(api_key),
        ),
        // These two have no free "who am I" endpoint, so the key is checked on first use.
        "bfl" | "ideogram" => return Ok(()),
        other => return Err(AiError::UnknownProvider(other.to_string())),
    };
    let _ = url;
    let res = req.send().await.map_err(|e| net(label, e))?;
    let status = res.status();
    if status.is_success() {
        return Ok(());
    }
    let text = res.text().await.unwrap_or_default();
    Err(AiError::from_status(
        label,
        status.as_u16(),
        request::error_message(&text),
    ))
}

// ---------------------------------------------------------------- per provider

type Generated = (Vec<u8>, String, Option<String>);

async fn openai(
    req: &GenerateRequest,
    key: &str,
    default_size: &str,
    label: &str,
) -> Result<Generated, AiError> {
    let res = match &req.reference_png {
        // An edit takes the reference picture as a file part.
        Some(png) => {
            let part = reqwest::multipart::Part::bytes(png.clone())
                .file_name("reference.png")
                .mime_str("image/png")
                .map_err(|e| AiError::Decode(e.to_string()))?;
            let mut form = reqwest::multipart::Form::new()
                .text("model", req.model.clone())
                .text("prompt", req.prompt.clone())
                .text("n", "1")
                .text("size", request::size_string(req, default_size))
                .part("image", part);
            if req.want_alpha {
                form = form
                    .text("background", "transparent")
                    .text("output_format", "png");
            }
            client()
                .post("https://api.openai.com/v1/images/edits")
                .bearer_auth(key)
                .multipart(form)
                .send()
                .await
        }
        None => {
            client()
                .post("https://api.openai.com/v1/images/generations")
                .bearer_auth(key)
                .json(&request::openai_body(req, default_size))
                .send()
                .await
        }
    }
    .map_err(|e| net(label, e))?;

    let body = json_or_error(label, res).await?;
    match request::read_data_array(label, &body)? {
        (Payload::Bytes(b), revised) => Ok((b, "image/png".into(), revised)),
        (Payload::Url(u), revised) => {
            let (bytes, media) = fetch_image(label, &u).await?;
            Ok((bytes, media, revised))
        }
    }
}

async fn xai(req: &GenerateRequest, key: &str, label: &str) -> Result<Generated, AiError> {
    let url = if req.reference_png.is_some() {
        "https://api.x.ai/v1/images/edits"
    } else {
        "https://api.x.ai/v1/images/generations"
    };
    let res = match &req.reference_png {
        Some(png) => {
            let part = reqwest::multipart::Part::bytes(png.clone())
                .file_name("reference.png")
                .mime_str("image/png")
                .map_err(|e| AiError::Decode(e.to_string()))?;
            let form = reqwest::multipart::Form::new()
                .text("model", req.model.clone())
                .text("prompt", req.prompt.clone())
                .text("n", "1")
                .text("response_format", "b64_json")
                .part("image", part);
            client()
                .post(url)
                .bearer_auth(key)
                .multipart(form)
                .send()
                .await
        }
        None => {
            client()
                .post(url)
                .bearer_auth(key)
                .json(&request::xai_body(req))
                .send()
                .await
        }
    }
    .map_err(|e| net(label, e))?;

    let body = json_or_error(label, res).await?;
    match request::read_data_array(label, &body)? {
        (Payload::Bytes(b), revised) => Ok((b, "image/png".into(), revised)),
        (Payload::Url(u), revised) => {
            let (bytes, media) = fetch_image(label, &u).await?;
            Ok((bytes, media, revised))
        }
    }
}

async fn recraft(
    req: &GenerateRequest,
    key: &str,
    default_size: &str,
    label: &str,
) -> Result<Generated, AiError> {
    let res = client()
        .post("https://external.api.recraft.ai/v1/images/generations")
        .bearer_auth(key)
        .json(&request::recraft_body(req, default_size))
        .send()
        .await
        .map_err(|e| net(label, e))?;
    let body = json_or_error(label, res).await?;
    match request::read_data_array(label, &body)? {
        (Payload::Bytes(b), revised) => Ok((b, "image/png".into(), revised)),
        (Payload::Url(u), revised) => {
            let (bytes, media) = fetch_image(label, &u).await?;
            Ok((bytes, media, revised))
        }
    }
}

async fn google(req: &GenerateRequest, key: &str, label: &str) -> Result<Generated, AiError> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
        req.model
    );
    let res = client()
        .post(url)
        .header("x-goog-api-key", key)
        .json(&request::google_body(req))
        .send()
        .await
        .map_err(|e| net(label, e))?;
    let body = json_or_error(label, res).await?;
    Ok((request::read_google(&body)?, "image/png".into(), None))
}

async fn bfl(
    req: &GenerateRequest,
    key: &str,
    default_size: &str,
    label: &str,
) -> Result<Generated, AiError> {
    let submit = client()
        .post(format!("https://api.bfl.ai/v1/{}", req.model))
        .header("x-key", key)
        .json(&request::bfl_body(req, default_size))
        .send()
        .await
        .map_err(|e| net(label, e))?;
    let body = json_or_error(label, submit).await?;
    let polling_url = request::read_bfl_submit(&body)?;

    for _ in 0..POLL_LIMIT {
        tokio_sleep(POLL_INTERVAL).await;
        let res = client()
            .get(&polling_url)
            .header("x-key", key)
            .send()
            .await
            .map_err(|e| net(label, e))?;
        let body = json_or_error(label, res).await?;
        match request::read_bfl_poll(&body) {
            BflPoll::Pending => continue,
            BflPoll::Ready(url) => {
                let (bytes, media) = fetch_image(label, &url).await?;
                return Ok((bytes, media, None));
            }
            BflPoll::Failed(why) => {
                return Err(AiError::Refused(format!(
                    "{label} could not finish the image: {why}"
                )))
            }
        }
    }
    Err(AiError::Timeout {
        provider: label.to_string(),
    })
}

async fn stability(req: &GenerateRequest, key: &str, label: &str) -> Result<Generated, AiError> {
    let mut form = reqwest::multipart::Form::new();
    for (name, value) in request::stability_fields(req) {
        form = form.text(name, value);
    }
    let res = client()
        .post(format!(
            "https://api.stability.ai/v2beta/stable-image/generate/{}",
            req.model
        ))
        .bearer_auth(key)
        .header(reqwest::header::ACCEPT, "image/*")
        .multipart(form)
        .send()
        .await
        .map_err(|e| net(label, e))?;
    let status = res.status();
    if !status.is_success() {
        let text = res.text().await.unwrap_or_default();
        return Err(AiError::from_status(
            label,
            status.as_u16(),
            request::error_message(&text),
        ));
    }
    let media = res
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/png")
        .split(';')
        .next()
        .unwrap_or("image/png")
        .to_string();
    let bytes = res.bytes().await.map_err(|e| net(label, e))?;
    Ok((bytes.to_vec(), media, None))
}

async fn ideogram(
    req: &GenerateRequest,
    key: &str,
    default_size: &str,
    label: &str,
) -> Result<Generated, AiError> {
    let res = client()
        .post("https://api.ideogram.ai/v1/ideogram-v3/generate")
        .header("Api-Key", key)
        .json(&request::ideogram_body(req, default_size))
        .send()
        .await
        .map_err(|e| net(label, e))?;
    let body = json_or_error(label, res).await?;
    let url = request::read_ideogram(&body)?;
    let (bytes, media) = fetch_image(label, &url).await?;
    Ok((bytes, media, None))
}

/// Sleeps without pulling in a tokio dependency of our own: reqwest already runs on tokio, so a
/// timer is available wherever this crate is used.
async fn tokio_sleep(duration: Duration) {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        std::thread::sleep(duration);
        let _ = tx.send(());
    });
    let _ = tokio_recv(rx).await;
}

async fn tokio_recv(rx: std::sync::mpsc::Receiver<()>) -> Option<()> {
    // The wait happens on a blocking helper thread, so this never parks the async executor for
    // longer than the poll interval.
    std::thread::spawn(move || rx.recv().ok())
        .join()
        .ok()
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(provider: &str, model: &str) -> GenerateRequest {
        GenerateRequest {
            provider: provider.into(),
            model: model.into(),
            prompt: "a copper patina".into(),
            reference_png: None,
            size: None,
            want_alpha: false,
        }
    }

    /// These four checks run before any network call, so they are testable without one.
    #[test]
    fn unknown_providers_and_models_are_refused() {
        let err = futures_lite_block(generate(&req("nope", "x"), "k"));
        assert!(matches!(err, Err(AiError::UnknownProvider(p)) if p == "nope"));
        let err = futures_lite_block(generate(&req("openai", "no-such-model"), "k"));
        assert!(matches!(err, Err(AiError::UnknownModel { .. })));
    }

    #[test]
    fn an_empty_key_is_caught_before_any_request() {
        let err = futures_lite_block(generate(&req("openai", "gpt-image-2.5-flare"), "   "));
        assert!(matches!(err, Err(AiError::MissingKey(_))));
        assert!(matches!(
            futures_lite_block(test_key("openai", "")),
            Err(AiError::MissingKey(_))
        ));
    }

    #[test]
    fn a_reference_picture_on_a_model_that_cannot_take_one_is_refused() {
        let mut r = req("recraft", "recraftv3");
        r.reference_png = Some(vec![1, 2, 3]);
        let err = futures_lite_block(generate(&r, "key"));
        assert!(matches!(err, Err(AiError::Unsupported(m)) if m.contains("reference picture")));
    }

    #[test]
    fn test_key_rejects_an_unknown_provider() {
        assert!(matches!(
            futures_lite_block(test_key("nope", "k")),
            Err(AiError::UnknownProvider(_))
        ));
    }

    /// Runs a future to completion on this thread. The futures under test return before their
    /// first await point, so no reactor is needed.
    fn futures_lite_block<T>(mut fut: impl std::future::Future<Output = T>) -> T {
        use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
        fn noop(_: *const ()) {}
        fn clone(p: *const ()) -> RawWaker {
            RawWaker::new(p, &VTABLE)
        }
        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
        let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
        let mut cx = Context::from_waker(&waker);
        let mut fut = unsafe { std::pin::Pin::new_unchecked(&mut fut) };
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => v,
            Poll::Pending => panic!("this future must resolve without a reactor"),
        }
    }
}
