//! Bring-your-own-key image generation for FolderSkin.
//!
//! The user pastes a key from a provider they already have an account with; the app keeps it
//! and hands it to [`generate`] for each request. There is no FolderSkin server: every call goes
//! straight from the user's machine to the provider they chose.
//!
//! [`catalogue`] lists what is on offer, [`request`] describes each provider's request and reads
//! its reply (pure, so every field is unit-tested), [`prompts`] composes the prompt, [`generate`]
//! sends the request and follows the reply to the picture, and [`finish`] plans the request and
//! makes the answer a skin.

pub mod catalogue;
pub mod error;
pub mod finish;
pub mod prompts;
pub mod request;

pub use catalogue::{model, provider, providers, ModelInfo, ProviderInfo};
pub use error::AiError;
pub use finish::{finish, plan, Finished};

use request::{Auth, Body, FieldValue, Incoming, Method, Outgoing, Picture, Wait};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// What the caller wants generated: the contract between what composes a request ([`plan`]) and
/// the providers. `prompt` is already composed, and every provider gets it verbatim. The other
/// fields are settings: each goes into the provider's own parameters where it has one and is
/// left out where it hasn't, and none of them is ever written into the prompt.
#[derive(Clone, Debug, Default)]
pub struct GenerateRequest {
    pub provider: String,
    pub model: String,
    pub prompt: String,
    /// Pictures to work from, each with its role. They are sent in role order (FolderSkin's
    /// template, then the subject pictures, then the style pictures), which is how the prompt
    /// numbers them, and a model that takes fewer gets the first ones.
    pub references: Vec<Reference>,
    /// The exact size the shape wants, "1024x960". Each provider asks for it, or for the nearest
    /// size or aspect ratio it offers. `None` uses the model's first listed size.
    pub size: Option<String>,
    /// Ask for a transparent background. Only set for models that really support it.
    pub want_alpha: bool,
    /// The flat colour around the subject when FolderSkin cuts it out itself: the colour the
    /// prompt names and the template sits on, and the one [`finish`] keys out. Recraft is also
    /// told it as a parameter. `None` when nothing is cut out.
    pub key_colour: Option<[u8; 3]>,
    /// What the picture must not show, as short phrases ("border", "cartoon"), for the providers
    /// that take a negative prompt. Watermarks and signatures are always kept out, and text is
    /// too unless `lettering` asks for some.
    pub keep_out: Vec<String>,
    /// The chosen style as the provider's own preset, by the provider's id for it: Stability's
    /// "photographic", Ideogram's "WATERCOLOR" (or a style type, "REALISTIC"). Sent only where
    /// the provider documents that preset; Recraft V4.1 and the rest have none.
    pub style_preset: Option<String>,
    /// The exact words to letter on the picture, when it should carry any. The prompt says them;
    /// this sets the providers that have a switch for text.
    pub lettering: Option<String>,
}

impl GenerateRequest {
    /// The reference pictures in the order they are sent, at most `max` of them: the template
    /// first, then the subject pictures, then the style pictures, each role in the order given.
    pub fn references_in_order(&self, max: usize) -> Vec<&Reference> {
        let mut refs: Vec<&Reference> = self.references.iter().collect();
        refs.sort_by_key(|r| r.role);
        refs.truncate(max);
        refs
    }
}

/// A picture sent with the prompt, and what it is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reference {
    pub role: Role,
    /// The picture as a PNG, no longer than [`finish::REFERENCE_MAX_SIDE`] on its longer side.
    pub png: Vec<u8>,
}

impl Reference {
    pub fn new(role: Role, png: Vec<u8>) -> Reference {
        Reference { role, png }
    }
}

/// What a reference picture is for. They are sent in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    /// FolderSkin's blank template: the exact shape to paint on.
    Template,
    /// The person's own picture of what to paint.
    Subject,
    /// The person's own picture of how to paint it: its medium, palette and light.
    Style,
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
    /// The colour the picture was asked to sit on, for [`finish`] to cut out: the request's own.
    pub key_colour: Option<[u8; 3]>,
    /// What the provider said the request used or cost, in its own units, for the log: "xAI Grok
    /// charged $0.040".
    pub usage: Option<String>,
}

/// Where the requests go. The default is every provider's own API. A test points them all at a
/// stand-in on this computer instead, which answers the way the provider's documentation says.
#[derive(Clone, Debug)]
pub struct Hosts {
    /// Takes the place of every provider's own address ("https://api.openai.com") when set.
    origin: Option<String>,
    /// How long to wait before asking Black Forest Labs again whether its picture is ready.
    poll_every: Duration,
}

impl Default for Hosts {
    fn default() -> Hosts {
        Hosts {
            origin: None,
            poll_every: Duration::from_secs(1),
        }
    }
}

impl Hosts {
    /// Every provider at `origin` ("http://127.0.0.1:8080"), asked again without waiting.
    pub fn at(origin: &str) -> Hosts {
        Hosts {
            origin: Some(origin.trim_end_matches('/').to_string()),
            poll_every: Duration::from_millis(5),
        }
    }

    /// `url` with a provider's own address swapped for the stand-in's, when there is one.
    fn url(&self, url: &str) -> String {
        match &self.origin {
            Some(origin) => request::PROVIDER_ORIGINS
                .iter()
                .find_map(|o| url.strip_prefix(o))
                .map_or_else(|| url.to_string(), |path| format!("{origin}{path}")),
            None => url.to_string(),
        }
    }

    /// Whether a URL a provider handed back may be sent the key: one of `hosts` over HTTPS, or
    /// the stand-in. A key only ever goes to the provider it belongs to.
    fn may_carry_key(&self, url: &str, hosts: &[&str]) -> bool {
        if let Some(origin) = &self.origin {
            if url.starts_with(&format!("{origin}/")) {
                return true;
            }
        }
        let Some(rest) = url.strip_prefix("https://") else {
            return false;
        };
        let host = rest.split(['/', '?', '#']).next().unwrap_or("");
        // "user@host" and "host:port" say where it really goes.
        if host.contains(['@', ':']) {
            return false;
        }
        hosts
            .iter()
            .any(|h| host == *h || host.ends_with(&format!(".{h}")))
    }
}

/// How long the one request that paints may take. A large model on a long prompt answers within
/// two minutes; one that hasn't answered in five has stalled.
const PAINT_TIMEOUT: Duration = Duration::from_secs(300);
/// How long checking a key, asking whether a picture is ready or downloading it may take.
const QUICK_TIMEOUT: Duration = Duration::from_secs(60);
/// How long to go on asking Black Forest Labs for a picture it has taken on.
const WAIT_LIMIT: Duration = Duration::from_secs(300);

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .timeout(PAINT_TIMEOUT)
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

/// Sends `out` with `key` and reads the whole reply, whatever its status: what a status means is
/// the provider's to say ([`request::read`]).
async fn send(
    out: &Outgoing,
    key: &str,
    hosts: &Hosts,
    label: &str,
    timeout: Duration,
) -> Result<Incoming, AiError> {
    let url = hosts.url(&out.url);
    let mut builder = match out.method {
        Method::Get => client().get(&url),
        Method::Post => client().post(&url),
    }
    .timeout(timeout);
    builder = match out.auth {
        Auth::Bearer => builder.bearer_auth(key),
        Auth::Header(name) => {
            let mut value = reqwest::header::HeaderValue::from_str(key.trim())
                .map_err(|_| AiError::Unauthorized(label.to_string()))?;
            value.set_sensitive(true);
            builder.header(name, value)
        }
    };
    if let Some(accept) = out.accept {
        builder = builder.header(reqwest::header::ACCEPT, accept);
    }
    builder = match &out.body {
        Body::Empty => builder,
        Body::Json(value) => builder.json(value),
        Body::Form(fields) => {
            let mut form = reqwest::multipart::Form::new();
            for field in fields {
                form = match &field.value {
                    FieldValue::Text(text) => form.text(field.name.clone(), text.clone()),
                    FieldValue::File {
                        file_name,
                        mime,
                        bytes,
                    } => {
                        let part = reqwest::multipart::Part::bytes(bytes.clone())
                            .file_name(file_name.clone())
                            .mime_str(mime)
                            .map_err(|e| AiError::Decode(e.to_string()))?;
                        form.part(field.name.clone(), part)
                    }
                };
            }
            builder.multipart(form)
        }
    };
    let res = builder.send().await.map_err(|e| net(label, e))?;
    let status = res.status().as_u16();
    let headers = res
        .headers()
        .iter()
        .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();
    let body = res.bytes().await.map_err(|e| net(label, e))?.to_vec();
    Ok(Incoming {
        status,
        headers,
        body,
    })
}

/// Downloads a picture the provider left at a link. The link is the provider's own and carries
/// its own permission, so no key goes with it.
async fn fetch_image(label: &str, url: &str, hosts: &Hosts) -> Result<(Vec<u8>, String), AiError> {
    let res = client()
        .get(hosts.url(url))
        .timeout(QUICK_TIMEOUT)
        .send()
        .await
        .map_err(|e| net(label, e))?;
    let status = res.status();
    if !status.is_success() {
        return Err(AiError::Provider {
            provider: label.to_string(),
            status: status.as_u16(),
            message: "the image link the provider gave could not be downloaded".into(),
        });
    }
    let media = request::media_type_of(
        res.headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
    );
    let bytes = res.bytes().await.map_err(|e| net(label, e))?;
    Ok((bytes.to_vec(), media))
}

/// Generates one image with the user's key.
pub async fn generate(req: &GenerateRequest, api_key: &str) -> Result<GenerateResult, AiError> {
    generate_with(req, api_key, &Hosts::default()).await
}

/// [`generate`], with the requests sent to `hosts`: how the tests send each provider's real
/// request to a stand-in on this computer.
pub async fn generate_with(
    req: &GenerateRequest,
    api_key: &str,
    hosts: &Hosts,
) -> Result<GenerateResult, AiError> {
    let provider =
        provider(&req.provider).ok_or_else(|| AiError::UnknownProvider(req.provider.clone()))?;
    let model = model(&req.provider, &req.model).ok_or_else(|| AiError::UnknownModel {
        provider: req.provider.clone(),
        model: req.model.clone(),
    })?;
    if api_key.trim().is_empty() {
        return Err(AiError::MissingKey(provider.label.to_string()));
    }
    if !req.references.is_empty() && !model.accepts_reference {
        return Err(AiError::Unsupported(format!(
            "{} cannot work from a reference picture. Clear it, or pick another model.",
            model.label
        )));
    }
    let label = provider.label;
    let out = request::build(provider.id, model, req);
    let reply = send(&out, api_key, hosts, label, PAINT_TIMEOUT).await?;
    let answer = request::read(provider.id, label, &reply)?;
    let (image, media_type) = match answer.picture {
        Picture::Bytes { bytes, media_type } => (bytes, media_type),
        Picture::Link(url) => fetch_image(label, &url, hosts).await?,
        Picture::Wait(wait) => wait_for(label, &wait, api_key, hosts).await?,
    };
    Ok(GenerateResult {
        image,
        media_type,
        native_alpha: req.want_alpha && model.native_alpha,
        model_used: model.id.to_string(),
        revised_prompt: answer.revised_prompt,
        key_colour: req.key_colour,
        usage: answer.usage,
    })
}

/// Asks where `wait` says until the picture is ready, then downloads it. Only Black Forest Labs
/// works this way: it takes the request on, and paints it in its own time.
async fn wait_for(
    label: &str,
    wait: &Wait,
    key: &str,
    hosts: &Hosts,
) -> Result<(Vec<u8>, String), AiError> {
    if !hosts.may_carry_key(&wait.url, wait.hosts) {
        return Err(AiError::Decode(format!(
            "{label} said to collect the picture somewhere FolderSkin doesn't send your key"
        )));
    }
    let started = Instant::now();
    loop {
        // The runtime's timer: the thread serves other requests while this one waits.
        tokio::time::sleep(hosts.poll_every).await;
        let reply = send(&wait.poll(), key, hosts, label, QUICK_TIMEOUT).await?;
        if let Some(url) = request::read_wait(label, &reply)? {
            return fetch_image(label, &url, hosts).await;
        }
        if started.elapsed() > WAIT_LIMIT {
            return Err(AiError::Timeout {
                provider: label.to_string(),
            });
        }
    }
}

/// A cheap authenticated call that proves the key works without generating anything.
pub async fn test_key(provider_id: &str, api_key: &str) -> Result<(), AiError> {
    test_key_with(provider_id, api_key, &Hosts::default()).await
}

/// [`test_key`], with the request sent to `hosts`.
pub async fn test_key_with(provider_id: &str, api_key: &str, hosts: &Hosts) -> Result<(), AiError> {
    let provider =
        provider(provider_id).ok_or_else(|| AiError::UnknownProvider(provider_id.to_string()))?;
    if api_key.trim().is_empty() {
        return Err(AiError::MissingKey(provider.label.to_string()));
    }
    let label = provider.label;
    let out = request::key_check(provider.id)
        .ok_or_else(|| AiError::UnknownProvider(provider.id.to_string()))?;
    let reply = send(&out, api_key, hosts, label, QUICK_TIMEOUT).await?;
    let message = if (200..300).contains(&reply.status) {
        String::new()
    } else {
        request::error_message(&String::from_utf8_lossy(&reply.body))
    };
    key_check(provider.id, label, reply.status, message)
}

/// What the answer to [`test_key`]'s request says about the key. The request asks for nothing
/// but the key, so a refusal is the key's, whatever the status.
fn key_check(provider_id: &str, label: &str, status: u16, message: String) -> Result<(), AiError> {
    let lower = message.to_lowercase();
    match (provider_id, status) {
        (_, 200..=299) => Ok(()),
        // A restricted OpenAI key may paint without being allowed to list the models.
        ("openai", 401 | 403) if lower.contains("scope") || lower.contains("permission") => Ok(()),
        // Black Forest Labs answers a key that isn't even shaped like one with 422, and xAI an
        // incorrect one with 400.
        ("bfl", 422) | ("xai", 400) | (_, 401 | 403) => {
            Err(AiError::Unauthorized(label.to_string()))
        }
        // Past the key and turned away for the picture that isn't one: the key is good. Only
        // the answers that mean a bad picture: a 404 or 405 says the endpoint moved, not that
        // the key was accepted.
        ("ideogram", 400 | 415 | 422) => Ok(()),
        _ => Err(AiError::from_status(label, status, message)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(provider: &str, model: &str) -> GenerateRequest {
        GenerateRequest {
            provider: provider.into(),
            model: model.into(),
            prompt: "a copper patina".into(),
            ..GenerateRequest::default()
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
        let mut r = req("stability", "core");
        r.references = vec![Reference::new(Role::Subject, vec![1, 2, 3])];
        let err = futures_lite_block(generate(&r, "key"));
        assert!(matches!(err, Err(AiError::Unsupported(m)) if m.contains("reference picture")));
    }

    #[test]
    fn references_go_template_first_then_subject_then_style() {
        let mut r = req("openai", "gpt-image-2.5-flare");
        r.references = vec![
            Reference::new(Role::Style, vec![3]),
            Reference::new(Role::Subject, vec![2]),
            Reference::new(Role::Template, vec![1]),
            Reference::new(Role::Subject, vec![4]),
        ];
        let order: Vec<u8> = r.references_in_order(16).iter().map(|p| p.png[0]).collect();
        assert_eq!(order, [1, 2, 4, 3]);
        // A model that takes two gets the template and the first subject picture.
        let order: Vec<u8> = r.references_in_order(2).iter().map(|p| p.png[0]).collect();
        assert_eq!(order, [1, 2]);
    }

    #[test]
    fn a_stand_in_takes_the_place_of_every_provider_but_nothing_else() {
        let hosts = Hosts::at("http://127.0.0.1:9/");
        assert_eq!(
            hosts.url("https://api.openai.com/v1/images/edits"),
            "http://127.0.0.1:9/v1/images/edits"
        );
        assert_eq!(
            hosts.url("https://generativelanguage.googleapis.com/v1beta/models"),
            "http://127.0.0.1:9/v1beta/models"
        );
        // A link a provider handed back goes where it says.
        assert_eq!(
            hosts.url("https://cdn.example.test/a.png"),
            "https://cdn.example.test/a.png"
        );
        assert_eq!(
            Hosts::default().url("https://api.x.ai/v1/images/generations"),
            "https://api.x.ai/v1/images/generations"
        );
    }

    #[test]
    fn a_key_goes_only_to_its_own_provider() {
        let real = Hosts::default();
        let bfl = &["bfl.ai"];
        assert!(real.may_carry_key("https://api.bfl.ai/v1/get_result?id=1", bfl));
        assert!(real.may_carry_key("https://api.us1.bfl.ai/v1/get_result?id=1", bfl));
        for elsewhere in [
            "http://api.bfl.ai/v1/get_result",
            "https://bfl.ai.example.test/v1",
            "https://notbfl.ai/v1",
            "https://api.bfl.ai@example.test/v1",
            "https://api.bfl.ai:8443/v1",
            "https://example.test/?https://api.bfl.ai/",
        ] {
            assert!(!real.may_carry_key(elsewhere, bfl), "{elsewhere}");
        }
        let test = Hosts::at("http://127.0.0.1:9");
        assert!(test.may_carry_key("http://127.0.0.1:9/v1/get_result?id=1", bfl));
        assert!(!test.may_carry_key("http://127.0.0.1:99/v1/get_result?id=1", bfl));
    }

    #[test]
    fn a_key_check_reads_each_providers_answer() {
        assert!(key_check("openai", "OpenAI", 200, String::new()).is_ok());
        assert!(matches!(
            key_check("openai", "OpenAI", 401, "no".into()),
            Err(AiError::Unauthorized(_))
        ));
        // A restricted key that can't list models may still paint.
        assert!(key_check(
            "openai",
            "OpenAI",
            403,
            "You have insufficient permissions for this operation. Missing scopes: \
             api.model.read."
                .into()
        )
        .is_ok());
        // xAI answers an incorrect key with 400; Google with 400 and its own words.
        assert!(matches!(
            key_check("xai", "xAI Grok", 400, "Incorrect API key provided".into()),
            Err(AiError::Unauthorized(_))
        ));
        assert!(matches!(
            key_check(
                "google",
                "Google Gemini",
                400,
                "API key not valid. Please pass a valid API key.".into()
            ),
            Err(AiError::Unauthorized(_))
        ));
        assert!(matches!(
            key_check("recraft", "Recraft", 403, "Forbidden".into()),
            Err(AiError::Unauthorized(_))
        ));
        // Black Forest Labs: 403 for a key it doesn't know, 422 for one that isn't shaped right.
        for status in [403, 422] {
            assert!(matches!(
                key_check(
                    "bfl",
                    "Black Forest Labs",
                    status,
                    "Not authenticated".into()
                ),
                Err(AiError::Unauthorized(_))
            ));
        }
        // Ideogram: the key is checked first, then the picture that isn't one is turned away.
        for status in [400, 415, 422] {
            assert!(key_check("ideogram", "Ideogram", status, "bad image".into()).is_ok());
        }
        // An endpoint that moved or changed says nothing about the key.
        for status in [404, 405, 413] {
            assert!(
                matches!(
                    key_check("ideogram", "Ideogram", status, "Not Found".into()),
                    Err(AiError::Provider { .. })
                ),
                "{status}"
            );
        }
        assert!(matches!(
            key_check("ideogram", "Ideogram", 401, "Access denied".into()),
            Err(AiError::Unauthorized(_))
        ));
        assert!(matches!(
            key_check("ideogram", "Ideogram", 429, String::new()),
            Err(AiError::RateLimited(_))
        ));
        assert!(matches!(
            key_check("ideogram", "Ideogram", 503, "down".into()),
            Err(AiError::Provider { status: 503, .. })
        ));
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
