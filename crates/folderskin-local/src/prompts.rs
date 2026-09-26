//! What the local model is asked to paint.
//!
//! The prompt is FolderSkin's recipe ([`folderskin_ai::recipe`]), rendered for FLUX the way a
//! provider's FLUX model gets it ([`folderskin_ai::prompts::Family::Flux`]): the idea first and
//! word for word, the style after it as treatment only, the words to letter in their own
//! sentence, and nothing named that shouldn't be painted. Two things are this computer's own:
//!
//! * A whole shape and a free icon are both painted on a picture handed in as image 1: the
//!   shape's blank template, or a flat canvas in the key colour. The key colour is never named: an
//!   edit model told about magenta paints with it. The model is told to leave the backdrop as it
//!   is, and the cut-out follows our silhouette or the backdrop it finds.
//! * klein's text encoder reads 512 tokens and drops the rest, where the shape's words sit. A
//!   prompt that would run past [`TOKEN_BUDGET`] keeps its style to the medium alone, and the
//!   idea is never cut.
//!
//! No living artist or studio is named by a built-in style, so what they make can go in a
//! community pack (docs/PACK-TERMS.md, rule 3).

use folderskin_ai::prompts::{self as ai_prompts, Family, Shape as AiShape};
use folderskin_ai::recipe::{Around, Lettering, Recipe, Role, Treatment};
use folderskin_core::base::{Base, MAC_FOLDER};
use serde::Serialize;

/// What comes out: artwork FolderSkin wraps onto a base, the whole base painted, or a free icon.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    /// A picture the app's compositor wraps onto its folder, so the geometry is always exact.
    #[default]
    Artwork,
    /// The base's blank template repainted, cut out along the app's own silhouette.
    Folder,
    /// A free icon: one subject standing on its own, painted on a flat canvas and cut out of it.
    Icon,
}

impl Shape {
    pub fn id(self) -> &'static str {
        match self {
            Shape::Artwork => "artwork",
            Shape::Folder => "folder",
            Shape::Icon => "icon",
        }
    }

    pub fn parse(s: &str) -> Option<Shape> {
        [Shape::Artwork, Shape::Folder, Shape::Icon]
            .into_iter()
            .find(|shape| shape.id() == s)
    }

    /// What is painted for `base` when `self` is asked for: a free icon has no base to paint art
    /// for or to repaint, so it is always an [`Shape::Icon`], and a base with a template never
    /// paints one.
    pub fn on(self, base: &Base) -> Shape {
        match (base.is_free(), self) {
            (true, _) => Shape::Icon,
            (false, Shape::Icon) => Shape::Folder,
            (false, shape) => shape,
        }
    }

    /// The same, as the recipe names it.
    pub fn recipe(self) -> AiShape {
        match self {
            Shape::Artwork => AiShape::Skin,
            Shape::Folder => AiShape::Folder,
            Shape::Icon => AiShape::Icon,
        }
    }
}

/// A named look, as a key and the words that paint it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Style {
    pub key: &'static str,
    pub text: &'static str,
}

/// The style presets `--style` takes: every built-in style ([`folderskin_ai::styles`]), then
/// `none`, which paints the idea as it is. An id a style had before still finds it.
pub fn styles() -> Vec<Style> {
    folderskin_ai::styles::styles()
        .iter()
        .map(|s| Style {
            key: s.id.as_str(),
            text: s.fragment.as_str(),
        })
        .chain(std::iter::once(Style {
            key: "none",
            text: "",
        }))
        .collect()
}

/// The words `style` paints with: a preset's fragment, someone's own words as they are, or
/// nothing for `none`.
pub fn style_text(style: &str) -> String {
    Treatment::named(style).map(|t| t.words).unwrap_or_default()
}

/// The most of klein's 512-token text encoder a prompt uses, leaving room for how its tokenizer
/// counts what [`tokens`] only estimates.
pub const TOKEN_BUDGET: usize = 400;

/// About how many tokens `text` is: 1.3 a word, which errs long for English.
pub fn tokens(text: &str) -> usize {
    (text.split_whitespace().count() * 13).div_ceil(10)
}

/// Everything the local model's prompt is made of.
#[derive(Clone, Copy, Debug)]
pub struct Slots<'a> {
    /// The subject and the scene in plain words.
    pub idea: &'a str,
    /// How it looks, or `None` to paint the idea as it is.
    pub treatment: Option<&'a Treatment>,
    /// What is painted ([`Shape::on`] the base).
    pub shape: Shape,
    /// What it is for: a folder in a system's look, or no base for a free icon.
    pub base: &'a Base,
    /// The reference pictures' roles, in the order they are handed in, after the template or
    /// canvas when there is one.
    pub refs: &'a [Role],
}

/// The prompt for `idea` in `style` (a preset key, someone's own words or "none"), as `shape` for
/// FolderSkin's own folder, with `n_refs` pictures of the subject.
pub fn compose(idea: &str, style: &str, shape: Shape, n_refs: usize) -> String {
    let treatment = Treatment::named(style);
    let refs = vec![Role::Subject; n_refs];
    compose_slots(&Slots {
        idea,
        treatment: treatment.as_ref(),
        shape,
        base: &MAC_FOLDER,
        refs: &refs,
    })
}

/// The pictures the runtime is handed, by role, in order: the template or the canvas the shape
/// is painted on, then the reference pictures.
pub fn roles(shape: Shape, base: &Base, refs: &[Role]) -> Vec<Role> {
    let mut roles = match shape.on(base) {
        Shape::Artwork => Vec::new(),
        Shape::Folder => vec![Role::Template],
        Shape::Icon => vec![Role::Canvas],
    };
    roles.extend(refs.iter().copied().filter(|r| !r.is_made()));
    roles
}

/// The prompt, put together from its slots.
pub fn compose_slots(slots: &Slots) -> String {
    let shape = slots.shape.on(slots.base);
    let roles = roles(shape, slots.base, slots.refs);
    let around = if shape == Shape::Artwork {
        Around::Nothing
    } else {
        Around::Kept
    };
    let render = |treatment: Option<&Treatment>| {
        let lettering = Lettering::of(slots.idea, treatment, slots.base, shape.recipe());
        ai_prompts::render(
            &Recipe {
                idea: slots.idea,
                treatment,
                lettering: lettering.as_ref(),
                pictures: &roles,
                base: slots.base,
                shape: shape.recipe(),
                around,
            },
            Family::Flux,
        )
    };
    let full = render(slots.treatment);
    match slots.treatment {
        Some(t) if tokens(&full) > TOKEN_BUDGET => {
            // The style gives way before the idea: its medium alone, "a classical oil painting".
            let short = Treatment {
                words: t.medium().to_string(),
                ..t.clone()
            };
            render(Some(&short))
        }
        _ => full,
    }
}

/// The idea `theme` paints for a folder called `name`. The name bare, and never the word
/// "folder": quoted, a model letters it (and misspells it); told "folder", it paints a folder.
pub fn theme_idea(name: &str) -> String {
    format!("one clear, recognisable object or scene that stands for {name}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use folderskin_core::base::{FREE, WINDOWS_FOLDER};

    #[test]
    fn artwork_leads_with_the_idea_and_the_style_follows() {
        let p = compose("a retro film camera.", "pop-art", Shape::Artwork, 0);
        assert!(
            p.starts_with("A retro film camera, as a pop-art comic illustration: thick black outlines, flat saturated primary colours, Ben-Day halftone dots, hard graphic shadows. The painted scene bleeds off all four edges"),
            "{p}"
        );
        assert!(p.ends_with("still read from across a room."));
        // Without a style the idea is all there is, capitalised.
        let bare = compose("  a koi pond at night ", "none", Shape::Artwork, 0);
        assert!(
            bare.starts_with("A koi pond at night. The painted scene"),
            "{bare}"
        );
    }

    #[test]
    fn someones_own_words_are_a_style_too() {
        let p = compose("a fox", "linocut in two inks", Shape::Artwork, 0);
        assert!(p.starts_with("A fox, as linocut in two inks."), "{p}");
    }

    #[test]
    fn an_old_style_key_still_finds_its_style() {
        for (old, words) in [
            ("travel-poster", "a mid-century screen-printed illustration"),
            ("sketch", "a detailed graphite pencil drawing"),
            ("woodblock", "a traditional woodblock-printed illustration"),
        ] {
            assert!(style_text(old).starts_with(words), "{old}");
        }
        assert_eq!(style_text("none"), "");
        let keys: Vec<&str> = styles().iter().map(|s| s.key).collect();
        assert_eq!(keys.len(), 31, "thirty styles and none");
        assert_eq!(keys.last(), Some(&"none"));
        for s in styles() {
            let text = s.text.to_lowercase();
            for word in [" no ", "without", "avoid", "don't", "not "] {
                assert!(!text.contains(word), "{} says {word:?}", s.key);
            }
        }
    }

    #[test]
    fn references_are_named_by_number_and_role() {
        let p = compose("our dog Biscuit", "anime", Shape::Artwork, 2);
        assert!(
            p.starts_with("Our dog Biscuit, as a cel-shaded anime illustration"),
            "{p}"
        );
        assert!(
            p.contains("Images 1 and 2 show the subject: keep it recognisably the same"),
            "{p}"
        );
        let styled = compose_slots(&Slots {
            idea: "a fox",
            treatment: None,
            shape: Shape::Artwork,
            base: &MAC_FOLDER,
            refs: &[Role::Subject, Role::Style],
        });
        assert!(
            styled.contains("Image 2 is a style reference only"),
            "{styled}"
        );
    }

    #[test]
    fn a_whole_folder_repaints_image_one_and_never_names_the_key_colour() {
        let p = compose("a koi pond at night", "woodblock", Shape::Folder, 0);
        assert!(
            p.starts_with("Turn the plain grey folder in image 1 into a folder painted all over as a koi pond at night, as a traditional woodblock-printed illustration"),
            "{p}"
        );
        assert!(p.contains("The thin paper strip between the panels stays pale cream. Keep the folder's exact outline"), "{p}");
        for word in ["magenta", "#ff00ff", "pink", "green"] {
            assert!(!p.to_lowercase().contains(word), "{word} in {p}");
        }
        let with_refs = compose("Biscuit", "none", Shape::Folder, 1);
        assert!(
            with_refs.contains("painted all over as Biscuit, taking the subject from image 2. The"),
            "{with_refs}"
        );
        assert_eq!(
            roles(Shape::Folder, &MAC_FOLDER, &[Role::Subject]),
            [Role::Template, Role::Subject]
        );
    }

    fn slots<'a>(base: &'a Base, shape: Shape, idea: &'a str) -> Slots<'a> {
        Slots {
            idea,
            treatment: None,
            shape,
            base,
            refs: &[],
        }
    }

    #[test]
    fn a_windows_folder_repaints_its_own_parts_and_no_paper() {
        let p = compose_slots(&slots(&WINDOWS_FOLDER, Shape::Folder, "a koi pond"));
        assert!(
            p.starts_with("Turn the plain grey folder in image 1 into a folder painted all over as a koi pond."),
            "{p}"
        );
        assert!(
            p.contains("the curved step where the front panel rises to meet it"),
            "{p}"
        );
        assert!(
            !p.contains("paper"),
            "Windows' folder has no paper strip: {p}"
        );
        let art = compose_slots(&slots(&WINDOWS_FOLDER, Shape::Artwork, "a koi pond"));
        assert!(
            art.contains("the top sixth of the picture, and its upper-left corner,"),
            "{art}"
        );
        assert!(compose("a koi pond", "none", Shape::Artwork, 0).contains("top eighth"));
    }

    #[test]
    fn a_free_icon_is_painted_on_a_canvas_it_is_never_told_the_colour_of() {
        for asked in [Shape::Artwork, Shape::Folder, Shape::Icon] {
            assert_eq!(asked.on(&FREE), Shape::Icon);
            let p = compose_slots(&slots(&FREE, asked, "a cheerful fox mascot."));
            assert!(
                p.starts_with("A cheerful fox mascot. It is one single, complete object in the middle of image 1"),
                "{p}"
            );
            assert!(
                p.ends_with("Leave the flat background around it exactly as it is."),
                "{p}"
            );
            assert!(!p.to_lowercase().contains("magenta"), "{p}");
        }
        assert_eq!(
            roles(Shape::Icon, &FREE, &[Role::Subject]),
            [Role::Canvas, Role::Subject]
        );
        let referred = compose_slots(&Slots {
            refs: &[Role::Subject, Role::Subject],
            ..slots(&FREE, Shape::Icon, "our dog Biscuit")
        });
        assert!(
            referred.starts_with(
                "Our dog Biscuit, taking the subject from images 2 and 3. It is one single"
            ),
            "{referred}"
        );
        assert_eq!(Shape::Icon.on(&MAC_FOLDER), Shape::Folder);
    }

    #[test]
    fn quoted_words_are_lettered_in_the_styles_own_lettering() {
        for shape in [Shape::Artwork, Shape::Folder, Shape::Icon] {
            let p = compose(
                "a retro poster that says \"ESCAPE\"",
                "screenprint",
                shape,
                0,
            );
            assert!(
                p.contains(" The words \"ESCAPE\" are written once in bold retro sans-serif letters in one flat ink, large and centred"),
                "{shape:?}: {p}"
            );
        }
        let plain = compose("a fox", "none", Shape::Artwork, 0);
        assert!(!plain.contains("written"), "{plain}");
    }

    #[test]
    fn a_long_prompt_keeps_its_idea_and_shortens_its_style() {
        let idea = "a lighthouse ".repeat(120);
        let oil = Treatment::named("oil").unwrap();
        let p = compose_slots(&Slots {
            treatment: Some(&oil),
            ..slots(&MAC_FOLDER, Shape::Artwork, &idea)
        });
        assert!(p.contains(", as a classical oil painting."), "{p}");
        assert!(!p.contains("impasto"), "the technique gave way: {p}");
        assert!(p.contains(&idea.trim()[1..]), "the idea is whole");
        let short = compose("a lighthouse", "oil", Shape::Artwork, 0);
        assert!(short.contains("impasto") && tokens(&short) <= TOKEN_BUDGET);
    }

    #[test]
    fn theme_ideas_never_say_folder() {
        let idea = theme_idea("Taxes 2025");
        assert!(idea.ends_with("stands for Taxes 2025"));
        assert!(!idea.contains("folder"));
        assert!(!idea.contains('"'));
        assert_eq!(Shape::parse("folder"), Some(Shape::Folder));
        assert_eq!(Shape::parse("icon"), Some(Shape::Icon));
        assert_eq!(Shape::parse("skin"), None);
        assert_eq!(Shape::Artwork.recipe(), AiShape::Skin);
    }
}
