//! What the local models are asked to paint.
//!
//! Styles are written as what to paint, never as what to avoid: a distilled model runs without a
//! negative prompt, and naming a thing ("no folder") tends to paint it. No living artist or studio
//! is named, so what they make can go in a community pack (docs/PACK-TERMS.md, rule 3).

use serde::Serialize;

/// What comes out: artwork FolderSkin wraps onto its folder, or the whole folder painted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    /// A picture the app's compositor wraps onto its folder, so the geometry is always exact.
    #[default]
    Artwork,
    /// FolderSkin's blank folder repainted, cut out along the app's own silhouette.
    Folder,
}

impl Shape {
    pub fn id(self) -> &'static str {
        match self {
            Shape::Artwork => "artwork",
            Shape::Folder => "folder",
        }
    }

    pub fn parse(s: &str) -> Option<Shape> {
        [Shape::Artwork, Shape::Folder]
            .into_iter()
            .find(|shape| shape.id() == s)
    }
}

/// A named look, as a key and the words that paint it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Style {
    pub key: &'static str,
    pub text: &'static str,
}

const fn style(key: &'static str, text: &'static str) -> Style {
    Style { key, text }
}

/// The style presets. `none` paints the idea as it is.
pub const STYLES: &[Style] = &[
    style(
        "pop-art",
        "a bold pop art illustration: thick black outlines, flat saturated primary colours, Ben-Day halftone dots",
    ),
    style(
        "anime",
        "a hand-painted anime film still: soft watercolour skies, lush greenery, gentle warm light, clean cel-shaded shapes, nostalgic and whimsical",
    ),
    // Not "on canvas" or "a museum masterpiece": those paint the painting in its frame, on a wall.
    style(
        "oil",
        "a classical oil painting: thick oil paint with visible impasto brushstrokes, rich glazes, dramatic chiaroscuro light",
    ),
    style(
        "sketch",
        "a black and white graphite pencil sketch on textured paper: confident linework, fine hatching and cross-hatching, pure monochrome",
    ),
    style(
        "woodblock",
        "an ukiyo-e woodblock print: bold black outlines, flat indigo and vermilion colour blocks, washi paper grain",
    ),
    style(
        "travel-poster",
        "a vintage travel poster: flat colour shapes, a limited palette, grainy lithograph print texture",
    ),
    style(
        "watercolour",
        "a loose watercolour painting: soft wet edges, granulating pigment, white paper showing through",
    ),
    style(
        "clay",
        "a soft clay stop-motion diorama: rounded handmade shapes, pastel colours, fingerprints in the clay, warm studio light",
    ),
    style(
        "risograph",
        "a three-colour risograph print in teal, yellow and orange: coarse grain, slight misregistration",
    ),
    style(
        "art-nouveau",
        "an art nouveau poster: flowing organic lines, ornate floral borders, thin gold outlines, muted jewel colours",
    ),
    style(
        "pixel",
        "detailed 16-bit pixel art: crisp pixels, a limited retro palette, gentle dithering",
    ),
    style(
        "synthwave",
        "a 1980s airbrushed synthwave poster: glossy chrome, neon magenta and cyan glow, a sunset grid horizon",
    ),
    // Name only what should be in the picture: "softbox lighting" painted the softboxes.
    style(
        "photo",
        "a close-up product photograph: soft diffused light, crisp focus, shallow depth of field, a plain seamless coloured backdrop, rich colour",
    ),
    style("none", ""),
];

/// The preset's words for `style`, or `style` itself when it is someone's own words.
pub fn style_text(style: &str) -> &str {
    STYLES
        .iter()
        .find(|s| s.key == style)
        .map_or(style, |s| s.text)
        .trim()
}

/// Artwork lands on the folder's back panel whole, and on its front panel less a band at the top
/// and bottom; the top eighth is the tab and the strip beside the paper (docs/SKINS.md). So: fill
/// the frame, keep the subject in the middle, keep the top quiet.
///
/// The one negation stays on evidence. Against "the painted scene continues past all four edges",
/// klein framed the same 1 picture in 6 (pop art and oil, three seeds each) either way, but the
/// positive wording signed 2 of the 3 oils and this one none. `trim_border` catches the frames.
const ARTWORK: &str = "{style_lead}{idea}. The painted scene bleeds off all four edges of the image: no white border, \
no margin, no frame line and no paper edge anywhere around it. The main subject is large and sits \
in the centre, fully visible, with open space above it; the top eighth of the picture is only sky \
or plain background. Bold shapes and strong contrast that still read from across a room.";

/// The picture handed in is FolderSkin's blank folder on magenta. The key colour is never named:
/// an edit model told about magenta paints the folder magenta. It is told to leave the background
/// alone instead, and the cut-out uses our silhouette, not the colour.
const FOLDER: &str = "Turn the plain grey folder in image 1 into a folder painted all over as {idea}{style_tail}. The \
painting covers the folder's entire surface edge to edge, the back panel, the tab and the front \
panel, like a printed wrap rather than a picture placed on it, with the main subject in the middle \
of the front panel. The thin paper strip between the panels stays pale cream. Keep the folder's \
exact outline, tab, size and position, and leave the background around the folder exactly as it is.";

const REFERENCES: &str = "Using {refs} as the reference, paint {idea}{style_tail}. Keep the subject recognisable from the \
reference. One continuous full-bleed illustration that fills the entire frame edge to edge, the \
subject large and centred with open space above it; the top eighth of the picture is only sky or \
plain background.";

/// "image 1", "image 1 and image 2", "image 2, image 3 and image 4".
pub fn ref_names(n: usize, first: usize) -> String {
    let names: Vec<String> = (first..first + n).map(|i| format!("image {i}")).collect();
    match names.as_slice() {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// The first letter in upper case.
fn capitalised(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The prompt for `idea` in `style` (a preset key or someone's own words), as `shape`, with
/// `n_refs` reference pictures.
pub fn compose(idea: &str, style: &str, shape: Shape, n_refs: usize) -> String {
    let s = style_text(style);
    let idea = idea.trim().trim_end_matches('.');
    let style_tail = if s.is_empty() {
        String::new()
    } else {
        format!(", as {s}")
    };
    match shape {
        Shape::Folder => {
            let extra = if n_refs > 0 {
                format!(", taking the subject from {}", ref_names(n_refs, 2))
            } else {
                String::new()
            };
            FOLDER
                .replace("{idea}", &format!("{idea}{extra}"))
                .replace("{style_tail}", &style_tail)
        }
        Shape::Artwork if n_refs > 0 => REFERENCES
            .replace("{refs}", &ref_names(n_refs, 1))
            .replace("{idea}", idea)
            .replace("{style_tail}", &style_tail),
        Shape::Artwork => {
            let (lead, idea) = if s.is_empty() {
                (String::new(), capitalised(idea))
            } else {
                (format!("{} of ", capitalised(s)), idea.to_string())
            };
            ARTWORK
                .replace("{style_lead}", &lead)
                .replace("{idea}", &idea)
        }
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

    #[test]
    fn artwork_leads_with_the_style() {
        let p = compose("a retro film camera.", "pop-art", Shape::Artwork, 0);
        assert!(
            p.starts_with("A bold pop art illustration: thick black outlines, flat saturated primary colours, Ben-Day halftone dots of a retro film camera. The painted scene bleeds off all four edges"),
            "{p}"
        );
        assert!(p.ends_with("still read from across a room."));
        // Without a style the idea leads, capitalised.
        let bare = compose("  a koi pond at night ", "none", Shape::Artwork, 0);
        assert!(
            bare.starts_with("A koi pond at night. The painted scene"),
            "{bare}"
        );
    }

    #[test]
    fn someones_own_words_are_a_style_too() {
        let p = compose("a fox", "linocut in two inks", Shape::Artwork, 0);
        assert!(p.starts_with("Linocut in two inks of a fox."), "{p}");
    }

    #[test]
    fn references_are_named_by_number() {
        let p = compose("our dog Biscuit", "anime", Shape::Artwork, 2);
        assert!(
            p.starts_with("Using image 1 and image 2 as the reference, paint our dog Biscuit, as a hand-painted anime film still"),
            "{p}"
        );
        assert_eq!(ref_names(3, 2), "image 2, image 3 and image 4");
        assert_eq!(ref_names(1, 1), "image 1");
        assert_eq!(ref_names(0, 1), "");
    }

    #[test]
    fn a_whole_folder_repaints_image_one_and_never_names_the_key_colour() {
        let p = compose("a koi pond at night", "woodblock", Shape::Folder, 0);
        assert!(
            p.starts_with("Turn the plain grey folder in image 1 into a folder painted all over as a koi pond at night, as an ukiyo-e woodblock print"),
            "{p}"
        );
        for word in ["magenta", "#FF00FF", "pink"] {
            assert!(
                !p.to_lowercase().contains(&word.to_lowercase()),
                "{word} in {p}"
            );
        }
        let with_refs = compose("Biscuit", "none", Shape::Folder, 1);
        assert!(
            with_refs.contains("painted all over as Biscuit, taking the subject from image 2. The"),
            "{with_refs}"
        );
    }

    #[test]
    fn styles_paint_what_they_want_not_what_they_dont() {
        // A distilled model has no negative prompt: naming a thing paints it.
        for s in STYLES {
            let text = s.text.to_lowercase();
            for word in [" no ", "without", "avoid", "don't", "not "] {
                assert!(!text.contains(word), "{} says {word:?}", s.key);
            }
        }
        assert_eq!(STYLES.len(), 14, "13 presets and none");
        assert_eq!(style_text("none"), "");
        assert_eq!(style_text("oil"), STYLES[2].text);
    }

    #[test]
    fn the_artwork_template_keeps_its_one_negation() {
        // It stays on evidence (see ARTWORK); the others have none.
        assert!(ARTWORK.contains("no white border"));
        for template in [FOLDER, REFERENCES] {
            assert!(!template.contains(" no "), "{template}");
        }
    }

    #[test]
    fn theme_ideas_never_say_folder() {
        let idea = theme_idea("Taxes 2025");
        assert!(idea.ends_with("stands for Taxes 2025"));
        assert!(!idea.contains("folder"));
        assert!(!idea.contains('"'));
        assert_eq!(Shape::parse("folder"), Some(Shape::Folder));
        assert_eq!(Shape::parse("skin"), None);
    }
}
