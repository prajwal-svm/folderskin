//! Everything around one request that isn't the network: what to ask for, and what to keep of
//! the answer. The app and the `folderskin` command line both go through here, so a skin made
//! with a key comes out the same from either.

use crate::prompts::{self, Shape};
use crate::{AiError, GenerateRequest, GenerateResult, ModelInfo, Reference, Role};
use folderskin_core::compositor::{self, SKIN_HEIGHT, SKIN_WIDTH};
use folderskin_core::matte::{self, KeyOptions, MAGENTA};
use image::RgbaImage;

/// Size the folder shape is rendered at before it is cut out (the folder's own aspect).
pub const FOLDER_W: u32 = 1166;
pub const FOLDER_H: u32 = 1091;
/// The magenta key as the prompts name it; [`MAGENTA`] is the same colour as pixels.
pub const KEY_HEX: &str = "#FF00FF";
/// The green key, for the providers that leave a dark rim around a subject on magenta
/// ([`crate::ProviderInfo::key_colour`]).
pub const GREEN: [u8; 3] = [0, 255, 0];
/// Reference pictures are downscaled before upload; models do not need more and it keeps the
/// request small.
pub const REFERENCE_MAX_SIDE: u32 = 1024;

/// The request for `idea` as `shape` from `model`, with the user's own reference picture if they
/// gave one (already made into a PNG by [`reference_png`]).
///
/// A whole folder from a model that can work from a picture, with none attached, sends our own
/// blank template, so the model repaints FolderSkin's folder instead of inventing one. A folder
/// render needs transparency: the model's own alpha when it has one, otherwise a flat backdrop in
/// the provider's key colour, cut out afterwards. The template sits on that colour and the prompt
/// says to keep it, so a template run is always keyed. Draws the template when it needs one, so
/// call it off the UI thread.
///
/// The picture is asked for at the shape's own size, as a parameter; `size` is one the caller
/// insists on instead.
pub fn plan(
    provider: &str,
    model: &ModelInfo,
    shape: Shape,
    idea: &str,
    size: Option<String>,
    reference_png: Option<Vec<u8>>,
) -> GenerateRequest {
    let reference_png = reference_png.filter(|_| model.accepts_reference);
    let use_template = shape == Shape::Folder && model.accepts_reference && reference_png.is_none();
    let wants_cutout = shape == Shape::Folder;
    let want_alpha = wants_cutout && model.native_alpha && !use_template;
    let key = key_colour(provider);
    let key_hex = hex(key);
    let keyed = wants_cutout && !want_alpha;
    let (width, height) = match shape {
        Shape::Skin => (SKIN_WIDTH, SKIN_HEIGHT),
        Shape::Folder => (FOLDER_W, FOLDER_H),
    };
    let key_named = keyed.then_some(key_hex.as_str());
    let (references, prompt) = if let Some(png) = reference_png {
        let prompt = prompts::compose_with_reference(shape, idea, key_named);
        (vec![Reference::new(Role::Subject, png)], prompt)
    } else if use_template {
        let prompt = prompts::compose_on_template(idea, &key_hex);
        // Drawn at the size every reference picture is sent at, which keeps it within the
        // megapixel the providers that bill by the megapixel charge for.
        let (w, h) = fitted((FOLDER_W, FOLDER_H), REFERENCE_MAX_SIDE);
        let template = template_reference(w, h, key);
        (vec![Reference::new(Role::Template, template)], prompt)
    } else {
        (Vec::new(), prompts::compose(shape, idea, key_named))
    };
    GenerateRequest {
        provider: provider.to_string(),
        model: model.id.to_string(),
        prompt,
        references,
        size: size.or_else(|| Some(format!("{width}x{height}"))),
        want_alpha,
        key_colour: keyed.then_some(key),
        ..GenerateRequest::default()
    }
}

/// The colour `provider` is asked to paint a backdrop in when FolderSkin cuts it out: magenta,
/// or green for a provider that leaves a dark rim on magenta.
pub fn key_colour(provider: &str) -> [u8; 3] {
    crate::provider(provider).map_or(MAGENTA, |p| p.key_colour)
}

/// `colour` as a prompt names it: "#FF00FF".
pub fn hex([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02X}{g:02X}{b:02X}")
}

/// A reference picture as the modest PNG that is uploaded.
pub fn reference_png(img: RgbaImage) -> Vec<u8> {
    folderskin_core::raster::encode_png(&shrink_to(img, REFERENCE_MAX_SIDE))
}

/// Our blank folder template, centred on the flat `key` colour, as the PNG a model is asked to
/// repaint.
pub fn template_reference(width: u32, height: u32, key: [u8; 3]) -> Vec<u8> {
    folderskin_core::raster::encode_png(&compositor::blank_template(width, height, key))
}

/// `(w, h)` scaled down, keeping its shape, until its longer side is at most `max_side`.
fn fitted((w, h): (u32, u32), max_side: u32) -> (u32, u32) {
    let longest = w.max(h);
    if longest <= max_side {
        return (w, h);
    }
    let s = max_side as f32 / longest as f32;
    (
        ((w as f32 * s).round() as u32).max(1),
        ((h as f32 * s).round() as u32).max(1),
    )
}

/// `img` no longer than `max_side` on its longer side.
fn shrink_to(img: RgbaImage, max_side: u32) -> RgbaImage {
    let (w, h) = fitted(img.dimensions(), max_side);
    if (w, h) == img.dimensions() {
        return img;
    }
    image::imageops::resize(&img, w, h, image::imageops::FilterType::Lanczos3)
}

/// What a generated image becomes.
#[derive(Clone, Debug)]
pub enum Finished {
    /// A whole folder, cut out and trimmed to itself: the icon as it is.
    Folder(RgbaImage),
    /// Artwork cropped to the template's aspect (1024 x 958), for FolderSkin's folder.
    Artwork(RgbaImage),
}

/// Makes a provider's image into a skin: a folder is cut out (with the model's own alpha, or by
/// keying the colour it was asked to sit on), artwork is cropped to the template's aspect.
///
/// Magenta is keyed wherever it appears, as it has always been: it almost never belongs to the
/// art. Green does, in every leaf and field, so a green backdrop is only taken away where it
/// reaches the edge of the picture, from the shade of green it was actually painted in, and the
/// greens painted on the folder stay.
pub fn finish(result: &GenerateResult, shape: Shape) -> Result<Finished, AiError> {
    let img = image::load_from_memory(&result.image)
        .map_err(|_| AiError::NotAnImage)?
        .to_rgba8();
    Ok(match shape {
        Shape::Folder if result.native_alpha => Finished::Folder(matte::autocrop(&img, 0)),
        Shape::Folder => {
            let key = result.key_colour.unwrap_or(MAGENTA);
            if !matte::has_key_background(&img, key, KeyOptions::default()) {
                return Err(AiError::NoBackdrop);
            }
            if key == MAGENTA {
                Finished::Folder(matte::cutout(&img, key, KeyOptions::default()))
            } else {
                let painted = matte::flat_backdrop(&img).unwrap_or(key);
                Finished::Folder(matte::cutout_connected(&img, painted))
            }
        }
        Shape::Skin => Finished::Artwork(matte::crop_to_aspect(
            &img,
            SKIN_WIDTH,
            SKIN_HEIGHT,
            (0.5, 0.5),
        )),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn result(img: &RgbaImage, native_alpha: bool) -> GenerateResult {
        GenerateResult {
            image: folderskin_core::raster::encode_png(img),
            media_type: "image/png".into(),
            native_alpha,
            model_used: "m".into(),
            revised_prompt: None,
            key_colour: None,
            usage: None,
        }
    }

    fn model(provider: &str, id: &str) -> &'static ModelInfo {
        crate::model(provider, id).unwrap()
    }

    #[test]
    fn a_folder_from_a_model_that_takes_pictures_repaints_our_template() {
        // Grok takes a picture and has no alpha.
        let req = plan(
            "xai",
            model("xai", "grok-imagine-image-2.0"),
            Shape::Folder,
            "koi",
            None,
            None,
        );
        assert!(req
            .prompt
            .starts_with("The attached image is the exact folder template"));
        assert_eq!(req.references.len(), 1);
        assert_eq!(req.references[0].role, Role::Template);
        let template = image::load_from_memory(&req.references[0].png).unwrap();
        // The folder's own shape, within the megapixel a reference picture is sent at.
        assert_eq!((template.width(), template.height()), (1024, 958));
        assert!(!req.want_alpha, "a template run is keyed");
        assert_eq!(req.key_colour, Some(MAGENTA));
        assert_eq!(req.size.as_deref(), Some("1166x1091"));
    }

    #[test]
    fn gemini_paints_on_green_and_is_cut_out_of_green() {
        let req = plan(
            "google",
            model("google", "gemini-3.1-flash-image"),
            Shape::Folder,
            "koi",
            None,
            None,
        );
        assert_eq!(req.key_colour, Some(GREEN));
        assert!(req.prompt.contains("#00FF00") && !req.prompt.contains("#FF00FF"));
        let template = image::load_from_memory(&req.references[0].png)
            .unwrap()
            .to_rgba8();
        assert_eq!(template.get_pixel(0, 0).0, [0, 255, 0, 255]);
        // A model without pictures names green too.
        let art = plan(
            "google",
            model("google", "gemini-3.1-flash-image"),
            Shape::Skin,
            "koi",
            None,
            None,
        );
        assert_eq!(art.key_colour, None, "artwork is never cut out");
    }

    #[test]
    fn a_folder_from_a_model_with_alpha_asks_for_it_and_names_no_key() {
        // A picture of the person's own on a model with alpha: no template, so real alpha.
        let png = reference_png(RgbaImage::from_pixel(64, 64, Rgba([9, 9, 9, 255])));
        let req = plan(
            "openai",
            model("openai", "gpt-image-2.5-flare"),
            Shape::Folder,
            "koi",
            None,
            Some(png),
        );
        assert!(req.want_alpha);
        assert_eq!(req.key_colour, None);
        assert!(!req.prompt.contains(KEY_HEX));
        assert_eq!(req.references[0].role, Role::Subject);
        // Artwork never asks for alpha or a key, and its size is the template's artwork.
        let art = plan(
            "openai",
            model("openai", "gpt-image-2.5-flare"),
            Shape::Skin,
            "koi",
            None,
            None,
        );
        assert!(!art.want_alpha && !art.prompt.contains(KEY_HEX));
        assert_eq!(art.size.as_deref(), Some("1024x958"));
        assert!(art.references.is_empty());
    }

    #[test]
    fn a_model_without_pictures_or_alpha_is_keyed_and_asked_at_the_shapes_size() {
        let req = plan(
            "recraft",
            model("recraft", "recraftv4_1"),
            Shape::Folder,
            "koi",
            None,
            None,
        );
        assert!(!req.want_alpha && req.references.is_empty());
        assert_eq!(req.key_colour, Some(MAGENTA));
        assert!(req.prompt.contains("pure #FF00FF"));
        // A size the caller insists on wins.
        let fixed = plan(
            "recraft",
            model("recraft", "recraftv4_1"),
            Shape::Skin,
            "koi",
            Some("1216x896".into()),
            None,
        );
        assert_eq!(fixed.size.as_deref(), Some("1216x896"));
    }

    #[test]
    fn a_reference_goes_only_to_a_model_that_takes_one() {
        let png = reference_png(RgbaImage::from_pixel(3000, 1500, Rgba([9, 9, 9, 255])));
        let shrunk = image::load_from_memory(&png).unwrap();
        assert_eq!((shrunk.width(), shrunk.height()), (1024, 512));
        let taken = plan(
            "xai",
            model("xai", "grok-imagine-image"),
            Shape::Skin,
            "x",
            None,
            Some(png.clone()),
        );
        assert!(taken.prompt.starts_with("Use the supplied picture"));
        assert_eq!(taken.references[0].role, Role::Subject);
        let dropped = plan(
            "recraft",
            model("recraft", "recraftv4_1"),
            Shape::Skin,
            "x",
            None,
            Some(png),
        );
        assert!(dropped.references.is_empty());
    }

    #[test]
    fn a_keyed_folder_is_cut_out_and_a_scene_is_refused() {
        let mut keyed = RgbaImage::from_pixel(200, 200, Rgba([255, 0, 255, 255]));
        for y in 50..150 {
            for x in 40..160 {
                keyed.put_pixel(x, y, Rgba([30, 90, 200, 255]));
            }
        }
        match finish(&result(&keyed, false), Shape::Folder).unwrap() {
            Finished::Folder(cut) => assert_eq!(cut.dimensions(), (120, 100)),
            Finished::Artwork(_) => panic!("a folder became artwork"),
        }
        let scene = RgbaImage::from_fn(200, 200, |x, y| Rgba([x as u8, y as u8, 90, 255]));
        let err = finish(&result(&scene, false), Shape::Folder).unwrap_err();
        assert!(matches!(err, AiError::NoBackdrop));
        assert!(err
            .to_string()
            .starts_with("the model drew a scene instead of a folder"));
        match finish(&result(&scene, false), Shape::Skin).unwrap() {
            Finished::Artwork(art) => assert_eq!(art.dimensions(), (SKIN_WIDTH, SKIN_HEIGHT)),
            Finished::Folder(_) => panic!("artwork became a folder"),
        }
        let broken = GenerateResult {
            image: b"not an image".to_vec(),
            ..result(&scene, false)
        };
        assert_eq!(
            finish(&broken, Shape::Skin).unwrap_err().to_string(),
            "the provider returned something that is not an image"
        );
    }

    #[test]
    fn a_green_backdrop_goes_and_the_green_painted_on_the_folder_stays() {
        // A folder painted with a leaf-green meadow, on a backdrop Gemini drew a shade off pure
        // green.
        let backdrop = Rgba([12, 246, 18, 255]);
        let mut img = RgbaImage::from_pixel(200, 200, backdrop);
        for y in 40..160 {
            for x in 30..170 {
                let meadow = if y > 100 {
                    [40, 200, 50, 255]
                } else {
                    [240, 200, 60, 255]
                };
                img.put_pixel(x, y, Rgba(meadow));
            }
        }
        let greened = GenerateResult {
            key_colour: Some(GREEN),
            ..result(&img, false)
        };
        let Finished::Folder(cut) = finish(&greened, Shape::Folder).unwrap() else {
            panic!("a folder became artwork");
        };
        assert_eq!(cut.dimensions(), (140, 120), "the backdrop is gone");
        // The meadow is solid all through, however close its green is to the key.
        assert_eq!(cut.get_pixel(70, 100).0, [40, 200, 50, 255]);
        assert!(cut.pixels().filter(|p| p.0[3] == 0).count() < 140 * 4);
        // The same picture cut as if it were keyed on magenta would have no backdrop to find.
        assert!(matches!(
            finish(&result(&img, false), Shape::Folder),
            Err(AiError::NoBackdrop)
        ));
    }

    #[test]
    fn the_template_reference_is_a_png_on_the_key_colour() {
        let png = template_reference(FOLDER_W, FOLDER_H, MAGENTA);
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(img.dimensions(), (FOLDER_W, FOLDER_H));
        assert_eq!(
            img.get_pixel(0, 0).0,
            [255, 0, 255, 255],
            "on the key colour"
        );
        assert!(
            matte::has_key_background(&img, MAGENTA, KeyOptions::default()),
            "so a model that keeps it gives a keyable result"
        );
        assert_eq!(hex(MAGENTA), KEY_HEX);
        assert_eq!(hex(GREEN), "#00FF00");
    }
}
