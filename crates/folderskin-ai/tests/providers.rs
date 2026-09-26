//! Every provider's request, sent for real by `generate_with` to a stand-in on this computer
//! that answers the way the provider's documentation says the real one does. What arrived is
//! checked against the provider's API reference: the method, the path, the header the key goes
//! in, the content type and the fields. No provider is called and no key is used.

mod standin;

use base64::Engine;
use folderskin_ai::finish::GREEN;
use folderskin_ai::prompts::Shape;
use folderskin_ai::{generate_with, plan, test_key_with, AiError, GenerateRequest, Hosts};
use folderskin_core::matte::MAGENTA;
use serde_json::{json, Value};
use standin::{run, serve, Answer, Log, Seen};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Not a key: the stand-in only checks it arrives in the right header.
const KEY: &str = "stand-in-key-0123456789";

fn picture() -> Vec<u8> {
    folderskin_core::raster::encode_png(&image::RgbaImage::from_pixel(
        4,
        4,
        image::Rgba([10, 20, 30, 255]),
    ))
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn unb64(text: &str) -> Vec<u8> {
    let text = text.split_once("base64,").map_or(text, |(_, rest)| rest);
    base64::engine::general_purpose::STANDARD
        .decode(text)
        .unwrap()
}

/// The request the app makes for `shape` of the Mac's folder from `model`, with nothing attached.
fn planned(provider: &str, model: &str, shape: Shape) -> GenerateRequest {
    let info = folderskin_ai::model(provider, model).unwrap();
    let brief = folderskin_ai::Brief {
        idea: "a lighthouse in fog",
        base: &folderskin_core::base::MAC_FOLDER,
        shape,
        treatment: None,
        pictures: Vec::new(),
    };
    plan(provider, info, &brief, None).request
}

fn seen(log: &Log) -> Vec<Seen> {
    log.lock().unwrap().clone()
}

/// A reference picture as it arrived, with its top-left pixel: the template sits on the key.
fn corner(png: &[u8]) -> [u8; 4] {
    image::load_from_memory(png)
        .unwrap()
        .to_rgba8()
        .get_pixel(0, 0)
        .0
}

#[test]
fn openai_generations_go_as_json_and_edits_as_a_form_of_image_parts() {
    let png = picture();
    let reply = json!({
        "created": 1713833628,
        "data": [{ "b64_json": b64(&png) }],
        "size": "1024x960",
        "quality": "high",
        "usage": { "input_tokens": 60, "output_tokens": 1610,
                   "input_tokens_details": { "text_tokens": 60, "image_tokens": 0 } }
    });
    let (base, log) = serve(move |_, _| Answer::json(200, reply.clone()));
    let hosts = Hosts::at(&base);

    let art = planned("openai", "gpt-image-2.5-flare", Shape::Skin);
    let made = run(generate_with(&art, KEY, &hosts)).unwrap();
    assert_eq!(made.image, png);
    assert_eq!(made.model_used, "gpt-image-2.5-flare");
    assert!(made.usage.unwrap().contains("at 1024x960 and high quality"));

    let folder = planned("openai", "gpt-image-2.5-flare", Shape::Folder);
    run(generate_with(&folder, KEY, &hosts)).unwrap();

    let [generation, edit] = seen(&log).try_into().unwrap();
    assert_eq!(
        (generation.method.as_str(), generation.path.as_str()),
        ("POST", "/v1/images/generations")
    );
    assert_eq!(
        generation.header("authorization"),
        Some(format!("Bearer {KEY}").as_str())
    );
    let body = generation.json();
    assert_eq!(body["model"], "gpt-image-2.5-flare");
    assert_eq!(body["size"], "1024x960");
    assert_eq!(body["quality"], "high");
    assert_eq!(body["output_format"], "png");
    assert_eq!(body["n"], 1);

    assert_eq!(
        (edit.method.as_str(), edit.path.as_str()),
        ("POST", "/v1/images/edits")
    );
    let parts = edit.form();
    let names: Vec<&str> = parts.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "model",
            "prompt",
            "n",
            "size",
            "quality",
            "output_format",
            "image[]"
        ]
    );
    // The Mac's whole folder, in its own shape.
    assert_eq!(edit.field("size").as_deref(), Some("1152x1088"));
    let template = &parts[6];
    assert_eq!(template.file_name.as_deref(), Some("image-1.png"));
    assert_eq!(template.content_type.as_deref(), Some("image/png"));
    assert_eq!(corner(&template.bytes), [255, 0, 255, 255], "on magenta");
}

#[test]
fn grok_edits_go_as_json_with_the_picture_as_a_data_url() {
    let png = picture();
    let reply = json!({
        "data": [{ "b64_json": b64(&png), "mime_type": "image/png" }],
        "usage": { "cost_in_usd_ticks": 400_000_000u64 }
    });
    let (base, log) = serve(move |_, _| Answer::json(200, reply.clone()));
    let hosts = Hosts::at(&base);

    let folder = planned("xai", "grok-imagine-image-2.0", Shape::Folder);
    let made = run(generate_with(&folder, KEY, &hosts)).unwrap();
    assert_eq!(made.image, png);
    assert_eq!(made.usage.as_deref(), Some("xAI Grok charged $0.040"));
    let art = planned("xai", "grok-imagine-image", Shape::Skin);
    run(generate_with(&art, KEY, &hosts)).unwrap();

    let [edit, generation] = seen(&log).try_into().unwrap();
    assert_eq!(edit.path, "/v1/images/edits");
    assert_eq!(
        edit.header("authorization"),
        Some(format!("Bearer {KEY}").as_str())
    );
    let body = edit.json();
    assert_eq!(body["model"], "grok-imagine-image-2.0");
    assert_eq!(body["response_format"], "b64_json");
    assert_eq!(body["image"]["type"], "image_url");
    let url = body["image"]["url"].as_str().unwrap();
    assert!(url.starts_with("data:image/png;base64,"));
    assert_eq!(corner(&unb64(url)), [255, 0, 255, 255]);
    assert!(body.get("aspect_ratio").is_none());

    assert_eq!(generation.path, "/v1/images/generations");
    assert_eq!(generation.json()["aspect_ratio"], "1:1");
}

#[test]
fn gemini_gets_its_output_settings_and_a_template_on_green() {
    let png = picture();
    let reply = json!({
        "candidates": [{
            "content": { "parts": [
                { "thought": true, "inlineData": { "mimeType": "image/png", "data": b64(b"draft") } },
                { "inlineData": { "mimeType": "image/png", "data": b64(&png) }, "thoughtSignature": "s" }
            ] },
            "finishReason": "STOP"
        }]
    });
    let (base, log) = serve(move |_, _| Answer::json(200, reply.clone()));
    let folder = planned("google", "gemini-3.1-flash-image", Shape::Folder);
    let made = run(generate_with(&folder, KEY, &Hosts::at(&base))).unwrap();
    assert_eq!(made.image, png, "the last picture that isn't a thought");
    assert_eq!(made.key_colour, Some(GREEN));

    let [asked] = seen(&log).try_into().unwrap();
    assert_eq!(
        asked.path,
        "/v1beta/models/gemini-3.1-flash-image:generateContent"
    );
    assert_eq!(asked.header("x-goog-api-key"), Some(KEY));
    assert_eq!(asked.header("authorization"), None);
    let body = asked.json();
    let parts = body["contents"][0]["parts"].as_array().unwrap();
    // The template's backdrop is kept, never named: a model told a colour paints with it.
    let prompt = parts[0]["text"].as_str().unwrap();
    assert!(
        !prompt.contains("#00FF00") && !prompt.contains("#FF00FF"),
        "{prompt}"
    );
    assert_eq!(parts[1]["inlineData"]["mimeType"], "image/png");
    assert_eq!(
        corner(&unb64(parts[1]["inlineData"]["data"].as_str().unwrap())),
        [0, 255, 0, 255],
        "the template sits on green"
    );
    assert_eq!(
        body["generationConfig"],
        json!({ "responseModalities": ["IMAGE"], "imageConfig": { "imageSize": "1K" } })
    );
}

#[test]
fn gemini_without_a_picture_is_worth_trying_again() {
    let reply = json!({ "candidates": [{ "finishReason": "NO_IMAGE" }] });
    let (base, _) = serve(move |_, _| Answer::json(200, reply.clone()));
    let art = planned("google", "gemini-3-pro-image", Shape::Skin);
    let e = run(generate_with(&art, KEY, &Hosts::at(&base))).unwrap_err();
    assert!(matches!(e, AiError::NoImage(_)), "{e:?}");
}

#[test]
fn flux_is_submitted_waited_for_and_collected_without_the_key() {
    let png = picture();
    let polls = Arc::new(AtomicUsize::new(0));
    let (sample, count) = (png.clone(), Arc::clone(&polls));
    let (base, log) = serve(move |seen, base| match seen.path.as_str() {
        "/v1/flux-2-pro" => Answer::json(
            200,
            json!({
                "id": "abc",
                "polling_url": format!("{base}/v1/get_result?id=abc"),
                "cost": 4.5
            }),
        ),
        "/v1/get_result?id=abc" if count.fetch_add(1, Ordering::SeqCst) == 0 => {
            Answer::json(200, json!({ "id": "abc", "status": "Pending" }))
        }
        "/v1/get_result?id=abc" => Answer::json(
            200,
            json!({
                "id": "abc",
                "status": "Ready",
                "result": { "sample": format!("{base}/delivery/sample.png") }
            }),
        ),
        _ => Answer::bytes("image/png", sample.clone()),
    });
    let folder = planned("bfl", "flux-2-pro", Shape::Folder);
    let made = run(generate_with(&folder, KEY, &Hosts::at(&base))).unwrap();
    assert_eq!(made.image, png);
    assert_eq!(
        made.usage.as_deref(),
        Some("Black Forest Labs charged 4.5 credits, $0.045")
    );

    let [submit, first, second, download] = seen(&log).try_into().unwrap();
    assert_eq!(
        (submit.method.as_str(), submit.path.as_str()),
        ("POST", "/v1/flux-2-pro")
    );
    assert_eq!(submit.header("x-key"), Some(KEY));
    let body = submit.json();
    assert_eq!(
        (body["width"].as_u64(), body["height"].as_u64()),
        (Some(1056), Some(992))
    );
    assert_eq!(body["disable_pup"], true);
    assert_eq!(body["output_format"], "png");
    assert_eq!(body["safety_tolerance"], 2);
    assert_eq!(
        corner(&unb64(body["input_image"].as_str().unwrap())),
        [255, 0, 255, 255]
    );
    for poll in [&first, &second] {
        assert_eq!(
            (poll.method.as_str(), poll.header("x-key")),
            ("GET", Some(KEY))
        );
    }
    assert_eq!(download.path, "/delivery/sample.png");
    assert_eq!(
        download.header("x-key"),
        None,
        "the picture's link needs no key"
    );
    assert_eq!(download.header("authorization"), None);
}

#[test]
fn stable_image_goes_as_a_form_and_a_blurred_picture_is_refused() {
    let png = picture();
    let filtered = Arc::new(AtomicUsize::new(0));
    let (bytes, count) = (png.clone(), Arc::clone(&filtered));
    let (base, log) = serve(move |_, _| {
        let reason = if count.fetch_add(1, Ordering::SeqCst) == 0 {
            "SUCCESS"
        } else {
            "CONTENT_FILTERED"
        };
        Answer::bytes("image/png", bytes.clone())
            .header("finish-reason", reason)
            .header("seed", "343940597")
    });
    let mut art = planned("stability", "ultra", Shape::Skin);
    art.style_preset = Some("photographic".into());
    art.keep_out = vec!["cartoon".into()];
    let made = run(generate_with(&art, KEY, &Hosts::at(&base))).unwrap();
    assert_eq!((made.image, made.media_type.as_str()), (png, "image/png"));
    let blurred = run(generate_with(&art, KEY, &Hosts::at(&base))).unwrap_err();
    assert!(matches!(blurred, AiError::Refused(_)), "{blurred:?}");

    let asked = &seen(&log)[0];
    assert_eq!(asked.path, "/v2beta/stable-image/generate/ultra");
    assert_eq!(asked.header("accept"), Some("image/*"));
    assert_eq!(
        asked.header("authorization"),
        Some(format!("Bearer {KEY}").as_str())
    );
    assert_eq!(asked.field("aspect_ratio").as_deref(), Some("1:1"));
    assert_eq!(
        asked.field("negative_prompt").as_deref(),
        Some("cartoon, text, letters, numbers, watermark, signature")
    );
    assert_eq!(asked.field("style_preset").as_deref(), Some("photographic"));
    assert_eq!(asked.field("output_format").as_deref(), Some("png"));
    assert_eq!(asked.field("mode"), None);
}

#[test]
fn recraft_is_told_the_backdrop_it_paints_on() {
    let png = picture();
    let reply = json!({ "created": 1, "credits": 35, "data": [{ "image_id": "i", "b64_json": b64(&png) }] });
    let (base, log) = serve(move |_, _| Answer::json(200, reply.clone()));
    let folder = planned("recraft", "recraftv4_1", Shape::Folder);
    let made = run(generate_with(&folder, KEY, &Hosts::at(&base))).unwrap();
    assert_eq!(made.image, png);
    assert!(
        !made.native_alpha,
        "cut out of its backdrop, not given alpha"
    );

    let [asked] = seen(&log).try_into().unwrap();
    assert_eq!(asked.path, "/v1/images/generations");
    let body = asked.json();
    assert_eq!(
        (&body["model"], &body["size"], &body["image_format"]),
        (&json!("recraftv4_1"), &json!("1024x1024"), &json!("png"))
    );
    assert_eq!(
        body["controls"]["background_color"]["rgb"],
        json!(MAGENTA.to_vec())
    );
    for gone in [
        "style",
        "substyle",
        "transparent_background",
        "negative_prompt",
    ] {
        assert!(body.get(gone).is_none(), "{gone}");
    }
}

#[test]
fn ideogram_goes_as_a_form_and_its_link_is_collected_without_the_key() {
    let png = picture();
    let bytes = png.clone();
    let (base, log) = serve(move |seen, base| match seen.path.as_str() {
        "/v1/ideogram-v3/generate" => Answer::json(
            200,
            json!({
                "created": "2000-01-23T04:56:07.000Z",
                "data": [{
                    "prompt": "a lighthouse in fog", "resolution": "1024x960",
                    "is_image_safe": true, "seed": 12345, "style_type": "GENERAL",
                    "url": format!("{base}/api/images/ephemeral/a.png?exp=1&sig=2")
                }]
            }),
        ),
        _ => Answer::bytes("image/png", bytes.clone()),
    });
    let art = planned("ideogram", "V_3", Shape::Skin);
    let made = run(generate_with(&art, KEY, &Hosts::at(&base))).unwrap();
    assert_eq!(made.image, png);

    let [asked, download] = seen(&log).try_into().unwrap();
    assert_eq!(asked.header("api-key"), Some(KEY));
    assert_eq!(asked.content_type(), "multipart/form-data");
    assert_eq!(asked.field("magic_prompt").as_deref(), Some("OFF"));
    assert_eq!(asked.field("resolution").as_deref(), Some("1024x960"));
    assert_eq!(asked.field("rendering_speed").as_deref(), Some("DEFAULT"));
    assert_eq!(asked.field("num_images").as_deref(), Some("1"));
    // The idea word for word, whatever the capital it starts a sentence with.
    assert!(asked
        .field("prompt")
        .unwrap()
        .to_lowercase()
        .contains("a lighthouse in fog"));
    assert_eq!(download.header("api-key"), None);
}

#[test]
fn a_provider_that_says_no_is_quoted_in_its_own_words() {
    let (base, _) = serve(|_, _| {
        Answer::json(
            422,
            json!({ "code": "invalid-argument", "error": "aspect_ratio must be one of the supported values" }),
        )
    });
    let art = planned("xai", "grok-imagine-image-2.0", Shape::Skin);
    let e = run(generate_with(&art, KEY, &Hosts::at(&base))).unwrap_err();
    assert_eq!(
        e.to_string(),
        "xAI Grok said: aspect_ratio must be one of the supported values (error 422)"
    );
}

#[test]
fn every_key_is_checked_at_its_providers_own_address_and_header() {
    let (base, log) = serve(|seen, _| {
        if seen.path == "/describe" {
            // Ideogram turns the picture away once the key is accepted.
            Answer::json(400, json!({ "error": "invalid image" }))
        } else {
            Answer::json(200, json!({}))
        }
    });
    let hosts = Hosts::at(&base);
    let ids: Vec<&str> = folderskin_ai::providers().iter().map(|p| p.id).collect();
    for id in &ids {
        run(test_key_with(id, KEY, &hosts)).unwrap_or_else(|e| panic!("{id}: {e}"));
    }
    let checks = seen(&log);
    let bearer = format!("Bearer {KEY}");
    let expected: Vec<(&str, &str, &str, &str)> = vec![
        ("GET", "/v1/models", "authorization", &bearer),
        ("GET", "/v1/models", "authorization", &bearer),
        ("GET", "/v1/users/me", "authorization", &bearer),
        ("GET", "/v1beta/models", "x-goog-api-key", KEY),
        ("GET", "/v1/credits", "x-key", KEY),
        ("GET", "/v1/user/balance", "authorization", &bearer),
        ("POST", "/describe", "api-key", KEY),
    ];
    assert_eq!(
        ids,
        [
            "openai",
            "xai",
            "recraft",
            "google",
            "bfl",
            "stability",
            "ideogram"
        ]
    );
    assert_eq!(checks.len(), expected.len());
    for (check, (method, path, header, value)) in checks.iter().zip(&expected) {
        assert_eq!(
            (check.method.as_str(), check.path.as_str()),
            (*method, *path)
        );
        assert_eq!(check.header(header), Some(*value), "{path}");
    }
    let describe: &Seen = &checks[6];
    let parts = describe.form();
    assert_eq!(parts[0].name, "image_file");
    assert_eq!(parts[0].bytes, b"not a picture");
}

/// The stand-in answers what the documentation says: this checks the stand-in itself reads a
/// JSON body the way a server does.
#[test]
fn the_stand_in_reads_what_it_is_sent() {
    let (base, log) = serve(|_, _| Answer::json(200, json!({ "data": [] })));
    let art = planned("xai", "grok-imagine-image", Shape::Skin);
    // No picture and no reason: one to try again.
    let e = run(generate_with(&art, KEY, &Hosts::at(&base))).unwrap_err();
    assert!(matches!(e, AiError::NoImage(_)));
    let body: Value = seen(&log)[0].json();
    assert_eq!(body["prompt"], json!(art.prompt));
}
