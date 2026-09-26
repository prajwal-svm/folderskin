//! Everything around one request that isn't the network: what to ask for, and what to keep of
//! the answer. The app and the `folderskin` command line both go through here, so a skin made
//! with a key comes out the same from either.

use crate::prompts::{self, Shape};
use crate::recipe::{
    self, sha256_hex, Around, Frame, Key, Lettering, PictureRecord, Recipe, Record, Role,
    Treatment, RECIPE_VERSION,
};
use crate::{AiError, GenerateRequest, GenerateResult, ModelInfo, Reference};
use folderskin_core::base::{Base, TEMPLATE_VERSION};
use folderskin_core::matte::{self, KeyOptions, MAGENTA};
use folderskin_core::painted;
use image::{GrayImage, Luma, RgbaImage};

/// Height a whole shape is asked for at; its width follows the shape's own aspect ([`frame`]).
pub const WHOLE_HEIGHT: u32 = 1088;
/// Side of the square a free icon is asked for in.
pub const ICON_SIDE: u32 = 1024;
/// The magenta key as the prompts name it; [`MAGENTA`] is the same colour as pixels.
pub const KEY_HEX: &str = "#FF00FF";
/// The green key, for the providers that leave a dark rim around a subject on magenta
/// ([`crate::ProviderInfo::key_colour`]) and for pink or violet art ([`recipe::key_for`]).
pub const GREEN: [u8; 3] = [0, 255, 0];
/// Reference pictures are downscaled before upload; models do not need more and it keeps the
/// request small, within the megapixel the providers that bill by the megapixel charge for.
pub const REFERENCE_MAX_SIDE: u32 = 1024;

/// The colour `provider` is asked to paint a backdrop in when FolderSkin cuts it out and nothing
/// else decides: magenta, or green for a provider that leaves a dark rim on magenta.
pub fn key_colour(provider: &str) -> [u8; 3] {
    crate::provider(provider).map_or(MAGENTA, |p| p.key_colour)
}

/// `colour` as a prompt names it: "#FF00FF".
pub fn hex([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02X}{g:02X}{b:02X}")
}

/// What the user asked for.
#[derive(Clone, Debug)]
pub struct Brief<'a> {
    /// Their own words.
    pub idea: &'a str,
    /// What it is for: a folder in a system's look, or a free icon.
    pub base: &'a Base,
    /// Artwork or the whole base. A free icon paints [`Shape::Icon`] whatever this says.
    pub shape: Shape,
    /// How it looks: a built-in style's treatment, or a saved prompt's.
    pub treatment: Option<&'a Treatment>,
    /// Their own pictures, each with its role, as the PNGs [`reference_png`] makes.
    pub pictures: Vec<Reference>,
}

/// The frame a picture is asked for in, every side a multiple of 16: the base's artwork size
/// for artwork (1024 x 960 for the Mac's folder, 1024 x 800 for Windows'), the whole base's own
/// shape for a whole one, and a square for a free icon.
pub fn frame(base: &Base, shape: Shape) -> Frame {
    match shape.on(base) {
        Shape::Skin => {
            let (w, h) = base.artwork_size();
            Frame::exact(w, h)
        }
        Shape::Icon => Frame::exact(ICON_SIDE, ICON_SIDE),
        Shape::Folder => {
            let aspect = base.template.map_or(1.0, |s| s.aspect());
            Frame::exact((WHOLE_HEIGHT as f32 * aspect).round() as u32, WHOLE_HEIGHT)
        }
    }
}

/// How an answer is cut out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cut {
    /// The colour its backdrop was asked to be, when it is cut out by colour.
    pub key: Key,
    /// Image 1 was the shape's template, drawn in this frame: the answer is cut along the
    /// shape's own silhouette.
    pub template: Option<Frame>,
}

/// A request, planned: what goes to the provider, how its answer is cut out, and what it was made
/// from, to keep with the skin.
#[derive(Clone, Debug)]
pub struct Planned {
    pub request: GenerateRequest,
    pub cut: Cut,
    pub record: Record,
}

/// The request for `brief` from `model`.
///
/// A whole base from a model that can work from pictures repaints the base's own blank template,
/// sent first, with the person's pictures after it, so the model paints FolderSkin's folder
/// instead of inventing one. A whole base or a free icon needs transparency: the model's own
/// alpha when it has one, otherwise a key colour cut out afterwards (green for Google's models
/// and for pink or violet looks, magenta otherwise). A template run keeps the template's
/// backdrop and is cut along the shape's silhouette. The picture is asked for at the shape's own
/// size, as a parameter; `size` is one the caller insists on instead. Draws the template when it
/// needs one, so call it off the UI thread.
pub fn plan(provider: &str, model: &ModelInfo, brief: &Brief, size: Option<String>) -> Planned {
    let base = brief.base;
    let shape = brief.shape.on(base);
    let frame = frame(base, shape);
    let key = recipe::key_for(provider, brief.idea, brief.treatment);
    let use_template = shape == Shape::Folder
        && model.accepts_reference
        && model.max_references > 0
        && !base.is_free();
    let wants_cutout = shape != Shape::Skin;
    let want_alpha = wants_cutout && model.native_alpha && !use_template;
    let keyed = wants_cutout && !want_alpha;
    let around = match shape {
        Shape::Skin => Around::Nothing,
        _ if use_template => Around::Kept,
        _ if want_alpha => Around::Alpha,
        _ => Around::Named(key),
    };

    // In the order the prompt numbers them: the template, then the person's, by role, as many as
    // the model takes. The prompt only ever names pictures that are sent.
    let mut pictures: Vec<Reference> = Vec::new();
    let mut template = None;
    if use_template {
        // Drawn at the size every reference picture is sent at, in the frame's own shape.
        let (w, h) = fitted((frame.width, frame.height), REFERENCE_MAX_SIDE);
        if let Some(png) = template_reference(base, w, h, key) {
            pictures.push(Reference::new(Role::Template, png));
            template = Some(format!("{}/{TEMPLATE_VERSION}", base.id));
        }
    }
    if model.accepts_reference {
        let mut own: Vec<Reference> = brief
            .pictures
            .iter()
            .filter(|p| !p.role.is_made())
            .cloned()
            .collect();
        own.sort_by_key(|p| p.role);
        pictures.extend(own);
    }
    pictures.truncate(model.max_references);
    let roles: Vec<Role> = pictures.iter().map(|p| p.role).collect();
    let lettering = Lettering::of(brief.idea, brief.treatment, base, shape);
    let recipe = Recipe {
        idea: brief.idea,
        treatment: brief.treatment,
        lettering: lettering.as_ref(),
        pictures: &roles,
        base,
        shape,
        around,
    };
    let prompt = prompts::render(&recipe, prompts::family(provider));
    let request = GenerateRequest {
        provider: provider.to_string(),
        model: model.id.to_string(),
        prompt,
        references: pictures,
        size: size.or_else(|| Some(frame.size())),
        want_alpha,
        key_colour: keyed.then_some(key.rgb()),
        keep_out: recipe::keep_out(&recipe),
        style_preset: recipe::style_preset(provider, brief.treatment, lettering.is_some()),
        lettering: lettering.as_ref().map(|l| l.words.join(" ")),
    };
    let record = Record {
        recipe: RECIPE_VERSION,
        prompt: request.prompt.clone(),
        // What the providers with a negative prompt were sent, exactly.
        negative_prompt: matches!(provider, "stability" | "ideogram")
            .then(|| crate::request::negative_prompt(&request)),
        style: brief
            .treatment
            .map(|t| t.id.clone())
            .filter(|id| !id.is_empty()),
        lettering: lettering.map(|l| l.words).unwrap_or_default(),
        pictures: request
            .references
            .iter()
            .map(|p| PictureRecord {
                role: p.role,
                sha256: sha256_hex(&p.png),
            })
            .collect(),
        template,
        key: keyed.then_some(key),
        seed: None,
        revised_prompt: None,
    };
    Planned {
        request,
        cut: Cut {
            key,
            template: use_template.then_some(frame),
        },
        record,
    }
}

/// A reference picture as the modest PNG that is uploaded.
pub fn reference_png(img: RgbaImage) -> Vec<u8> {
    folderskin_core::raster::encode_png(&shrink_to(img, REFERENCE_MAX_SIDE))
}

/// `base`'s blank template, `width` x `height`, centred on flat `key`, as the PNG a model is
/// asked to repaint. `None` for a free icon, which has no base to repaint.
pub fn template_reference(base: &Base, width: u32, height: u32, key: Key) -> Option<Vec<u8>> {
    base.blank(width, height, key.rgb())
        .map(|img| folderskin_core::raster::encode_png(&img))
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

/// `base`'s silhouette in a `width` x `height` frame, white inside, as its blank template has it:
/// what a whole painting is cut out along. `None` for a free icon, which has no template.
pub fn silhouette_of(base: &Base, width: u32, height: u32) -> Option<GrayImage> {
    let cut = base.blank_cutout(width, height)?;
    Some(GrayImage::from_fn(width, height, |x, y| {
        Luma([cut.get_pixel(x, y).0[3]])
    }))
}

/// `img` no longer than `max_side` on its longer side.
fn shrink_to(img: RgbaImage, max_side: u32) -> RgbaImage {
    let (w, h) = fitted(img.dimensions(), max_side);
    if (w, h) == img.dimensions() {
        return img;
    }
    image::imageops::resize(&img, w, h, image::imageops::FilterType::Lanczos3)
}

/// Alpha a provider left a shade under solid made solid: some return 252 to 254 for pixels that
/// are meant to be opaque.
fn solid_alpha(mut img: RgbaImage) -> RgbaImage {
    for p in img.pixels_mut() {
        if p.0[3] >= 250 {
            p.0[3] = 255;
        }
    }
    img
}

/// What a generated image becomes.
#[derive(Clone, Debug)]
pub enum Finished {
    /// A finished icon, cut out and trimmed to itself and used as it is: a whole folder, or a
    /// free icon.
    Folder(RgbaImage),
    /// Artwork cropped to the base's artwork size, for the compositor to wrap onto it.
    Artwork(RgbaImage),
}

/// Makes a provider's image into a skin for `base`, cut as `cut` says: a whole base or a free
/// icon is cut out (with the model's own alpha, along the shape's silhouette when it repainted
/// the template, or by keying the colour it was asked for), artwork is cropped to the base's
/// artwork size.
///
/// A free icon also comes out of a backdrop that drifted from the key, or one of another flat
/// colour, since it has no template whose shape could be mistaken for one. A whole folder or a
/// free icon with no flat backdrop at all is [`AiError::NoBackdrop`]: the caller decides what to
/// keep of it.
///
/// Magenta is keyed wherever it appears: it almost never belongs to the art. Green does, in every
/// leaf and field, so a green backdrop is only taken away where it reaches the edge of the
/// picture, from the shade of green it was actually painted in, and the greens painted on the
/// subject stay.
pub fn finish(
    result: &GenerateResult,
    base: &Base,
    shape: Shape,
    cut: Cut,
) -> Result<Finished, AiError> {
    let img = image::load_from_memory(&result.image)
        .map_err(|_| AiError::NotAnImage)?
        .to_rgba8();
    let key = cut.key.rgb();
    let keyed = |img: &RgbaImage| match cut.key {
        Key::Magenta => matte::cutout(img, key, KeyOptions::default()),
        Key::Green => matte::cutout_connected(img, matte::flat_backdrop(img).unwrap_or(key)),
    };
    Ok(match shape.on(base) {
        Shape::Folder | Shape::Icon if result.native_alpha => {
            Finished::Folder(matte::autocrop(&solid_alpha(img), 0))
        }
        Shape::Folder => {
            // Along our own silhouette, as the local model's are cut: it doesn't mind a backdrop
            // that drifted, and the edge keeps the painting's colours instead of a key-coloured
            // rim. A model that changed the shape falls back to the key.
            let along = cut
                .template
                .and_then(|f| silhouette_of(base, f.width, f.height))
                .and_then(|silhouette| painted::cut_along_silhouette(&img, &silhouette).image);
            match along {
                Some(cutout) => Finished::Folder(matte::autocrop(&cutout, 0)),
                None if matte::has_key_background(&img, key, KeyOptions::default()) => {
                    Finished::Folder(keyed(&img))
                }
                None => return Err(AiError::NoBackdrop),
            }
        }
        Shape::Icon => {
            if matte::has_key_background(&img, key, KeyOptions::default()) {
                Finished::Folder(keyed(&img))
            } else if let Some(backdrop) = matte::flat_backdrop(&img) {
                Finished::Folder(matte::cutout_connected(&img, backdrop))
            } else {
                return Err(AiError::NoBackdrop);
            }
        }
        Shape::Skin => {
            let (w, h) = base.artwork_size();
            Finished::Artwork(matte::crop_to_aspect(&img, w, h, (0.5, 0.5)))
        }
    })
}

/// A free icon kept whole when it came with no backdrop to cut it out of: the square picture as
/// it is, which is still an icon.
pub fn uncut(result: &GenerateResult) -> Result<Finished, AiError> {
    let img = image::load_from_memory(&result.image)
        .map_err(|_| AiError::NotAnImage)?
        .to_rgba8();
    Ok(Finished::Folder(img))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::styles;
    use folderskin_core::base::{BASES, FREE, MAC_FOLDER, WINDOWS_FOLDER};
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

    fn model(id: &str) -> &'static ModelInfo {
        crate::catalogue::providers()
            .iter()
            .flat_map(|p| p.models.iter())
            .find(|m| m.id == id)
            .unwrap()
    }

    fn brief<'a>(base: &'a Base, shape: Shape) -> Brief<'a> {
        Brief {
            idea: "koi",
            base,
            shape,
            treatment: None,
            pictures: Vec::new(),
        }
    }

    const MAGENTA_CUT: Cut = Cut {
        key: Key::Magenta,
        template: None,
    };

    #[test]
    fn every_shape_is_asked_for_in_a_frame_every_model_takes() {
        assert_eq!(
            frame(&MAC_FOLDER, Shape::Skin),
            Frame {
                width: 1024,
                height: 960
            }
        );
        assert_eq!(
            frame(&WINDOWS_FOLDER, Shape::Skin),
            Frame {
                width: 1024,
                height: 800
            }
        );
        // A whole folder in its own shape: about 1.06 wide for the Mac's, 1.27 for Windows'.
        for base in [&MAC_FOLDER, &WINDOWS_FOLDER] {
            let f = frame(base, Shape::Folder);
            let aspect = base.template.unwrap().aspect();
            assert_eq!(f.height, WHOLE_HEIGHT);
            assert!(
                (f.ratio() - aspect).abs() < 0.02,
                "{}: {f:?} for {aspect}",
                base.id
            );
        }
        assert!(
            frame(&WINDOWS_FOLDER, Shape::Folder).width > frame(&MAC_FOLDER, Shape::Folder).width
        );
        assert_eq!(
            frame(&FREE, Shape::Folder),
            Frame {
                width: 1024,
                height: 1024
            }
        );
        for base in BASES {
            for shape in [Shape::Skin, Shape::Folder, Shape::Icon] {
                let f = frame(base, shape);
                assert!(
                    f.width.is_multiple_of(16) && f.height.is_multiple_of(16),
                    "{} {shape:?}",
                    base.id
                );
            }
        }
    }

    #[test]
    fn a_folder_from_a_model_that_takes_pictures_repaints_its_template_first() {
        // Grok takes a picture and has no alpha.
        for base in [&MAC_FOLDER, &WINDOWS_FOLDER] {
            let planned = plan(
                "xai",
                model("grok-imagine-image"),
                &brief(base, Shape::Folder),
                None,
            );
            let req = &planned.request;
            assert!(
                req.prompt.starts_with("<IMAGE_0> is a blank"),
                "{}: {}",
                base.id,
                req.prompt
            );
            let f = frame(base, Shape::Folder);
            let template = image::load_from_memory(&req.references[0].png)
                .unwrap()
                .to_rgba8();
            // In the frame's own shape, at the size every reference picture is sent at.
            let (w, h) = fitted((f.width, f.height), REFERENCE_MAX_SIDE);
            assert_eq!(template.dimensions(), (w, h), "{}", base.id);
            assert_eq!(w.max(h), REFERENCE_MAX_SIDE);
            assert_eq!(req.references[0].role, Role::Template);
            assert!(!req.want_alpha, "a template run keeps its backdrop");
            assert_eq!(req.key_colour, Some(MAGENTA), "and says which it is");
            // The template is that base's own: the same pixels as its blank on the key.
            assert_eq!(template, base.blank(w, h, Key::Magenta.rgb()).unwrap());
            assert_eq!(
                planned.cut,
                Cut {
                    key: Key::Magenta,
                    template: Some(f)
                }
            );
            assert_eq!(
                planned.record.template.as_deref(),
                Some(format!("{}/1", base.id).as_str())
            );
            assert_eq!(req.size, Some(f.size()), "asked for at the shape's size");
        }
    }

    #[test]
    fn the_persons_pictures_follow_the_template_by_role() {
        let png = |v: u8| reference_png(RgbaImage::from_pixel(8, 8, Rgba([v, v, v, 255])));
        let mut b = brief(&MAC_FOLDER, Shape::Folder);
        b.idea = "our dog Biscuit";
        b.pictures = vec![
            Reference::new(Role::Style, png(1)),
            Reference::new(Role::Subject, png(2)),
            // Only FolderSkin makes templates.
            Reference::new(Role::Template, png(3)),
        ];
        let planned = plan("openai", model("gpt-image-2.5-flare"), &b, None);
        let roles: Vec<Role> = planned.request.references.iter().map(|p| p.role).collect();
        assert_eq!(roles, [Role::Template, Role::Subject, Role::Style]);
        let p = &planned.request.prompt;
        assert!(
            p.contains("Image 2 shows the subject")
                && p.contains("Image 3 is a style reference only"),
            "{p}"
        );
        assert_eq!(planned.record.pictures.len(), 3);
        assert_eq!(planned.record.pictures[1].sha256, sha256_hex(&png(2)));
        // A model that can't take pictures gets none, and is told of none.
        let words = plan("stability", model("core"), &b, None);
        assert!(words.request.references.is_empty());
        assert!(!words.request.prompt.contains("image 2"));
        // One that takes fewer than there are gets the first ones, and is told only of those.
        let one = ModelInfo {
            max_references: 2,
            ..*model("gpt-image-2.5-flare")
        };
        let two = plan("openai", &one, &b, None);
        let roles: Vec<Role> = two.request.references.iter().map(|p| p.role).collect();
        assert_eq!(roles, [Role::Template, Role::Subject]);
        assert!(
            !two.request.prompt.contains("Image 3"),
            "{}",
            two.request.prompt
        );
    }

    #[test]
    fn a_cut_out_from_a_model_with_alpha_asks_for_it_and_names_no_key() {
        let planned = plan(
            "openai",
            model("gpt-image-2.5-flare"),
            &brief(&FREE, Shape::Icon),
            None,
        );
        let req = &planned.request;
        assert!(req.want_alpha && req.references.is_empty());
        assert!(
            req.prompt.contains("The background is fully transparent."),
            "{}",
            req.prompt
        );
        assert_eq!((req.key_colour, planned.record.key), (None, None));
        // Artwork never asks for alpha or a key, and goes at its folder's own size.
        let art = plan(
            "openai",
            model("gpt-image-2.5-flare"),
            &brief(&WINDOWS_FOLDER, Shape::Skin),
            None,
        );
        assert!(!art.request.want_alpha && !art.request.prompt.contains("#FF00FF"));
        assert_eq!(art.request.key_colour, None);
        assert_eq!(art.request.size.as_deref(), Some("1024x800"));
        let asked = plan(
            "openai",
            model("gpt-image-2.5-flare"),
            &brief(&WINDOWS_FOLDER, Shape::Skin),
            Some("1024x1024".into()),
        );
        assert_eq!(
            asked.request.size.as_deref(),
            Some("1024x1024"),
            "a size asked for stays"
        );
    }

    #[test]
    fn a_folder_from_a_model_that_can_see_nothing_is_built_from_words_on_a_named_key() {
        // A FLUX model that takes no pictures.
        let blind = ModelInfo {
            accepts_reference: false,
            max_references: 0,
            ..*model("flux-2-pro")
        };
        let planned = plan("bfl", &blind, &brief(&WINDOWS_FOLDER, Shape::Folder), None);
        let req = &planned.request;
        assert!(req.references.is_empty());
        assert_eq!(req.key_colour, Some(MAGENTA));
        assert!(
            req.prompt
                .starts_with("Koi, painted all over one Windows-style folder"),
            "{}",
            req.prompt
        );
        assert!(req.prompt.contains("one perfectly flat magenta, #FF00FF"));
        assert_eq!(planned.cut.template, None);
        assert_eq!(planned.record.key, Some(Key::Magenta));
    }

    #[test]
    fn a_free_icon_is_asked_for_on_its_own_from_every_provider() {
        for p in crate::catalogue::providers() {
            for m in p.models {
                let planned = plan(p.id, m, &brief(&FREE, Shape::Folder), None);
                let req = &planned.request;
                assert!(
                    req.prompt.to_lowercase().contains("koi"),
                    "{}/{}",
                    p.id,
                    m.id
                );
                assert!(
                    req.prompt.contains("standalone icon")
                        || req.prompt.contains("one single, complete object"),
                    "{}/{}: {}",
                    p.id,
                    m.id,
                    req.prompt
                );
                assert!(req.references.is_empty(), "no template for a free icon");
                assert_eq!(req.want_alpha, m.native_alpha, "{}/{}", p.id, m.id);
                assert_eq!(
                    req.prompt.contains("perfectly flat"),
                    !m.native_alpha,
                    "{}/{}",
                    p.id,
                    m.id
                );
                assert_eq!(req.key_colour.is_some(), !m.native_alpha);
                assert_eq!(req.size.as_deref(), Some("1024x1024"));
                assert!(req.keep_out.iter().any(|k| k == "checkerboard pattern"));
            }
        }
    }

    #[test]
    fn google_and_pink_looks_are_keyed_on_green() {
        let g = plan(
            "google",
            model("gemini-3.1-flash-image"),
            &brief(&MAC_FOLDER, Shape::Folder),
            None,
        );
        assert_eq!(g.cut.key, Key::Green);
        assert_eq!(g.request.key_colour, Some(GREEN));
        let template = image::load_from_memory(&g.request.references[0].png)
            .unwrap()
            .to_rgba8();
        assert_eq!(
            template.get_pixel(0, 0).0,
            [0, 255, 0, 255],
            "the template sits on green"
        );
        let pop = Treatment::of(styles::style("pop-art").unwrap());
        let mut b = brief(&FREE, Shape::Icon);
        b.treatment = Some(&pop);
        let p = plan("bfl", model("flux-2-pro"), &b, None);
        assert_eq!(p.cut.key, Key::Green);
        assert_eq!(p.request.key_colour, Some(GREEN));
        assert!(
            p.request
                .prompt
                .contains("one perfectly flat green, #00FF00"),
            "{}",
            p.request.prompt
        );
    }

    #[test]
    fn every_base_plans_on_every_provider() {
        for base in BASES {
            for shape in [Shape::Skin, Shape::Folder, Shape::Icon] {
                for p in crate::catalogue::providers() {
                    for m in p.models {
                        let planned = plan(p.id, m, &brief(base, shape), None);
                        let req = &planned.request;
                        assert!(
                            req.prompt.to_lowercase().contains("koi"),
                            "{} {shape:?} {}",
                            base.id,
                            m.id
                        );
                        assert_eq!((req.provider.as_str(), req.model.as_str()), (p.id, m.id));
                        let whole = shape.on(base) != Shape::Skin;
                        assert!(!req.want_alpha || whole, "only a cut-out asks for alpha");
                        assert_eq!(
                            req.references.first().map(|p| p.role) == Some(Role::Template),
                            shape.on(base) == Shape::Folder && m.accepts_reference,
                            "{} {shape:?} {}",
                            base.id,
                            m.id
                        );
                        assert!(req.references.len() <= m.max_references);
                        assert_eq!(req.size, Some(frame(base, shape).size()));
                        assert_eq!(planned.record.prompt, req.prompt);
                        assert_eq!(planned.record.recipe, RECIPE_VERSION);
                    }
                }
            }
        }
    }

    #[test]
    fn the_style_the_lettering_and_the_switches_go_in_their_slots() {
        let woodblock = Treatment::of(styles::style("woodblock").unwrap());
        let b = Brief {
            idea: "a koi pond that says \"ZEN\"",
            base: &MAC_FOLDER,
            shape: Shape::Skin,
            treatment: Some(&woodblock),
            pictures: Vec::new(),
        };
        let planned = plan("openai", model("gpt-image-2.5-flare"), &b, None);
        let req = &planned.request;
        assert!(
            req.prompt
                .contains("Style: a traditional woodblock-printed illustration"),
            "{}",
            req.prompt
        );
        assert!(req.prompt.contains("Lettering: the words \"ZEN\" (Z-E-N) appear exactly once, in carved block lettering in keyline black"), "{}", req.prompt);
        assert_eq!(req.lettering.as_deref(), Some("ZEN"));
        assert_eq!(req.style_preset, None, "OpenAI has no presets");
        assert!(req.keep_out.iter().any(|k| k == "Mount Fuji"));
        assert_eq!(planned.record.style.as_deref(), Some("woodblock"));
        assert_eq!(planned.record.lettering, ["ZEN"]);
        assert_eq!(
            planned.record.negative_prompt, None,
            "OpenAI has no negative prompt"
        );
        // Ideogram gets the style's own preset, and a negative prompt that lets the words in.
        let ideogram = plan("ideogram", model("V_3"), &b, None);
        assert_eq!(
            ideogram.request.style_preset.as_deref(),
            Some("WOODBLOCK_PRINT")
        );
        let negative = ideogram.record.negative_prompt.unwrap();
        assert!(negative.contains("Mount Fuji"), "{negative}");
        assert!(
            !negative.contains("text"),
            "words were asked for: {negative}"
        );
        assert_eq!(
            negative,
            crate::request::negative_prompt(&ideogram.request),
            "the record keeps what was sent"
        );
        let stability = plan("stability", model("core"), &b, None);
        assert!(stability.record.negative_prompt.is_some());
    }

    #[test]
    fn a_template_run_is_cut_along_the_shapes_silhouette_even_on_a_drifted_backdrop() {
        let f = frame(&MAC_FOLDER, Shape::Folder);
        // The model's answer: the template painted blue, on a backdrop that drifted to raspberry.
        let blank = MAC_FOLDER.blank(f.width, f.height, [200, 30, 110]).unwrap();
        let painted = RgbaImage::from_fn(f.width, f.height, |x, y| {
            let p = blank.get_pixel(x, y).0;
            if p == [200, 30, 110, 255] {
                Rgba(p)
            } else {
                Rgba([30, 90, 200, 255])
            }
        });
        let cut = Cut {
            key: Key::Magenta,
            template: Some(f),
        };
        match finish(&result(&painted, false), &MAC_FOLDER, Shape::Folder, cut).unwrap() {
            Finished::Folder(out) => {
                assert!(
                    out.width() < f.width && out.height() < f.height,
                    "trimmed to the folder"
                );
                assert_eq!(out.get_pixel(out.width() / 2, out.height() / 2).0[3], 255);
                assert_eq!(
                    out.get_pixel(0, 0).0[3],
                    0,
                    "the corner is outside the folder"
                );
            }
            Finished::Artwork(_) => panic!("a folder became artwork"),
        }
        // Without the template, the raspberry isn't the magenta asked for: no backdrop.
        assert!(matches!(
            finish(
                &result(&painted, false),
                &MAC_FOLDER,
                Shape::Folder,
                MAGENTA_CUT
            ),
            Err(AiError::NoBackdrop)
        ));
    }

    #[test]
    fn a_keyed_folder_is_cut_out_and_a_scene_is_refused() {
        let mut keyed = RgbaImage::from_pixel(200, 200, Rgba([255, 0, 255, 255]));
        for y in 50..150 {
            for x in 40..160 {
                keyed.put_pixel(x, y, Rgba([30, 90, 200, 255]));
            }
        }
        match finish(
            &result(&keyed, false),
            &MAC_FOLDER,
            Shape::Folder,
            MAGENTA_CUT,
        )
        .unwrap()
        {
            Finished::Folder(cut) => assert_eq!(cut.dimensions(), (120, 100)),
            Finished::Artwork(_) => panic!("a folder became artwork"),
        }
        // The same on green, cut as green.
        let green = RgbaImage::from_fn(200, 200, |x, y| {
            let p = keyed.get_pixel(x, y).0;
            Rgba(if p == [255, 0, 255, 255] {
                [0, 255, 0, 255]
            } else {
                p
            })
        });
        let on_green = Cut {
            key: Key::Green,
            template: None,
        };
        assert!(matches!(
            finish(&result(&green, false), &MAC_FOLDER, Shape::Folder, on_green),
            Ok(Finished::Folder(_))
        ));
        let scene = RgbaImage::from_fn(200, 200, |x, y| Rgba([x as u8, y as u8, 90, 255]));
        let err = finish(
            &result(&scene, false),
            &MAC_FOLDER,
            Shape::Folder,
            MAGENTA_CUT,
        )
        .unwrap_err();
        assert!(matches!(err, AiError::NoBackdrop));
        match finish(
            &result(&scene, false),
            &MAC_FOLDER,
            Shape::Skin,
            MAGENTA_CUT,
        )
        .unwrap()
        {
            Finished::Artwork(art) => assert_eq!(art.dimensions(), (1024, 958)),
            Finished::Folder(_) => panic!("artwork became a folder"),
        }
        match finish(
            &result(&scene, false),
            &WINDOWS_FOLDER,
            Shape::Skin,
            MAGENTA_CUT,
        )
        .unwrap()
        {
            Finished::Artwork(art) => assert_eq!(art.dimensions(), (1024, 805)),
            Finished::Folder(_) => panic!("artwork became a folder"),
        }
        let broken = GenerateResult {
            image: b"not an image".to_vec(),
            ..result(&scene, false)
        };
        assert_eq!(
            finish(&broken, &MAC_FOLDER, Shape::Skin, MAGENTA_CUT)
                .unwrap_err()
                .to_string(),
            "the provider returned something that is not an image"
        );
    }

    #[test]
    fn a_free_icon_is_cut_out_of_whatever_flat_backdrop_it_came_on() {
        let on = |backdrop: [u8; 3]| {
            let mut img =
                RgbaImage::from_pixel(200, 200, Rgba([backdrop[0], backdrop[1], backdrop[2], 255]));
            for y in 60..140 {
                for x in 70..130 {
                    img.put_pixel(x, y, Rgba([240, 170, 30, 255]));
                }
            }
            img
        };
        for backdrop in [[255, 0, 255], [196, 40, 120], [250, 250, 250], [0, 255, 0]] {
            match finish(
                &result(&on(backdrop), false),
                &FREE,
                Shape::Skin,
                MAGENTA_CUT,
            )
            .unwrap()
            {
                Finished::Folder(cut) => {
                    assert_eq!(cut.dimensions(), (60, 80), "{backdrop:?}");
                    assert_eq!(cut.get_pixel(30, 40).0[3], 255);
                }
                Finished::Artwork(_) => panic!("a free icon is never artwork"),
            }
        }
        // A model with alpha: only trimmed, with alpha a shade under solid made solid.
        let mut clear = RgbaImage::new(100, 100);
        for y in 20..60 {
            for x in 30..50 {
                clear.put_pixel(x, y, Rgba([10, 200, 90, 252]));
            }
        }
        match finish(&result(&clear, true), &FREE, Shape::Icon, MAGENTA_CUT).unwrap() {
            Finished::Folder(cut) => {
                assert_eq!(cut.dimensions(), (20, 40));
                assert_eq!(cut.get_pixel(5, 5).0[3], 255);
            }
            Finished::Artwork(_) => panic!("a free icon is never artwork"),
        }
        // A scene with no backdrop is the caller's to keep whole, as a square picture.
        let scene = RgbaImage::from_fn(200, 200, |x, y| Rgba([x as u8, y as u8, 90, 255]));
        assert!(matches!(
            finish(&result(&scene, false), &FREE, Shape::Icon, MAGENTA_CUT),
            Err(AiError::NoBackdrop)
        ));
        match uncut(&result(&scene, false)).unwrap() {
            Finished::Folder(whole) => assert_eq!(whole.dimensions(), (200, 200)),
            Finished::Artwork(_) => panic!("kept whole, as it is"),
        }
    }

    #[test]
    fn references_are_uploaded_small_and_templates_and_silhouettes_line_up() {
        let png = reference_png(RgbaImage::from_pixel(3000, 1500, Rgba([9, 9, 9, 255])));
        let shrunk = image::load_from_memory(&png).unwrap();
        assert_eq!((shrunk.width(), shrunk.height()), (1024, 512));
        let f = frame(&MAC_FOLDER, Shape::Folder);
        let img = image::load_from_memory(
            &template_reference(&MAC_FOLDER, f.width, f.height, Key::Magenta).unwrap(),
        )
        .unwrap()
        .to_rgba8();
        assert_eq!(img.dimensions(), (f.width, f.height));
        assert_eq!(
            img.get_pixel(0, 0).0,
            [255, 0, 255, 255],
            "on the key colour"
        );
        assert!(matte::has_key_background(
            &img,
            Key::Magenta.rgb(),
            KeyOptions::default()
        ));
        let silhouette = silhouette_of(&MAC_FOLDER, f.width, f.height).unwrap();
        assert_eq!(silhouette.get_pixel(0, 0).0[0], 0);
        assert_eq!(silhouette.get_pixel(f.width / 2, f.height / 2).0[0], 255);
        assert!(template_reference(&FREE, 64, 64, Key::Magenta).is_none());
        assert!(silhouette_of(&FREE, 64, 64).is_none());
    }
}
