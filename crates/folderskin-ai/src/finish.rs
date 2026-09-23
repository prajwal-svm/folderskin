//! Everything around one request that isn't the network: what to ask for, and what to keep of
//! the answer. The app and the `folderskin` command line both go through here, so a skin made
//! with a key comes out the same from either.

use crate::prompts::{self, Shape};
use crate::{AiError, GenerateRequest, GenerateResult, ModelInfo};
use folderskin_core::compositor::{self, SKIN_HEIGHT, SKIN_WIDTH};
use folderskin_core::matte::{self, KeyOptions, MAGENTA};
use image::RgbaImage;

/// Size the folder shape is rendered at before it is cut out (the folder's own aspect).
pub const FOLDER_W: u32 = 1166;
pub const FOLDER_H: u32 = 1091;
/// The key colour as the prompts name it; [`MAGENTA`] is the same colour as pixels.
pub const KEY_HEX: &str = "#FF00FF";
/// Reference pictures are downscaled before upload; models do not need more and it keeps the
/// request small.
pub const REFERENCE_MAX_SIDE: u32 = 1024;

/// The request for `idea` as `shape` from `model`, with the user's own reference picture if they
/// gave one (already made into a PNG by [`reference_png`]).
///
/// A whole folder from a model that can work from a picture, with none attached, sends our own
/// blank template, so the model repaints FolderSkin's folder instead of inventing one. A folder
/// render needs transparency: the model's own alpha when it has one, otherwise a magenta backdrop
/// cut out afterwards. The template sits on magenta and the prompt says to keep it, so a template
/// run is always keyed. Draws the template when it needs one, so call it off the UI thread.
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
    let key_hex = (wants_cutout && !want_alpha).then_some(KEY_HEX);
    let (width, height) = match shape {
        Shape::Skin => (SKIN_WIDTH, SKIN_HEIGHT),
        Shape::Folder => (FOLDER_W, FOLDER_H),
    };
    let (reference_png, prompt) = if reference_png.is_some() {
        let prompt = prompts::compose_with_reference(shape, idea, width, height, key_hex);
        (reference_png, prompt)
    } else if use_template {
        let prompt = prompts::compose_on_template(idea, width, height, KEY_HEX);
        (Some(template_reference(FOLDER_W, FOLDER_H)), prompt)
    } else {
        (None, prompts::compose(shape, idea, width, height, key_hex))
    };
    GenerateRequest {
        provider: provider.to_string(),
        model: model.id.to_string(),
        prompt,
        reference_png,
        size,
        want_alpha,
    }
}

/// A reference picture as the modest PNG that is uploaded.
pub fn reference_png(img: RgbaImage) -> Vec<u8> {
    folderskin_core::raster::encode_png(&shrink_to(img, REFERENCE_MAX_SIDE))
}

/// Our blank folder template, centred on flat magenta, as the PNG a model is asked to repaint.
pub fn template_reference(width: u32, height: u32) -> Vec<u8> {
    folderskin_core::raster::encode_png(&compositor::blank_template(width, height, MAGENTA))
}

/// `img` no longer than `max_side` on its longer side.
fn shrink_to(img: RgbaImage, max_side: u32) -> RgbaImage {
    let (w, h) = img.dimensions();
    let longest = w.max(h);
    if longest <= max_side {
        return img;
    }
    let s = max_side as f32 / longest as f32;
    image::imageops::resize(
        &img,
        ((w as f32 * s).round() as u32).max(1),
        ((h as f32 * s).round() as u32).max(1),
        image::imageops::FilterType::Lanczos3,
    )
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
/// keying the magenta it was asked for), artwork is cropped to the template's aspect.
pub fn finish(result: &GenerateResult, shape: Shape) -> Result<Finished, AiError> {
    let img = image::load_from_memory(&result.image)
        .map_err(|_| AiError::NotAnImage)?
        .to_rgba8();
    Ok(match shape {
        Shape::Folder if result.native_alpha => Finished::Folder(matte::autocrop(&img, 0)),
        Shape::Folder => {
            if !matte::has_key_background(&img, MAGENTA, KeyOptions::default()) {
                return Err(AiError::NoBackdrop);
            }
            Finished::Folder(matte::cutout(&img, MAGENTA, KeyOptions::default()))
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
        }
    }

    fn model(id: &str) -> &'static ModelInfo {
        crate::catalogue::providers()
            .iter()
            .flat_map(|p| p.models.iter())
            .find(|m| m.id == id)
            .unwrap()
    }

    #[test]
    fn a_folder_from_a_model_that_takes_pictures_repaints_our_template() {
        // Grok takes a picture and has no alpha.
        let req = plan(
            "xai",
            model("grok-imagine-image"),
            Shape::Folder,
            "koi",
            None,
            None,
        );
        assert!(req
            .prompt
            .starts_with("The attached image is the exact folder template"));
        let template = image::load_from_memory(req.reference_png.as_ref().unwrap()).unwrap();
        assert_eq!((template.width(), template.height()), (FOLDER_W, FOLDER_H));
        assert!(!req.want_alpha, "a template run is keyed");
    }

    #[test]
    fn a_folder_from_a_model_with_alpha_asks_for_it_and_names_no_key() {
        let req = plan(
            "recraft",
            model("recraftv3"),
            Shape::Folder,
            "koi",
            None,
            None,
        );
        assert!(req.want_alpha);
        assert!(req.reference_png.is_none());
        assert!(!req.prompt.contains(KEY_HEX));
        // Artwork never asks for alpha or a key.
        let art = plan(
            "recraft",
            model("recraftv3"),
            Shape::Skin,
            "koi",
            None,
            None,
        );
        assert!(!art.want_alpha && !art.prompt.contains(KEY_HEX));
        assert!(art.prompt.contains("1024 by 958"));
    }

    #[test]
    fn a_reference_goes_only_to_a_model_that_takes_one() {
        let png = reference_png(RgbaImage::from_pixel(3000, 1500, Rgba([9, 9, 9, 255])));
        let shrunk = image::load_from_memory(&png).unwrap();
        assert_eq!((shrunk.width(), shrunk.height()), (1024, 512));
        let taken = plan(
            "xai",
            model("grok-imagine-image"),
            Shape::Skin,
            "x",
            None,
            Some(png.clone()),
        );
        assert!(taken.prompt.starts_with("Use the supplied picture"));
        let dropped = plan(
            "recraft",
            model("recraftv3"),
            Shape::Skin,
            "x",
            None,
            Some(png),
        );
        assert!(dropped.reference_png.is_none());
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
    fn the_template_reference_is_a_png_at_the_folder_request_size() {
        let png = template_reference(FOLDER_W, FOLDER_H);
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
    }
}
