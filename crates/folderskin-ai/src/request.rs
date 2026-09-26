//! Turning a [`GenerateRequest`] into each provider's HTTP request, and reading its reply.
//!
//! Everything here is pure, so every request can be checked field by field without a network: a
//! builder returns the request as it goes on the wire, all but the key ([`Outgoing`]), and a
//! reader takes the reply's status, headers and bytes ([`Incoming`]). [`crate::generate`] only
//! sends what is built here and follows what is read: a picture, a link to one, or somewhere to
//! wait for one.
//!
//! Each request follows its provider's own API reference, checked against that provider's own
//! SDK where it has one. The settings of a request go into the provider's parameters wherever it
//! has one (the exact size or the nearest aspect ratio it offers, its negative prompt, its own
//! style presets, its switch for rewriting prompts), and none of them is ever written into the
//! prompt.

use crate::catalogue::ModelInfo;
use crate::{AiError, GenerateRequest, Reference, Role};
use base64::Engine;
use serde_json::{json, Value};

// ---------------------------------------------------------------- where each provider is

pub const OPENAI: &str = "https://api.openai.com";
pub const XAI: &str = "https://api.x.ai";
pub const RECRAFT: &str = "https://external.api.recraft.ai";
pub const GOOGLE: &str = "https://generativelanguage.googleapis.com";
pub const BFL: &str = "https://api.bfl.ai";
pub const STABILITY: &str = "https://api.stability.ai";
pub const IDEOGRAM: &str = "https://api.ideogram.ai";

/// Every provider's own address, which a test swaps for its stand-in's.
pub const PROVIDER_ORIGINS: &[&str] = &[OPENAI, XAI, RECRAFT, GOOGLE, BFL, STABILITY, IDEOGRAM];

// ---------------------------------------------------------------- a request, described

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

/// How the key goes with a request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Auth {
    /// `Authorization: Bearer <key>`.
    Bearer,
    /// The key alone, in a header of the provider's own.
    Header(&'static str),
}

/// A request's body.
#[derive(Clone, Debug, PartialEq)]
pub enum Body {
    Empty,
    Json(Value),
    /// multipart/form-data, its fields in order.
    Form(Vec<Field>),
}

/// One field of a multipart/form-data body.
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub name: String,
    pub value: FieldValue,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FieldValue {
    Text(String),
    File {
        file_name: String,
        mime: &'static str,
        bytes: Vec<u8>,
    },
}

impl Field {
    fn text(name: &str, value: impl Into<String>) -> Field {
        Field {
            name: name.to_string(),
            value: FieldValue::Text(value.into()),
        }
    }

    fn file(name: &str, file_name: &str, bytes: &[u8]) -> Field {
        Field {
            name: name.to_string(),
            value: FieldValue::File {
                file_name: file_name.to_string(),
                mime: "image/png",
                bytes: bytes.to_vec(),
            },
        }
    }
}

impl Body {
    /// The JSON body, when it is one.
    pub fn json(&self) -> Option<&Value> {
        match self {
            Body::Json(v) => Some(v),
            _ => None,
        }
    }

    /// A form's text field by name.
    pub fn text(&self, name: &str) -> Option<&str> {
        match self {
            Body::Form(fields) => fields.iter().find_map(|f| match &f.value {
                FieldValue::Text(t) if f.name == name => Some(t.as_str()),
                _ => None,
            }),
            _ => None,
        }
    }

    /// A form's files under `name`, in the order they are sent.
    pub fn files(&self, name: &str) -> Vec<&[u8]> {
        match self {
            Body::Form(fields) => fields
                .iter()
                .filter_map(|f| match &f.value {
                    FieldValue::File { bytes, .. } if f.name == name => Some(bytes.as_slice()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Every field name of a form in order, or every key of a JSON object.
    pub fn names(&self) -> Vec<String> {
        match self {
            Body::Form(fields) => fields.iter().map(|f| f.name.clone()).collect(),
            Body::Json(Value::Object(map)) => map.keys().cloned().collect(),
            _ => Vec::new(),
        }
    }
}

/// A request to a provider as it goes on the wire, all but the key: built here, sent by
/// [`crate::generate`], and checked field by field in the tests.
#[derive(Clone, Debug, PartialEq)]
pub struct Outgoing {
    pub method: Method,
    pub url: String,
    pub auth: Auth,
    /// The Accept header, for a provider that can answer in more than one form.
    pub accept: Option<&'static str>,
    pub body: Body,
}

impl Outgoing {
    fn get(url: String, auth: Auth) -> Outgoing {
        Outgoing {
            method: Method::Get,
            url,
            auth,
            accept: None,
            body: Body::Empty,
        }
    }

    fn json(url: String, auth: Auth, body: Value) -> Outgoing {
        Outgoing {
            method: Method::Post,
            url,
            auth,
            accept: None,
            body: Body::Json(body),
        }
    }

    fn form(url: String, auth: Auth, fields: Vec<Field>) -> Outgoing {
        Outgoing {
            method: Method::Post,
            url,
            auth,
            accept: None,
            body: Body::Form(fields),
        }
    }

    /// The media type the body goes as, without the multipart boundary the sender adds.
    pub fn content_type(&self) -> Option<&'static str> {
        match self.body {
            Body::Empty => None,
            Body::Json(_) => Some("application/json"),
            Body::Form(_) => Some("multipart/form-data"),
        }
    }
}

// ---------------------------------------------------------------- a reply

/// A provider's reply, whole: its status, its headers and its bytes.
#[derive(Clone, Debug, Default)]
pub struct Incoming {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Incoming {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    fn json(&self, label: &str) -> Result<Value, AiError> {
        serde_json::from_slice(&self.body).map_err(|_| {
            AiError::Decode(format!(
                "{label} replied with something FolderSkin could not read: {}",
                glimpse(&self.text())
            ))
        })
    }
}

/// What a reply led to.
#[derive(Clone, Debug, PartialEq)]
pub struct Answer {
    pub picture: Picture,
    /// The prompt as the provider rewrote it, when it says.
    pub revised_prompt: Option<String>,
    /// What the provider says the request used or cost, for the log.
    pub usage: Option<String>,
}

/// The picture, or the way to it.
#[derive(Clone, Debug, PartialEq)]
pub enum Picture {
    Bytes {
        bytes: Vec<u8>,
        media_type: String,
    },
    /// A link to download it from, with no key.
    Link(String),
    /// Somewhere to ask until it is ready.
    Wait(Wait),
}

impl Answer {
    fn bytes(bytes: Vec<u8>, media_type: Option<&str>) -> Answer {
        let media_type = media_type
            .map(|m| media_type_of(Some(m)))
            .unwrap_or_else(|| sniff(&bytes).to_string());
        Answer {
            picture: Picture::Bytes { bytes, media_type },
            revised_prompt: None,
            usage: None,
        }
    }

    fn link(url: &str) -> Answer {
        Answer {
            picture: Picture::Link(url.to_string()),
            revised_prompt: None,
            usage: None,
        }
    }

    fn with_usage(mut self, usage: Option<String>) -> Answer {
        self.usage = usage;
        self
    }
}

/// Where to ask whether a picture is ready: Black Forest Labs takes a request on and paints it in
/// its own time.
#[derive(Clone, Debug, PartialEq)]
pub struct Wait {
    pub url: String,
    /// The hosts, with their subdomains, that may be sent the key when asking.
    pub hosts: &'static [&'static str],
}

impl Wait {
    /// The request that asks.
    pub fn poll(&self) -> Outgoing {
        Outgoing::get(self.url.clone(), Auth::Header("x-key"))
    }
}

/// A media type without its parameters, "image/png" when there is none.
pub fn media_type_of(header: Option<&str>) -> String {
    header
        .and_then(|v| v.split(';').next())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("image/png")
        .to_string()
}

/// The media type a picture's first bytes say it is.
fn sniff(bytes: &[u8]) -> &'static str {
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => "image/png",
        [0xFF, 0xD8, 0xFF, ..] => "image/jpeg",
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => "image/webp",
        _ => "image/png",
    }
}

/// A reply's first words, for a failure that shows what came back. Long runs without a space
/// (a picture's base64) are left out.
fn glimpse(text: &str) -> String {
    let words: Vec<&str> = text
        .split_whitespace()
        .map(|w| if w.len() > 60 { "…" } else { w })
        .collect();
    crate::error::shorten(&words.join(" "), 160)
}

/// A JSON reply's outline, for a failure that says what came back instead of a picture: its
/// keys, with long strings cut short.
fn outline(body: &Value) -> String {
    fn trim(v: &Value) -> Value {
        match v {
            Value::String(s) if s.chars().count() > 40 => Value::String("…".into()),
            Value::Array(items) => Value::Array(items.iter().take(2).map(trim).collect()),
            Value::Object(map) => map.iter().map(|(k, v)| (k.clone(), trim(v))).collect(),
            other => other.clone(),
        }
    }
    crate::error::shorten(&trim(body).to_string(), 200)
}

/// The failure for a reply that holds no picture where the provider's reference puts one. It
/// quotes what came back, so it can be reported as it is.
fn unexpected(label: &str, body: &Value) -> AiError {
    AiError::Decode(format!(
        "{label} replied without a picture where FolderSkin looks for one: {}",
        outline(body)
    ))
}

// ---------------------------------------------------------------- sizes

/// Width and height from a "1024x1024" string, falling back to the model's first size.
pub fn dimensions(req: &GenerateRequest, default: &str) -> (u32, u32) {
    let parse = |s: &str| {
        s.split_once(['x', 'X'])
            .and_then(|(w, h)| Some((w.trim().parse().ok()?, h.trim().parse().ok()?)))
            .filter(|&(w, h): &(u32, u32)| w > 0 && h > 0)
    };
    req.size
        .as_deref()
        .and_then(parse)
        .or_else(|| parse(default))
        .unwrap_or((1024, 1024))
}

/// `(w, h)` on a grid of `step` pixels, each side rounded to the nearest line and at least one
/// step: the Mac look's 1024 × 958 becomes 1024 × 960, the Windows look's 1024 × 805 becomes
/// 1024 × 800.
pub fn on_grid((w, h): (u32, u32), step: u32) -> (u32, u32) {
    let snap = |v: u32| ((v + step / 2) / step).max(1) * step;
    (snap(w), snap(h))
}

/// `(w, h)` scaled, keeping its shape, so it covers at least `min` pixels and at most `max`, then
/// put on a grid of `step`.
pub fn within((w, h): (u32, u32), min: u64, max: u64, step: u32) -> (u32, u32) {
    let area = u64::from(w) * u64::from(h);
    let scale = if area < min {
        (min as f64 / area as f64).sqrt()
    } else if area > max {
        (max as f64 / area as f64).sqrt()
    } else {
        1.0
    };
    let (gw, gh) = on_grid(((w as f64 * scale) as u32, (h as f64 * scale) as u32), step);
    // Rounding to the grid can land a line past a limit; one line back puts it inside.
    let area = u64::from(gw) * u64::from(gh);
    let gw = if area > max {
        gw - step
    } else if area < min {
        gw + step
    } else {
        gw
    };
    (gw, gh)
}

/// The shape of `r`, "5:4" or "1365x1024", on a log scale, where 1:2 and 2:1 are as far from 1:1
/// as each other.
fn log_shape(r: &str) -> f64 {
    let (a, b) = r.split_once([':', 'x']).unwrap_or(("1", "1"));
    let (a, b): (f64, f64) = (a.parse().unwrap_or(1.0), b.parse().unwrap_or(1.0));
    (a / b).ln()
}

/// The ratio ("5:4") or size ("1152x896") from `choices` whose shape is nearest `(w, h)`'s.
pub fn nearest(size: (u32, u32), choices: &[&'static str]) -> &'static str {
    let want = (size.0 as f64 / size.1 as f64).ln();
    choices
        .iter()
        .copied()
        .min_by(|a, b| {
            (log_shape(a) - want)
                .abs()
                .total_cmp(&(log_shape(b) - want).abs())
        })
        .unwrap_or("1:1")
}

// ---------------------------------------------------------------- shared settings

/// A reference picture as a data URL, which the JSON APIs take in place of a link.
fn data_url(png: &[u8]) -> String {
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    )
}

/// Whether the first picture sent is FolderSkin's template: then the output takes the template's
/// own frame, which is the shape's.
fn follows_template(refs: &[&Reference]) -> bool {
    refs.first().is_some_and(|r| r.role == Role::Template)
}

/// The negative prompt for a provider that takes one: the request's own keep-outs, then text
/// unless lettering was asked for, then watermarks and signatures, which no skin wants. Each
/// phrase once, in that order.
pub fn negative_prompt(req: &GenerateRequest) -> String {
    let mut phrases: Vec<String> = Vec::new();
    let mut add = |p: &str| {
        let p = p.trim();
        if !p.is_empty() && !phrases.iter().any(|q| q.eq_ignore_ascii_case(p)) {
            phrases.push(p.to_string());
        }
    };
    for p in &req.keep_out {
        add(p);
    }
    if !wants_lettering(req) {
        for p in ["text", "letters", "numbers"] {
            add(p);
        }
    }
    for p in ["watermark", "signature"] {
        add(p);
    }
    phrases.join(", ")
}

fn wants_lettering(req: &GenerateRequest) -> bool {
    req.lettering
        .as_deref()
        .is_some_and(|l| !l.trim().is_empty())
}

/// The request's style preset, when it is one of `allowed`: a preset meant for another provider,
/// or one the provider has since dropped, is left out rather than failing a paid request.
fn preset<'a>(req: &'a GenerateRequest, allowed: &[&str]) -> Option<&'a str> {
    req.style_preset
        .as_deref()
        .map(str::trim)
        .filter(|p| allowed.contains(p))
}

/// The failure a provider's own filter makes.
fn blocked(label: &str) -> AiError {
    AiError::Refused(format!(
        "{label} declined that prompt: its filter blocked the picture"
    ))
}

// ---------------------------------------------------------------- building and reading

/// The request that paints `req` with `model` at `provider`.
pub fn build(provider: &str, model: &ModelInfo, req: &GenerateRequest) -> Outgoing {
    match provider {
        "openai" => openai(model, req),
        "xai" => xai(model, req),
        "recraft" => recraft(model, req),
        "google" => google(model, req),
        "bfl" => bfl(model, req),
        "stability" => stability(model, req),
        _ => ideogram(model, req),
    }
}

/// What `reply`, to the request [`build`] made, led to. A status that isn't a success is the
/// provider's failure, in its own terms.
pub fn read(provider: &str, label: &str, reply: &Incoming) -> Result<Answer, AiError> {
    if !reply.ok() {
        return Err(failure(provider, label, reply));
    }
    match provider {
        "stability" => read_stability(label, reply),
        "google" => read_google(label, &reply.json(label)?),
        "bfl" => read_bfl_submit(label, &reply.json(label)?),
        "ideogram" => read_ideogram(label, &reply.json(label)?),
        "xai" => read_xai(label, &reply.json(label)?),
        "recraft" => read_recraft(label, &reply.json(label)?),
        _ => read_openai(label, &reply.json(label)?),
    }
}

/// A failed status as the error it is at `provider`. Most providers mean the same by the same
/// status, but not all: a 403 from Stability, or a 422 from Ideogram, is its safety check, not
/// the key or the request. Stability also names each failure, as its own clients read it.
pub fn failure(provider: &str, label: &str, reply: &Incoming) -> AiError {
    let text = reply.text();
    let message = error_message(&text);
    let refused = || AiError::Refused(format!("{label} declined that prompt: {message}"));
    match (provider, reply.status) {
        ("stability", status) => {
            let name = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v.get("name")?.as_str().map(str::to_string));
            match name.as_deref() {
                Some("content_moderation") => refused(),
                Some("unauthorized") => AiError::Unauthorized(label.to_string()),
                // Out of credits, or any other reason it names: its own words say which.
                Some(_) if status == 403 => AiError::Provider {
                    provider: label.to_string(),
                    status,
                    message,
                },
                _ if status == 403 => refused(),
                _ => AiError::from_status(label, status, message),
            }
        }
        ("ideogram", 422) => refused(),
        _ => AiError::from_status(label, reply.status, message),
    }
}

// ---------------------------------------------------------------- OpenAI

/// Sizes GPT Image 2 and 2.5 take: any, with both sides on a 16-pixel grid and between 655,360
/// and 8,294,400 pixels in all.
const OPENAI_MIN_PIXELS: u64 = 655_360;
const OPENAI_MAX_PIXELS: u64 = 8_294_400;

/// OpenAI's prices per million tokens for GPT Image 2 and 2.5: pictures out, words in, pictures
/// in.
const OPENAI_PER_MILLION: (f64, f64, f64) = (30.0, 5.0, 8.0);

/// The quality each model is asked for. Left to itself (`auto`) the price of a picture can't be
/// said beforehand. Flare is asked for its high quality, Sunburst for its best, and GPT Image 2
/// for medium, which costs what Flare's high does.
fn openai_quality(model: &str) -> &'static str {
    match model {
        "gpt-image-2.5-sunburst" => "max",
        "gpt-image-2" => "medium",
        _ => "high",
    }
}

/// A picture from words goes to generations as JSON. One from pictures goes to edits as a form,
/// every picture a part named `image[]` in role order, as OpenAI's reference and SDKs send them.
/// GPT Image 2 and 2.5 keep every input picture at high fidelity already and take no
/// `input_fidelity`.
fn openai(model: &ModelInfo, req: &GenerateRequest) -> Outgoing {
    let (w, h) = within(
        dimensions(req, model.sizes[0]),
        OPENAI_MIN_PIXELS,
        OPENAI_MAX_PIXELS,
        16,
    );
    let size = format!("{w}x{h}");
    let quality = openai_quality(model.id);
    let refs = req.references_in_order(model.max_references);
    if refs.is_empty() {
        let mut body = json!({
            "model": model.id,
            "prompt": req.prompt,
            "n": 1,
            "size": size,
            "quality": quality,
            "output_format": "png",
        });
        if req.want_alpha {
            body["background"] = json!("transparent");
        }
        return Outgoing::json(
            format!("{OPENAI}/v1/images/generations"),
            Auth::Bearer,
            body,
        );
    }
    let mut fields = vec![
        Field::text("model", model.id),
        Field::text("prompt", req.prompt.clone()),
        Field::text("n", "1"),
        Field::text("size", size),
        Field::text("quality", quality),
        Field::text("output_format", "png"),
    ];
    if req.want_alpha {
        fields.push(Field::text("background", "transparent"));
    }
    for (i, r) in refs.iter().enumerate() {
        fields.push(Field::file(
            "image[]",
            &format!("image-{}.png", i + 1),
            &r.png,
        ));
    }
    Outgoing::form(format!("{OPENAI}/v1/images/edits"), Auth::Bearer, fields)
}

/// `data[0].b64_json`, with the size, quality and tokens OpenAI says it used.
fn read_openai(label: &str, body: &Value) -> Result<Answer, AiError> {
    let first = body
        .get("data")
        .and_then(|d| d.as_array())
        .and_then(|a| a.first())
        .ok_or_else(|| unexpected(label, body))?;
    let b64 = first
        .get("b64_json")
        .and_then(Value::as_str)
        .ok_or_else(|| unexpected(label, body))?;
    Ok(Answer::bytes(decode_base64(label, b64)?, None).with_usage(openai_usage(label, body)))
}

fn openai_usage(label: &str, body: &Value) -> Option<String> {
    let usage = body.get("usage")?;
    let count = |v: Option<&Value>| v.and_then(Value::as_u64);
    let output = count(usage.get("output_tokens"))?;
    let input = count(usage.get("input_tokens")).unwrap_or(0);
    let details = usage.get("input_tokens_details");
    let pictures_in = count(details.and_then(|d| d.get("image_tokens"))).unwrap_or(0);
    let words_in = count(details.and_then(|d| d.get("text_tokens")))
        .unwrap_or_else(|| input.saturating_sub(pictures_in));
    let (out_rate, words_rate, pictures_rate) = OPENAI_PER_MILLION;
    let dollars = (output as f64 * out_rate
        + words_in as f64 * words_rate
        + pictures_in as f64 * pictures_rate)
        / 1_000_000.0;
    let made = match (
        body.get("size").and_then(Value::as_str),
        body.get("quality").and_then(Value::as_str),
    ) {
        (Some(size), Some(quality)) => format!(" at {size} and {quality} quality"),
        _ => String::new(),
    };
    Some(format!(
        "{label} made it{made} from {input} input tokens and {output} output tokens, about \
         ${dollars:.3}"
    ))
}

fn decode_base64(label: &str, b64: &str) -> Result<Vec<u8>, AiError> {
    // A data URL's header, if a provider sends one, is not part of the picture.
    let b64 = b64.split_once("base64,").map_or(b64, |(_, rest)| rest);
    base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|_| {
            AiError::Decode(format!(
                "{label} returned an image FolderSkin could not read"
            ))
        })
}

// ---------------------------------------------------------------- xAI

/// The aspect ratios both Grok Imagine models take.
const XAI_RATIOS: &[&str] = &[
    "1:1", "3:4", "4:3", "9:16", "16:9", "2:3", "3:2", "9:19.5", "19.5:9", "9:20", "20:9", "1:2",
    "2:1",
];

/// Grok Imagine takes JSON only, pictures included: each is an object whose `url` may be a data
/// URL. One picture goes as `image`, several as `images`, and the prompt names them `<IMAGE_0>`,
/// `<IMAGE_1>` in that order.
fn xai(model: &ModelInfo, req: &GenerateRequest) -> Outgoing {
    let refs = req.references_in_order(model.max_references);
    let mut body = json!({
        "model": model.id,
        "prompt": req.prompt,
        "n": 1,
        "response_format": "b64_json",
    });
    // With the template first, the picture takes the template's frame, which is the shape's
    // exactly. Otherwise the ratio is pinned: left to itself, Grok picks one for the prompt.
    if !follows_template(&refs) {
        body["aspect_ratio"] = json!(nearest(dimensions(req, model.sizes[0]), XAI_RATIOS));
    }
    let picture = |r: &&Reference| json!({ "url": data_url(&r.png), "type": "image_url" });
    let endpoint = match refs.as_slice() {
        [] => "generations",
        [one] => {
            body["image"] = picture(one);
            "edits"
        }
        many => {
            body["images"] = Value::Array(many.iter().map(picture).collect());
            "edits"
        }
    };
    Outgoing::json(format!("{XAI}/v1/images/{endpoint}"), Auth::Bearer, body)
}

/// Grok's `data[0]`, with its `mime_type` and the exact cost it reports. A picture its moderation
/// held back says `respect_moderation: false` (its picture is then a placeholder). A reply with
/// no picture and no reason is one to try again.
fn read_xai(label: &str, body: &Value) -> Result<Answer, AiError> {
    let data = body
        .get("data")
        .and_then(|d| d.as_array())
        .ok_or_else(|| unexpected(label, body))?;
    let Some(first) = data.first() else {
        return Err(AiError::NoImage(label.to_string()));
    };
    if first.get("respect_moderation").and_then(Value::as_bool) == Some(false) {
        return Err(blocked(label));
    }
    let text = |key: &str| {
        first
            .get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
    };
    // One tick is a ten-billionth of a dollar.
    let usage = body
        .get("usage")
        .and_then(|u| u.get("cost_in_usd_ticks"))
        .and_then(Value::as_u64)
        .map(|ticks| format!("{label} charged ${:.3}", ticks as f64 / 1e10));
    if let Some(b64) = text("b64_json") {
        return Ok(Answer::bytes(decode_base64(label, b64)?, text("mime_type")).with_usage(usage));
    }
    if let Some(url) = text("url") {
        return Ok(Answer::link(url).with_usage(usage));
    }
    Err(AiError::NoImage(label.to_string()))
}

// ---------------------------------------------------------------- Recraft

/// Recraft V4.1: the size nearest the shape's from its list, the picture as a PNG rather than its
/// default WebP, and, when FolderSkin cuts the picture out itself, the key colour as the
/// backdrop Recraft is told to paint. Its style names, negative prompt and no-text switch are
/// V2 and V3 only, so they aren't sent.
fn recraft(model: &ModelInfo, req: &GenerateRequest) -> Outgoing {
    let mut body = json!({
        "model": model.id,
        "prompt": req.prompt,
        "size": nearest(dimensions(req, model.sizes[0]), model.sizes),
        "response_format": "b64_json",
        "image_format": "png",
    });
    if let Some([r, g, b]) = req.key_colour {
        body["controls"] = json!({ "background_color": { "rgb": [r, g, b] } });
    }
    Outgoing::json(
        format!("{RECRAFT}/v1/images/generations"),
        Auth::Bearer,
        body,
    )
}

/// `data[0].b64_json` (or a link), the prompt as Recraft rewrote it when it says, and the units
/// it charged: a thousand to the dollar.
fn read_recraft(label: &str, body: &Value) -> Result<Answer, AiError> {
    let first = body
        .get("data")
        .and_then(|d| d.as_array())
        .and_then(|a| a.first())
        .ok_or_else(|| unexpected(label, body))?;
    let usage = body
        .get("credits")
        .and_then(Value::as_f64)
        .map(|units| format!("{label} charged {units} units, ${:.3}", units / 1000.0));
    let answer = if let Some(b64) = first.get("b64_json").and_then(Value::as_str) {
        Answer::bytes(decode_base64(label, b64)?, None)
    } else if let Some(url) = first.get("url").and_then(Value::as_str) {
        Answer::link(url)
    } else {
        return Err(unexpected(label, body));
    };
    Ok(Answer {
        revised_prompt: first
            .get("revised_prompt")
            .and_then(Value::as_str)
            .filter(|p| !p.trim().is_empty())
            .map(str::to_string),
        ..answer.with_usage(usage)
    })
}

// ---------------------------------------------------------------- Google

/// The aspect ratios every Gemini image model takes.
const GOOGLE_RATIOS: &[&str] = &[
    "1:1", "2:3", "3:2", "3:4", "4:3", "4:5", "5:4", "9:16", "16:9", "21:9",
];

/// Gemini's generateContent: the words, then the pictures in role order, and the output settings
/// in `generationConfig`: pictures only, at 1K, in the nearest aspect ratio it offers
/// (`imageConfig`, as Google's reference and its SDKs send it). With the template first the ratio
/// is left out, and the picture takes the template's own frame. The pictures go as `inlineData`,
/// the spelling Google's JavaScript SDK sends.
fn google(model: &ModelInfo, req: &GenerateRequest) -> Outgoing {
    let refs = req.references_in_order(model.max_references);
    let mut parts = vec![json!({ "text": req.prompt })];
    for r in &refs {
        parts.push(json!({
            "inlineData": {
                "mimeType": "image/png",
                "data": base64::engine::general_purpose::STANDARD.encode(&r.png),
            }
        }));
    }
    let mut image_config = json!({ "imageSize": "1K" });
    if !follows_template(&refs) {
        image_config["aspectRatio"] =
            json!(nearest(dimensions(req, model.sizes[0]), GOOGLE_RATIOS));
    }
    Outgoing::json(
        format!("{GOOGLE}/v1beta/models/{}:generateContent", model.id),
        Auth::Header("x-goog-api-key"),
        json!({
            "contents": [{ "parts": parts }],
            "generationConfig": {
                "responseModalities": ["IMAGE"],
                "imageConfig": image_config,
            },
        }),
    )
}

/// Finish reasons that mean a filter stopped the picture, rather than the model not making one.
const GOOGLE_BLOCKED: &[&str] = &[
    "SAFETY",
    "IMAGE_SAFETY",
    "PROHIBITED_CONTENT",
    "IMAGE_PROHIBITED_CONTENT",
    "BLOCKLIST",
    "SPII",
    "RECITATION",
    "IMAGE_RECITATION",
];

/// Gemini puts the picture in `candidates[0].content.parts[].inlineData`. A prompt it blocks
/// comes back with `promptFeedback.blockReason` and no candidates; a picture it blocks, or one it
/// didn't make, with a `finishReason` that says which.
fn read_google(label: &str, body: &Value) -> Result<Answer, AiError> {
    if body
        .get("promptFeedback")
        .and_then(|f| f.get("blockReason"))
        .and_then(Value::as_str)
        .is_some_and(|r| {
            !matches!(
                r,
                "" | "BLOCK_REASON_UNSPECIFIED" | "BLOCKED_REASON_UNSPECIFIED"
            )
        })
    {
        return Err(blocked(label));
    }
    let Some(candidate) = body
        .get("candidates")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
    else {
        return Err(unexpected(label, body));
    };
    let parts = candidate
        .get("content")
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.as_array())
        .map(Vec::as_slice)
        .unwrap_or_default();
    // Gemini 3 thinks before it answers, and a thought ("thought": true) can carry a draft
    // picture. The picture to keep is the last one that isn't a thought.
    let picture = parts
        .iter()
        .filter(|part| {
            !part
                .get("thought")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .filter_map(|part| part.get("inline_data").or_else(|| part.get("inlineData")))
        .filter_map(|data| Some((data, data.get("data")?.as_str()?)))
        .next_back();
    if let Some((data, b64)) = picture {
        let media = data
            .get("mime_type")
            .or_else(|| data.get("mimeType"))
            .and_then(Value::as_str);
        return Ok(
            Answer::bytes(decode_base64(label, b64)?, media).with_usage(google_usage(label, body))
        );
    }
    let reason = candidate
        .get("finishReason")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if GOOGLE_BLOCKED.contains(&reason) {
        return Err(blocked(label));
    }
    // NO_IMAGE, IMAGE_OTHER, words instead of a picture: nothing is wrong with the request.
    Err(AiError::NoImage(label.to_string()))
}

fn google_usage(label: &str, body: &Value) -> Option<String> {
    let usage = body.get("usageMetadata")?;
    let count = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or(0);
    Some(format!(
        "{label} used {} prompt tokens, {} thinking tokens and {} output tokens",
        count("promptTokenCount"),
        count("thoughtsTokenCount"),
        count("candidatesTokenCount")
    ))
}

// ---------------------------------------------------------------- Black Forest Labs

/// FLUX.2 takes any size on a 16-pixel grid. It bills by the started megapixel, for the picture
/// made and for each picture sent, so FolderSkin keeps what it asks for within one.
const BFL_MAX_PIXELS: u64 = 1024 * 1024;
const BFL_MIN_PIXELS: u64 = 64 * 64;

/// Where Black Forest Labs may say to collect a picture, with the key.
const BFL_HOSTS: &[&str] = &["bfl.ai"];

fn bfl(model: &ModelInfo, req: &GenerateRequest) -> Outgoing {
    let (w, h) = within(
        dimensions(req, model.sizes[0]),
        BFL_MIN_PIXELS,
        BFL_MAX_PIXELS,
        16,
    );
    let mut body = json!({
        "prompt": req.prompt,
        "width": w,
        "height": h,
        "output_format": "png",
        "safety_tolerance": 2,
    });
    // FLUX.2 rewrites a prompt before it paints unless told not to, and FolderSkin's prompt is
    // already the one it wants painted. klein never rewrites.
    match model.id {
        "flux-2-pro" | "flux-2-max" => body["disable_pup"] = json!(true),
        "flux-2-flex" => body["prompt_upsampling"] = json!(false),
        _ => {}
    }
    // Plain base64, as Black Forest Labs' own clients and examples send a picture.
    for (i, r) in req
        .references_in_order(model.max_references)
        .iter()
        .enumerate()
    {
        let field = match i {
            0 => "input_image".to_string(),
            n => format!("input_image_{}", n + 1),
        };
        body[field] = json!(base64::engine::general_purpose::STANDARD.encode(&r.png));
    }
    Outgoing::json(
        format!("{BFL}/v1/{}", model.id),
        Auth::Header("x-key"),
        body,
    )
}

/// The submit reply names where to ask for the picture, and what the request costs in credits,
/// a hundred to the dollar. A fast model can answer with the picture's link at once.
fn read_bfl_submit(label: &str, body: &Value) -> Result<Answer, AiError> {
    let usage = body
        .get("cost")
        .and_then(Value::as_f64)
        .map(|credits| format!("{label} charged {credits} credits, ${:.3}", credits / 100.0));
    let text = |key: &str| {
        body.get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    if let Some(sample) = text("sample") {
        return Ok(Answer::link(&sample).with_usage(usage));
    }
    let url = text("polling_url").ok_or_else(|| unexpected(label, body))?;
    Ok(Answer {
        picture: Picture::Wait(Wait {
            url: url.to_string(),
            hosts: BFL_HOSTS,
        }),
        revised_prompt: None,
        usage,
    })
}

/// What asking Black Forest Labs said: the picture's link once it is ready, `None` while it is
/// still under way.
pub fn read_wait(label: &str, reply: &Incoming) -> Result<Option<String>, AiError> {
    if !reply.ok() {
        return Err(failure("bfl", label, reply));
    }
    let body = reply.json(label)?;
    let status = body
        .get("status")
        .and_then(|s| s.as_str())
        .unwrap_or_default();
    match status {
        "Ready" => body
            .get("result")
            .and_then(|r| r.get("sample"))
            .and_then(|s| s.as_str())
            .map(|url| Some(url.to_string()))
            .ok_or_else(|| unexpected(label, &body)),
        "Content Moderated" | "Request Moderated" => Err(blocked(label)),
        // "Failed" is an older name for the same end.
        "Error" | "Failed" => {
            let why = bfl_details(&body);
            if why.contains("content_policy") || why.to_lowercase().contains("moderat") {
                return Err(blocked(label));
            }
            Err(AiError::Decode(format!(
                "{label} could not finish the image: {why}"
            )))
        }
        // The request's id is unknown or has expired: asking again won't find it.
        "Task not found" => Err(AiError::Decode(format!(
            "{label} lost track of the image before it was ready"
        ))),
        // Pending, Reasoning, Generating, and anything newer.
        _ => Ok(None),
    }
}

/// The words in a failed task's `details` (text, an object or nothing) or its `error`.
fn bfl_details(body: &Value) -> String {
    if let Some(error) = body.get("error").and_then(Value::as_str) {
        return crate::error::trim_message(error);
    }
    match body.get("details") {
        Some(Value::String(s)) if !s.trim().is_empty() => crate::error::trim_message(s),
        Some(Value::Object(map)) => map
            .values()
            .find_map(|v| match v {
                Value::String(s) => Some(crate::error::trim_message(s)),
                Value::Array(items) => items.first().and_then(Value::as_str).map(str::to_string),
                _ => None,
            })
            .unwrap_or_else(|| "the request failed".into()),
        _ => "the request failed".into(),
    }
}

// ---------------------------------------------------------------- Stability AI

/// The aspect ratios Stable Image takes.
const STABILITY_RATIOS: &[&str] = &[
    "21:9", "16:9", "3:2", "5:4", "1:1", "4:5", "2:3", "9:16", "9:21",
];

/// Stable Image's style presets, for Ultra and Core alike.
const STABILITY_PRESETS: &[&str] = &[
    "enhance",
    "anime",
    "photographic",
    "digital-art",
    "comic-book",
    "fantasy-art",
    "line-art",
    "analog-film",
    "neon-punk",
    "isometric",
    "low-poly",
    "origami",
    "modeling-compound",
    "cinematic",
    "3d-model",
    "pixel-art",
    "tile-texture",
];

/// Stable Image takes multipart form fields and answers with the picture's bytes.
fn stability(model: &ModelInfo, req: &GenerateRequest) -> Outgoing {
    let mut fields = vec![
        Field::text("prompt", req.prompt.clone()),
        Field::text(
            "aspect_ratio",
            nearest(dimensions(req, model.sizes[0]), STABILITY_RATIOS),
        ),
        Field::text("negative_prompt", negative_prompt(req)),
        Field::text("output_format", "png"),
    ];
    if let Some(style) = preset(req, STABILITY_PRESETS) {
        fields.push(Field::text("style_preset", style));
    }
    let mut out = Outgoing::form(
        format!("{STABILITY}/v2beta/stable-image/generate/{}", model.id),
        Auth::Bearer,
        fields,
    );
    out.accept = Some("image/*");
    out
}

/// The picture's bytes. One its filter caught comes back blurred, with a `finish-reason` of
/// `CONTENT_FILTERED` (and isn't charged): that one is refused, not kept.
fn read_stability(label: &str, reply: &Incoming) -> Result<Answer, AiError> {
    if reply
        .header("finish-reason")
        .is_some_and(|r| r.eq_ignore_ascii_case("CONTENT_FILTERED"))
    {
        return Err(blocked(label));
    }
    if reply.body.is_empty() {
        return Err(AiError::NoImage(label.to_string()));
    }
    Ok(Answer::bytes(
        reply.body.clone(),
        reply.header("content-type"),
    ))
}

// ---------------------------------------------------------------- Ideogram

/// The resolutions Ideogram 3.0 takes.
const IDEOGRAM_RESOLUTIONS: &[&str] = &[
    "512x1536",
    "576x1408",
    "576x1472",
    "576x1536",
    "640x1344",
    "640x1408",
    "640x1472",
    "640x1536",
    "704x1152",
    "704x1216",
    "704x1280",
    "704x1344",
    "704x1408",
    "704x1472",
    "736x1312",
    "768x1088",
    "768x1216",
    "768x1280",
    "768x1344",
    "800x1280",
    "832x960",
    "832x1024",
    "832x1088",
    "832x1152",
    "832x1216",
    "832x1248",
    "864x1152",
    "896x960",
    "896x1024",
    "896x1088",
    "896x1120",
    "896x1152",
    "960x832",
    "960x896",
    "960x1024",
    "960x1088",
    "1024x832",
    "1024x896",
    "1024x960",
    "1024x1024",
    "1088x768",
    "1088x832",
    "1088x896",
    "1088x960",
    "1120x896",
    "1152x704",
    "1152x832",
    "1152x864",
    "1152x896",
    "1216x704",
    "1216x768",
    "1216x832",
    "1248x832",
    "1280x704",
    "1280x768",
    "1280x800",
    "1312x736",
    "1344x640",
    "1344x704",
    "1344x768",
    "1408x576",
    "1408x640",
    "1408x704",
    "1472x576",
    "1472x640",
    "1472x704",
    "1536x512",
    "1536x576",
    "1536x640",
];

/// Ideogram 3.0's style types. One of these as the style preset sets the type.
const IDEOGRAM_STYLE_TYPES: &[&str] = &["AUTO", "GENERAL", "REALISTIC", "DESIGN", "FICTION"];

/// Ideogram 3.0's style presets.
const IDEOGRAM_STYLE_PRESETS: &[&str] = &[
    "80S_ILLUSTRATION",
    "90S_NOSTALGIA",
    "ABSTRACT_ORGANIC",
    "ANALOG_NOSTALGIA",
    "ART_BRUT",
    "ART_DECO",
    "ART_POSTER",
    "AURA",
    "AVANT_GARDE",
    "BAUHAUS",
    "BLUEPRINT",
    "BLURRY_MOTION",
    "BRIGHT_ART",
    "C4D_CARTOON",
    "CHILDRENS_BOOK",
    "COLLAGE",
    "COLORING_BOOK_I",
    "COLORING_BOOK_II",
    "CUBISM",
    "DARK_AURA",
    "DOODLE",
    "DOUBLE_EXPOSURE",
    "DRAMATIC_CINEMA",
    "EDITORIAL",
    "EMOTIONAL_MINIMAL",
    "ETHEREAL_PARTY",
    "EXPIRED_FILM",
    "FLAT_ART",
    "FLAT_VECTOR",
    "FOREST_REVERIE",
    "GEO_MINIMALIST",
    "GLASS_PRISM",
    "GOLDEN_HOUR",
    "GRAFFITI_I",
    "GRAFFITI_II",
    "HALFTONE_PRINT",
    "HIGH_CONTRAST",
    "HIPPIE_ERA",
    "ICONIC",
    "JAPANDI_FUSION",
    "JAZZY",
    "LONG_EXPOSURE",
    "MAGAZINE_EDITORIAL",
    "MINIMAL_ILLUSTRATION",
    "MIXED_MEDIA",
    "MONOCHROME",
    "NIGHTLIFE",
    "OIL_PAINTING",
    "OLD_CARTOONS",
    "PAINT_GESTURE",
    "POP_ART",
    "RETRO_ETCHING",
    "RIVIERA_POP",
    "SPOTLIGHT_80S",
    "STYLIZED_RED",
    "SURREAL_COLLAGE",
    "TRAVEL_POSTER",
    "VINTAGE_GEO",
    "VINTAGE_POSTER",
    "WATERCOLOR",
    "WEIRD",
    "WOODBLOCK_PRINT",
];

/// Ideogram 3.0, as the multipart form its reference documents: Magic Prompt off, so the prompt
/// is painted as written, the resolution nearest the shape's, and a negative prompt.
fn ideogram(model: &ModelInfo, req: &GenerateRequest) -> Outgoing {
    let mut fields = vec![
        Field::text("prompt", req.prompt.clone()),
        Field::text("rendering_speed", "DEFAULT"),
        Field::text("magic_prompt", "OFF"),
        Field::text("num_images", "1"),
        Field::text(
            "resolution",
            nearest(dimensions(req, model.sizes[0]), IDEOGRAM_RESOLUTIONS),
        ),
        Field::text("negative_prompt", negative_prompt(req)),
    ];
    if let Some(style_type) = preset(req, IDEOGRAM_STYLE_TYPES) {
        fields.push(Field::text("style_type", style_type));
    } else if let Some(style) = preset(req, IDEOGRAM_STYLE_PRESETS) {
        fields.push(Field::text("style_preset", style));
    }
    Outgoing::form(
        format!("{IDEOGRAM}/v1/ideogram-v3/generate"),
        Auth::Header("Api-Key"),
        fields,
    )
}

/// Ideogram leaves a link to the picture in `data[0].url`. One its safety check caught says
/// `is_image_safe: false` and has no link.
fn read_ideogram(label: &str, body: &Value) -> Result<Answer, AiError> {
    let first = body
        .get("data")
        .and_then(|d| d.as_array())
        .and_then(|a| a.first())
        .ok_or_else(|| unexpected(label, body))?;
    if first.get("is_image_safe").and_then(Value::as_bool) == Some(false) {
        return Err(blocked(label));
    }
    first
        .get("url")
        .and_then(|u| u.as_str())
        .filter(|u| !u.is_empty())
        .map(Answer::link)
        .ok_or_else(|| blocked(label))
}

// ---------------------------------------------------------------- keys

/// The cheap authenticated request that proves a key works without making anything.
pub fn key_check(provider: &str) -> Option<Outgoing> {
    Some(match provider {
        "openai" => Outgoing::get(format!("{OPENAI}/v1/models"), Auth::Bearer),
        "xai" => Outgoing::get(format!("{XAI}/v1/models"), Auth::Bearer),
        "recraft" => Outgoing::get(format!("{RECRAFT}/v1/users/me"), Auth::Bearer),
        "google" => Outgoing::get(
            format!("{GOOGLE}/v1beta/models"),
            Auth::Header("x-goog-api-key"),
        ),
        // The credit balance alone, where /v1/user/account would fetch the account's email.
        "stability" => Outgoing::get(format!("{STABILITY}/v1/user/balance"), Auth::Bearer),
        // The account's credit balance: free, and only answered for a key it knows.
        "bfl" => Outgoing::get(format!("{BFL}/v1/credits"), Auth::Header("x-key")),
        // Ideogram has no "who am I". Describing a picture checks the key before it looks at the
        // picture, so a few bytes that aren't one are turned away once the key is accepted, and
        // nothing is described or charged.
        "ideogram" => Outgoing::form(
            format!("{IDEOGRAM}/describe"),
            Auth::Header("Api-Key"),
            vec![Field::file("image_file", "key-check.png", b"not a picture")],
        ),
        _ => return None,
    })
}

/// Pulls a human-readable message out of whatever error shape a provider uses: OpenAI's and
/// Google's `error.message`, xAI's and Ideogram's `error`, Black Forest Labs' `detail` (a
/// sentence, or a list of what failed validation), Stability's `errors`.
pub fn error_message(body: &str) -> String {
    let Ok(v) = serde_json::from_str::<Value>(body) else {
        // An HTML page from a proxy says nothing worth showing.
        if body.trim_start().starts_with('<') {
            return String::new();
        }
        return crate::error::trim_message(body);
    };
    for path in [
        &["error", "message"][..],
        &["error"],
        &["message"],
        &["detail"],
        &["detail", "0", "msg"],
        &["errors", "0"],
        &["name"],
    ] {
        let mut node = &v;
        let mut found = true;
        for key in path {
            let next = match key.parse::<usize>() {
                Ok(i) => node.get(i),
                Err(_) => node.get(key),
            };
            match next {
                Some(n) => node = n,
                None => {
                    found = false;
                    break;
                }
            }
        }
        if found {
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
    use crate::catalogue;

    /// A request as `plan` makes one: the Mac look's artwork, no pictures, no settings.
    fn req(provider: &str, model: &str) -> GenerateRequest {
        GenerateRequest {
            provider: provider.into(),
            model: model.into(),
            prompt: "a copper patina".into(),
            size: Some("1024x958".into()),
            ..GenerateRequest::default()
        }
    }

    fn model(provider: &str, id: &str) -> &'static ModelInfo {
        catalogue::model(provider, id).unwrap()
    }

    fn built(r: &GenerateRequest) -> Outgoing {
        build(&r.provider, model(&r.provider, &r.model), r)
    }

    fn template() -> Reference {
        Reference::new(Role::Template, b"template".to_vec())
    }

    fn subject() -> Reference {
        Reference::new(Role::Subject, b"subject".to_vec())
    }

    fn ok_json(body: Value) -> Incoming {
        Incoming {
            status: 200,
            headers: vec![("content-type".into(), "application/json".into())],
            body: body.to_string().into_bytes(),
        }
    }

    fn failed(status: u16, body: &str) -> Incoming {
        Incoming {
            status,
            headers: Vec::new(),
            body: body.as_bytes().to_vec(),
        }
    }

    fn b64(bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    /// A few bytes that start like a PNG.
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nrest";

    fn bytes_of(answer: &Answer) -> (&[u8], &str) {
        match &answer.picture {
            Picture::Bytes { bytes, media_type } => (bytes, media_type),
            other => panic!("not the picture itself: {other:?}"),
        }
    }

    // ------------------------------------------------------------ sizes

    #[test]
    fn sizes_go_on_the_grid_and_within_the_limits() {
        assert_eq!(on_grid((1024, 958), 16), (1024, 960), "the Mac look");
        assert_eq!(on_grid((1024, 805), 16), (1024, 800), "the Windows look");
        assert_eq!(on_grid((1166, 1091), 16), (1168, 1088), "a whole folder");
        // Black Forest Labs bills by the started megapixel: a whole folder stays within one.
        let (w, h) = within((1166, 1091), 64 * 64, 1024 * 1024, 16);
        assert!(u64::from(w) * u64::from(h) <= 1024 * 1024, "{w}x{h}");
        assert_eq!((w, h), (1056, 992));
        // Too small for OpenAI grows to its least.
        let (w, h) = within((512, 512), 655_360, 8_294_400, 16);
        assert!(u64::from(w) * u64::from(h) >= 655_360 && w % 16 == 0 && h % 16 == 0);
    }

    #[test]
    fn the_nearest_shape_is_chosen_on_a_log_scale() {
        assert_eq!(nearest((1024, 958), GOOGLE_RATIOS), "1:1");
        assert_eq!(nearest((1024, 805), GOOGLE_RATIOS), "5:4");
        assert_eq!(nearest((1024, 805), XAI_RATIOS), "4:3");
        assert_eq!(nearest((805, 1024), STABILITY_RATIOS), "4:5");
        assert_eq!(nearest((1024, 958), IDEOGRAM_RESOLUTIONS), "1024x960");
        assert_eq!(nearest((1024, 805), IDEOGRAM_RESOLUTIONS), "1152x896");
        let recraft = model("recraft", "recraftv4_1");
        assert_eq!(nearest((1024, 958), recraft.sizes), "1024x1024");
    }

    #[test]
    fn sizes_parse_and_fall_back() {
        let mut r = req("openai", "gpt-image-2.5-flare");
        assert_eq!(dimensions(&r, "1024x1024"), (1024, 958));
        r.size = None;
        assert_eq!(dimensions(&r, "1536x1024"), (1536, 1024));
        r.size = Some("nonsense".into());
        assert_eq!(dimensions(&r, "1024x1024"), (1024, 1024));
        r.size = Some("0x0".into());
        assert_eq!(dimensions(&r, "1024x1024"), (1024, 1024));
    }

    #[test]
    fn a_negative_prompt_keeps_text_out_unless_lettering_is_asked_for() {
        let mut r = req("stability", "ultra");
        r.keep_out = vec!["border".into(), "cartoon".into(), "Border".into()];
        assert_eq!(
            negative_prompt(&r),
            "border, cartoon, text, letters, numbers, watermark, signature"
        );
        r.lettering = Some("ESCAPE".into());
        assert_eq!(negative_prompt(&r), "border, cartoon, watermark, signature");
    }

    // ------------------------------------------------------------ OpenAI

    #[test]
    fn openai_paints_from_words_as_json_at_the_exact_size() {
        let out = built(&req("openai", "gpt-image-2.5-flare"));
        assert_eq!(out.method, Method::Post);
        assert_eq!(out.url, "https://api.openai.com/v1/images/generations");
        assert_eq!(out.auth, Auth::Bearer);
        assert_eq!(out.content_type(), Some("application/json"));
        assert_eq!(
            out.body.json().unwrap(),
            &json!({
                "model": "gpt-image-2.5-flare",
                "prompt": "a copper patina",
                "n": 1,
                "size": "1024x960",
                "quality": "high",
                "output_format": "png",
            })
        );
        // Sunburst at its best, GPT Image 2 at medium; transparency only when it is wanted.
        let mut sunburst = req("openai", "gpt-image-2.5-sunburst");
        sunburst.want_alpha = true;
        let body = built(&sunburst).body;
        assert_eq!(body.json().unwrap()["quality"], "max");
        assert_eq!(body.json().unwrap()["background"], "transparent");
        let two = built(&req("openai", "gpt-image-2")).body;
        assert_eq!(two.json().unwrap()["quality"], "medium");
        assert!(two.json().unwrap().get("background").is_none());
        // What the API doesn't take for these models is never sent.
        for key in ["response_format", "input_fidelity", "style"] {
            assert!(two.json().unwrap().get(key).is_none(), "{key}");
        }
    }

    #[test]
    fn openai_edits_are_a_form_with_every_picture_as_image_brackets_in_role_order() {
        let mut r = req("openai", "gpt-image-2.5-flare");
        r.size = Some("1024x805".into());
        r.want_alpha = true;
        r.references = vec![subject(), template()];
        let out = built(&r);
        assert_eq!(out.url, "https://api.openai.com/v1/images/edits");
        assert_eq!(out.content_type(), Some("multipart/form-data"));
        assert_eq!(
            out.body.names(),
            [
                "model",
                "prompt",
                "n",
                "size",
                "quality",
                "output_format",
                "background",
                "image[]",
                "image[]"
            ]
        );
        assert_eq!(out.body.text("size"), Some("1024x800"));
        assert_eq!(out.body.text("background"), Some("transparent"));
        assert_eq!(
            out.body.files("image[]"),
            [b"template".as_slice(), b"subject".as_slice()],
            "the template first"
        );
    }

    #[test]
    fn openai_replies_are_read_with_what_they_cost() {
        // The documented ImagesResponse example.
        let reply = ok_json(json!({
            "created": 1713833628,
            "data": [{ "b64_json": b64(PNG) }],
            "background": "transparent",
            "output_format": "png",
            "size": "1024x1024",
            "quality": "high",
            "usage": {
                "total_tokens": 100, "input_tokens": 50, "output_tokens": 50,
                "input_tokens_details": { "text_tokens": 10, "image_tokens": 40 }
            }
        }));
        let answer = read("openai", "OpenAI", &reply).unwrap();
        assert_eq!(bytes_of(&answer), (PNG, "image/png"));
        assert_eq!(answer.revised_prompt, None);
        assert_eq!(
            answer.usage.as_deref(),
            Some(
                "OpenAI made it at 1024x1024 and high quality from 50 input tokens and 50 output \
                 tokens, about $0.002"
            )
        );
        assert!(matches!(
            read("openai", "OpenAI", &ok_json(json!({ "created": 1 }))),
            Err(AiError::Decode(m)) if m.contains("without a picture") && m.contains("created")
        ));
    }

    #[test]
    fn openai_failures_read_as_what_they_are() {
        let blocked = failed(
            400,
            r#"{"error":{"type":"image_generation_user_error","code":"moderation_blocked",
               "message":"Your request was rejected as a result of our safety system.","param":null}}"#,
        );
        assert!(matches!(
            read("openai", "OpenAI", &blocked),
            Err(AiError::Refused(_))
        ));
        let unverified = failed(
            403,
            r#"{"error":{"type":"invalid_request_error","code":null,"param":null,
               "message":"Your organization must be verified to use the model `gpt-image-2.5-flare`."}}"#,
        );
        let e = read("openai", "OpenAI", &unverified).unwrap_err();
        assert!(e
            .to_string()
            .starts_with("OpenAI said: Your organization must be verified"));
        let wrong_key = failed(
            401,
            r#"{"error":{"message":"Incorrect API key provided: sk-abc"}}"#,
        );
        assert!(matches!(
            read("openai", "OpenAI", &wrong_key),
            Err(AiError::Unauthorized(_))
        ));
    }

    // ------------------------------------------------------------ xAI

    #[test]
    fn grok_paints_from_words_as_json_with_its_aspect_ratio_pinned() {
        let out = built(&req("xai", "grok-imagine-image-2.0"));
        assert_eq!(out.url, "https://api.x.ai/v1/images/generations");
        assert_eq!(out.auth, Auth::Bearer);
        assert_eq!(
            out.body.json().unwrap(),
            &json!({
                "model": "grok-imagine-image-2.0",
                "prompt": "a copper patina",
                "n": 1,
                "response_format": "b64_json",
                "aspect_ratio": "1:1",
            })
        );
    }

    #[test]
    fn grok_edits_are_json_with_data_urls_never_a_form() {
        let mut r = req("xai", "grok-imagine-image");
        r.references = vec![template()];
        let out = built(&r);
        assert_eq!(out.url, "https://api.x.ai/v1/images/edits");
        assert_eq!(out.content_type(), Some("application/json"));
        let body = out.body.json().unwrap();
        assert_eq!(
            body["image"],
            json!({
                "url": format!("data:image/png;base64,{}", b64(b"template")),
                "type": "image_url",
            })
        );
        assert!(
            body.get("aspect_ratio").is_none(),
            "the template's own frame decides it"
        );
        // A picture of the person's own: the frame is pinned instead.
        r.references = vec![subject()];
        assert_eq!(built(&r).body.json().unwrap()["aspect_ratio"], "1:1");
        // Several pictures go as `images`, the template first.
        r.references = vec![subject(), template()];
        let body = built(&r).body;
        let images = body.json().unwrap()["images"].as_array().unwrap().clone();
        assert_eq!(images.len(), 2);
        assert!(images[0]["url"]
            .as_str()
            .unwrap()
            .ends_with(&b64(b"template")));
        assert!(body.json().unwrap().get("image").is_none());
    }

    #[test]
    fn grok_replies_are_read_with_their_type_and_exact_cost() {
        let reply = ok_json(json!({
            "data": [{ "b64_json": b64(b"\xFF\xD8\xFFjpeg"), "mime_type": "image/jpeg" }],
            "usage": { "cost_in_usd_ticks": 400_000_000u64 }
        }));
        let answer = read("xai", "xAI Grok", &reply).unwrap();
        assert_eq!(
            bytes_of(&answer),
            (b"\xFF\xD8\xFFjpeg".as_slice(), "image/jpeg")
        );
        assert_eq!(answer.usage.as_deref(), Some("xAI Grok charged $0.040"));
        // The documented edit reply leaves a link.
        let reply = ok_json(json!({ "data": [{ "url": "https://x.test/a.jpg" }] }));
        let link = read("xai", "xAI Grok", &reply).unwrap();
        assert_eq!(link.picture, Picture::Link("https://x.test/a.jpg".into()));
        // Held back by its moderation, which leaves a placeholder in place of the picture.
        let held = json!({ "data": [{ "respect_moderation": false, "b64_json": b64(PNG) }] });
        assert!(matches!(
            read("xai", "xAI Grok", &ok_json(held)),
            Err(AiError::Refused(_))
        ));
        // No picture and no reason: one to try again.
        for empty in [
            json!({ "data": [] }),
            json!({ "data": [{ "b64_json": "" }] }),
        ] {
            assert!(
                matches!(
                    read("xai", "xAI Grok", &ok_json(empty.clone())),
                    Err(AiError::NoImage(_))
                ),
                "{empty}"
            );
        }
        let invalid = failed(
            422,
            r#"{"code":"invalid-argument","error":"aspect_ratio must be one of the supported values"}"#,
        );
        assert_eq!(
            read("xai", "xAI Grok", &invalid).unwrap_err().to_string(),
            "xAI Grok said: aspect_ratio must be one of the supported values (error 422)"
        );
    }

    // ------------------------------------------------------------ Google

    #[test]
    fn gemini_is_asked_for_a_picture_only_at_1k_in_the_nearest_ratio() {
        let out = built(&req("google", "gemini-3.1-flash-image"));
        assert_eq!(
            out.url,
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-3.1-flash-image:generateContent"
        );
        assert_eq!(out.auth, Auth::Header("x-goog-api-key"));
        assert_eq!(
            out.body.json().unwrap(),
            &json!({
                "contents": [{ "parts": [{ "text": "a copper patina" }] }],
                "generationConfig": {
                    "responseModalities": ["IMAGE"],
                    "imageConfig": { "imageSize": "1K", "aspectRatio": "1:1" },
                },
            })
        );
        let mut windows = req("google", "gemini-3-pro-image");
        windows.size = Some("1024x805".into());
        let body = built(&windows).body;
        assert_eq!(
            body.json().unwrap()["generationConfig"]["imageConfig"]["aspectRatio"],
            "5:4"
        );
    }

    #[test]
    fn gemini_takes_the_pictures_after_the_words_and_the_templates_frame() {
        let mut r = req("google", "gemini-3.1-flash-lite-image");
        r.references = vec![subject(), template()];
        let body = built(&r).body;
        let body = body.json().unwrap();
        let parts = body["contents"][0]["parts"].as_array().unwrap();
        assert_eq!(parts[0]["text"], "a copper patina");
        assert_eq!(parts[1]["inlineData"]["mimeType"], "image/png");
        assert_eq!(parts[1]["inlineData"]["data"], b64(b"template"));
        assert_eq!(parts[2]["inlineData"]["data"], b64(b"subject"));
        let config = &body["generationConfig"]["imageConfig"];
        assert!(config.get("aspectRatio").is_none(), "{config}");
        assert_eq!(config["imageSize"], "1K");
    }

    #[test]
    fn gemini_keeps_the_last_picture_that_isnt_a_thought() {
        let body = json!({
            "candidates": [{
                "content": { "parts": [
                    { "thought": true, "inline_data": { "data": b64(b"draft") } },
                    { "text": "Here it is." },
                    { "inline_data": { "data": b64(b"first") } },
                    {
                        "inlineData": { "mimeType": "image/jpeg", "data": b64(b"final") },
                        "thoughtSignature": "sig"
                    },
                    { "thought": true, "inline_data": { "data": b64(b"late draft") } },
                ] },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 300, "candidatesTokenCount": 1120, "thoughtsTokenCount": 250
            }
        });
        let answer = read("google", "Google Gemini", &ok_json(body)).unwrap();
        assert_eq!(bytes_of(&answer), (b"final".as_slice(), "image/jpeg"));
        assert_eq!(
            answer.usage.as_deref(),
            Some(
                "Google Gemini used 300 prompt tokens, 250 thinking tokens and 1120 output tokens"
            )
        );
    }

    #[test]
    fn gemini_says_whether_it_was_blocked_or_made_no_picture() {
        let read_google = |body: Value| read("google", "Google Gemini", &ok_json(body));
        // The prompt itself blocked: no candidates at all.
        assert!(matches!(
            read_google(json!({ "promptFeedback": { "blockReason": "PROHIBITED_CONTENT" } })),
            Err(AiError::Refused(_))
        ));
        for reason in [
            "IMAGE_SAFETY",
            "IMAGE_PROHIBITED_CONTENT",
            "SAFETY",
            "IMAGE_RECITATION",
        ] {
            let body =
                json!({ "candidates": [{ "content": { "parts": [] }, "finishReason": reason }] });
            assert!(
                matches!(read_google(body), Err(AiError::Refused(_))),
                "{reason}"
            );
        }
        // No picture, and nothing wrong with the words: worth trying again.
        for body in [
            json!({ "candidates": [{ "finishReason": "NO_IMAGE" }] }),
            json!({ "candidates": [{
                "content": { "parts": [{ "text": "I can't" }] }, "finishReason": "STOP"
            }] }),
            json!({ "candidates": [{ "content": { "parts": [] }, "finishReason": "IMAGE_OTHER" }] }),
        ] {
            let e = read_google(body.clone()).unwrap_err();
            assert!(matches!(e, AiError::NoImage(_)), "{body}");
            assert_eq!(
                e.to_string(),
                "Google Gemini finished without painting a picture. Try again, or reword the idea."
            );
        }
        // Something else entirely says what it was.
        assert!(matches!(
            read_google(json!({ "surprise": true })),
            Err(AiError::Decode(_))
        ));
        // A key Google doesn't know.
        let bad_key = failed(
            400,
            r#"{"error":{"code":400,"message":"API key not valid. Please pass a valid API key.","status":"INVALID_ARGUMENT"}}"#,
        );
        assert!(matches!(
            read("google", "Google Gemini", &bad_key),
            Err(AiError::Unauthorized(_))
        ));
    }

    // ------------------------------------------------------------ Black Forest Labs

    #[test]
    fn flux_is_asked_at_the_exact_size_with_prompt_rewriting_off() {
        let out = built(&req("bfl", "flux-2-pro"));
        assert_eq!(out.url, "https://api.bfl.ai/v1/flux-2-pro");
        assert_eq!(out.auth, Auth::Header("x-key"));
        assert_eq!(
            out.body.json().unwrap(),
            &json!({
                "prompt": "a copper patina",
                "width": 1024,
                "height": 960,
                "output_format": "png",
                "safety_tolerance": 2,
                "disable_pup": true,
            })
        );
        let flex = built(&req("bfl", "flux-2-flex")).body;
        assert_eq!(flex.json().unwrap()["prompt_upsampling"], false);
        assert!(flex.json().unwrap().get("disable_pup").is_none());
        let klein = built(&req("bfl", "flux-2-klein-4b")).body;
        let klein = klein.json().unwrap();
        assert!(klein.get("disable_pup").is_none() && klein.get("prompt_upsampling").is_none());
    }

    #[test]
    fn flux_takes_its_pictures_as_numbered_base64_and_a_folder_within_a_megapixel() {
        let mut r = req("bfl", "flux-2-max");
        r.size = Some("1166x1091".into());
        r.references = vec![subject(), template()];
        let body = built(&r).body;
        let body = body.json().unwrap();
        assert_eq!(
            (body["width"].as_u64(), body["height"].as_u64()),
            (Some(1056), Some(992))
        );
        assert_eq!(body["input_image"], b64(b"template"));
        assert_eq!(body["input_image_2"], b64(b"subject"));
        assert!(body.get("input_image_3").is_none());
    }

    #[test]
    fn flux_is_waited_for_and_its_states_are_read() {
        let submitted = ok_json(json!({
            "id": "abc",
            "polling_url": "https://api.us1.bfl.ai/v1/get_result?id=abc",
            "cost": 4.5
        }));
        let answer = read("bfl", "Black Forest Labs", &submitted).unwrap();
        let Picture::Wait(wait) = &answer.picture else {
            panic!("{answer:?}");
        };
        assert_eq!(wait.url, "https://api.us1.bfl.ai/v1/get_result?id=abc");
        assert_eq!(wait.poll().auth, Auth::Header("x-key"));
        assert_eq!(wait.poll().method, Method::Get);
        assert_eq!(
            answer.usage.as_deref(),
            Some("Black Forest Labs charged 4.5 credits, $0.045")
        );
        let poll = |body: Value| read_wait("Black Forest Labs", &ok_json(body));
        for pending in ["Pending", "Reasoning", "Generating", "Something newer"] {
            assert_eq!(
                poll(json!({ "status": pending })).unwrap(),
                None,
                "{pending}"
            );
        }
        let ready = json!({
            "status": "Ready",
            "result": { "sample": "https://delivery-us1.bfl.ai/i.png" }
        });
        assert_eq!(
            poll(ready).unwrap().as_deref(),
            Some("https://delivery-us1.bfl.ai/i.png")
        );
        let moderated = json!({
            "status": "Content Moderated",
            "details": { "Moderation Reasons": ["Derivative Works Filter"] }
        });
        assert!(matches!(poll(moderated), Err(AiError::Refused(_))));
        for failed in ["Error", "Failed"] {
            let e = poll(json!({ "status": failed, "details": { "error": "boom" } })).unwrap_err();
            assert_eq!(
                e.to_string(),
                "Black Forest Labs could not finish the image: boom"
            );
        }
        // Its filter, reported as an error.
        assert!(matches!(
            poll(json!({ "status": "Error", "error": "content_policy_violation" })),
            Err(AiError::Refused(_))
        ));
        // A fast model can answer with the picture at once.
        let at_once = read(
            "bfl",
            "Black Forest Labs",
            &ok_json(json!({ "id": "abc", "sample": "https://delivery-eu1.bfl.ai/i.png" })),
        )
        .unwrap();
        assert_eq!(
            at_once.picture,
            Picture::Link("https://delivery-eu1.bfl.ai/i.png".into())
        );
        assert!(matches!(
            poll(json!({ "status": "Task not found" })),
            Err(AiError::Decode(_))
        ));
        assert!(matches!(
            read("bfl", "Black Forest Labs", &ok_json(json!({ "id": "abc" }))),
            Err(AiError::Decode(_))
        ));
        let invalid = failed(
            422,
            r#"{"detail":[{"loc":["body","width"],"msg":"Input should be a multiple of 16","type":"value_error"}]}"#,
        );
        let e = read("bfl", "Black Forest Labs", &invalid).unwrap_err();
        assert!(
            e.to_string().contains("Input should be a multiple of 16"),
            "{e}"
        );
    }

    // ------------------------------------------------------------ Stability AI

    #[test]
    fn stable_image_is_a_form_with_its_ratio_negative_prompt_and_preset() {
        let mut r = req("stability", "ultra");
        r.style_preset = Some("photographic".into());
        let out = built(&r);
        assert_eq!(
            out.url,
            "https://api.stability.ai/v2beta/stable-image/generate/ultra"
        );
        assert_eq!(out.auth, Auth::Bearer);
        assert_eq!(out.accept, Some("image/*"));
        assert_eq!(out.content_type(), Some("multipart/form-data"));
        assert_eq!(
            out.body.names(),
            [
                "prompt",
                "aspect_ratio",
                "negative_prompt",
                "output_format",
                "style_preset"
            ]
        );
        assert_eq!(out.body.text("aspect_ratio"), Some("1:1"));
        assert_eq!(
            out.body.text("negative_prompt"),
            Some("text, letters, numbers, watermark, signature")
        );
        assert_eq!(out.body.text("output_format"), Some("png"));
        assert_eq!(out.body.text("style_preset"), Some("photographic"));
        assert_eq!(out.body.text("mode"), None, "Core and Ultra have no mode");
        // A preset Stability doesn't have is left out, not sent to fail.
        r.style_preset = Some("WATERCOLOR".into());
        r.model = "core".into();
        let out = built(&r);
        assert_eq!(
            out.url,
            "https://api.stability.ai/v2beta/stable-image/generate/core"
        );
        assert_eq!(out.body.text("style_preset"), None);
    }

    #[test]
    fn stable_image_replies_are_the_picture_unless_its_filter_blurred_it() {
        let reply = Incoming {
            status: 200,
            headers: vec![
                ("content-type".into(), "image/png".into()),
                ("finish-reason".into(), "SUCCESS".into()),
                ("seed".into(), "343940597".into()),
            ],
            body: PNG.to_vec(),
        };
        let answer = read("stability", "Stability AI", &reply).unwrap();
        assert_eq!(bytes_of(&answer), (PNG, "image/png"));
        let blurred = Incoming {
            headers: vec![("finish-reason".into(), "CONTENT_FILTERED".into())],
            ..reply
        };
        assert!(matches!(
            read("stability", "Stability AI", &blurred),
            Err(AiError::Refused(_))
        ));
        // A 403 is its moderation, not the key.
        let flagged = failed(
            403,
            r#"{"id":"a1","name":"content_moderation","errors":["Your request was flagged by our content moderation system."]}"#,
        );
        let e = read("stability", "Stability AI", &flagged).unwrap_err();
        assert!(matches!(e, AiError::Refused(_)));
        assert!(e
            .to_string()
            .contains("flagged by our content moderation system"));
        let wrong_key = failed(
            401,
            r#"{"id":"a2","name":"unauthorized","errors":["invalid key"]}"#,
        );
        assert!(matches!(
            read("stability", "Stability AI", &wrong_key),
            Err(AiError::Unauthorized(_))
        ));
        // Stability names its failures: out of credits is neither moderation nor the key.
        let broke = failed(
            403,
            r#"{"id":"a3","name":"payment_required","errors":["lacking credits"]}"#,
        );
        assert_eq!(
            read("stability", "Stability AI", &broke)
                .unwrap_err()
                .to_string(),
            "Stability AI said: lacking credits (error 403)"
        );
        // A 403 that names nothing is its moderation, as its reference says.
        assert!(matches!(
            read("stability", "Stability AI", &failed(403, "")),
            Err(AiError::Refused(_))
        ));
    }

    // ------------------------------------------------------------ Recraft

    #[test]
    fn recraft_is_asked_for_a_png_at_its_nearest_size_and_told_the_backdrop() {
        let mut r = req("recraft", "recraftv4_1");
        r.key_colour = Some(crate::finish::GREEN);
        r.style_preset = Some("Photorealism".into());
        r.keep_out = vec!["cartoon".into()];
        let out = built(&r);
        assert_eq!(
            out.url,
            "https://external.api.recraft.ai/v1/images/generations"
        );
        assert_eq!(out.auth, Auth::Bearer);
        assert_eq!(
            out.body.json().unwrap(),
            &json!({
                "model": "recraftv4_1",
                "prompt": "a copper patina",
                "size": "1024x1024",
                "response_format": "b64_json",
                "image_format": "png",
                "controls": { "background_color": { "rgb": [0, 255, 0] } },
            }),
            "V4.1 takes no style name, negative prompt or transparent_background"
        );
        r.key_colour = None;
        r.size = Some("1024x805".into());
        let body = built(&r).body;
        assert!(body.json().unwrap().get("controls").is_none());
        assert_eq!(body.json().unwrap()["size"], "1152x896");
    }

    #[test]
    fn recraft_replies_are_read_with_the_units_they_cost() {
        let reply = ok_json(json!({
            "created": 1, "credits": 35,
            "data": [{ "image_id": "i", "b64_json": b64(PNG) }]
        }));
        let answer = read("recraft", "Recraft", &reply).unwrap();
        assert_eq!(bytes_of(&answer), (PNG, "image/png"));
        assert_eq!(
            answer.usage.as_deref(),
            Some("Recraft charged 35 units, $0.035")
        );
        let reply = ok_json(json!({ "data": [{
            "url": "https://img.recraft.ai/a", "revised_prompt": "a copper patina, detailed"
        }] }));
        let link = read("recraft", "Recraft", &reply).unwrap();
        assert_eq!(
            link.picture,
            Picture::Link("https://img.recraft.ai/a".into())
        );
        assert_eq!(
            link.revised_prompt.as_deref(),
            Some("a copper patina, detailed")
        );
    }

    // ------------------------------------------------------------ Ideogram

    #[test]
    fn ideogram_is_a_form_with_magic_prompt_off_and_its_resolution() {
        let mut r = req("ideogram", "V_3");
        r.style_preset = Some("WATERCOLOR".into());
        let out = built(&r);
        assert_eq!(out.url, "https://api.ideogram.ai/v1/ideogram-v3/generate");
        assert_eq!(out.auth, Auth::Header("Api-Key"));
        assert_eq!(out.content_type(), Some("multipart/form-data"));
        assert_eq!(
            out.body.names(),
            [
                "prompt",
                "rendering_speed",
                "magic_prompt",
                "num_images",
                "resolution",
                "negative_prompt",
                "style_preset"
            ]
        );
        assert_eq!(out.body.text("rendering_speed"), Some("DEFAULT"));
        assert_eq!(out.body.text("magic_prompt"), Some("OFF"));
        assert_eq!(out.body.text("num_images"), Some("1"));
        assert_eq!(out.body.text("resolution"), Some("1024x960"));
        assert_eq!(out.body.text("style_preset"), Some("WATERCOLOR"));
        assert_eq!(
            out.body.text("aspect_ratio"),
            None,
            "never with a resolution"
        );
        // A style type sets the type instead.
        r.style_preset = Some("REALISTIC".into());
        let out = built(&r);
        assert_eq!(out.body.text("style_type"), Some("REALISTIC"));
        assert_eq!(out.body.text("style_preset"), None);
    }

    #[test]
    fn ideogram_replies_leave_a_link_unless_its_safety_check_held_the_picture() {
        // The documented example.
        let reply = ok_json(json!({
            "created": "2000-01-23T04:56:07.000Z",
            "data": [{
                "prompt": "a copper patina", "resolution": "1024x960", "is_image_safe": true,
                "seed": 12345, "style_type": "GENERAL",
                "url": "https://ideogram.ai/api/images/ephemeral/xtdZiqPwRxqY1Y7NExFmzB.png?exp=1&sig=2"
            }]
        }));
        let answer = read("ideogram", "Ideogram", &reply).unwrap();
        assert!(
            matches!(&answer.picture, Picture::Link(u) if u.contains("ideogram.ai/api/images"))
        );
        let held = ok_json(json!({ "data": [{ "is_image_safe": false, "url": null }] }));
        assert!(matches!(
            read("ideogram", "Ideogram", &held),
            Err(AiError::Refused(_))
        ));
        let refused = failed(422, r#"{"error":"Prompt failed the safety check."}"#);
        assert_eq!(
            read("ideogram", "Ideogram", &refused)
                .unwrap_err()
                .to_string(),
            "Ideogram declined that prompt: Prompt failed the safety check."
        );
    }

    // ------------------------------------------------------------ keys and errors

    #[test]
    fn every_key_is_checked_where_it_costs_nothing() {
        let check = |p: &str| key_check(p).unwrap();
        assert_eq!(check("openai").url, "https://api.openai.com/v1/models");
        assert_eq!(check("xai").url, "https://api.x.ai/v1/models");
        assert_eq!(
            check("recraft").url,
            "https://external.api.recraft.ai/v1/users/me"
        );
        assert_eq!(
            check("google"),
            Outgoing::get(
                "https://generativelanguage.googleapis.com/v1beta/models".into(),
                Auth::Header("x-goog-api-key")
            )
        );
        assert_eq!(
            check("stability").url,
            "https://api.stability.ai/v1/user/balance"
        );
        assert_eq!(check("bfl").auth, Auth::Header("x-key"));
        let ideogram = check("ideogram");
        assert_eq!(ideogram.url, "https://api.ideogram.ai/describe");
        assert_eq!(
            ideogram.body.files("image_file"),
            [b"not a picture".as_slice()]
        );
        assert!(key_check("nope").is_none());
        for p in catalogue::providers() {
            let out = check(p.id);
            assert!(
                PROVIDER_ORIGINS.iter().any(|o| out.url.starts_with(o)),
                "{}",
                p.id
            );
        }
    }

    #[test]
    fn error_messages_are_pulled_from_each_provider_shape() {
        assert_eq!(
            error_message(r#"{"error":{"message":"bad key"}}"#),
            "bad key"
        );
        assert_eq!(error_message(r#"{"code":"x","error":"nope"}"#), "nope");
        assert_eq!(
            error_message(r#"{"detail":"missing field"}"#),
            "missing field"
        );
        assert_eq!(
            error_message(
                r#"{"detail":[{"loc":["body"],"msg":"field required","type":"missing"}]}"#
            ),
            "field required"
        );
        assert_eq!(
            error_message(r#"{"id":"1","name":"bad_request","errors":["prompt: is required"]}"#),
            "prompt: is required"
        );
        assert_eq!(error_message("plain text"), "plain text");
        assert_eq!(
            error_message("<html><body>502 Bad Gateway</body></html>"),
            ""
        );
    }

    #[test]
    fn a_reply_that_isnt_json_is_quoted_without_the_picture_data() {
        let noise = format!("oops {} tail", "A".repeat(500));
        let reply = Incoming {
            status: 200,
            headers: Vec::new(),
            body: noise.into_bytes(),
        };
        assert_eq!(
            read("openai", "OpenAI", &reply).unwrap_err().to_string(),
            "OpenAI replied with something FolderSkin could not read: oops … tail"
        );
    }
}
