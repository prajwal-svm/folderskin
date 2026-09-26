//! The recipe a picture is made from, the same for every provider and for the local model.
//!
//! A request is not one string. It is the person's idea, word for word; a style, as treatment
//! only ([`Treatment`]); the words to letter, when the idea quotes any ([`Lettering`]); the
//! pictures sent with it, each with its role ([`Role`]); and the shape it is painted for. Each
//! provider family then renders the same recipe in its own form ([`crate::prompts::render`]), and
//! everything that isn't prose goes to the provider as a parameter: the exact size ([`Frame`]),
//! what to keep out where a provider has a negative prompt ([`keep_out`]) and the provider's own
//! preset for the style ([`style_preset`]).
//!
//! What was asked for is kept with the skin it made ([`Record`]), so a result can be traced to
//! its prompt and a request can be made again.

use crate::prompts::Shape;
use crate::styles::{Native, Style};
use folderskin_core::base::Base;
use serde::{Deserialize, Serialize};

/// The version of the prompt templates. Bumped when their words change, and recorded with every
/// skin, so a result can be traced to the templates that made it.
pub const RECIPE_VERSION: u32 = 1;

/// How the picture is rendered: a built-in style's treatment ([`crate::styles`]), or a saved
/// prompt's ([`crate::skill`]).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Treatment {
    /// The built-in style's id, or the saved prompt's, for the record.
    pub id: String,
    /// "a <medium>: <technique>, …", the words that paint it, read after "as".
    pub words: String,
    /// How lettering looks in it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lettering: Option<String>,
    /// What it tends to add that nobody asked for, for a negative prompt.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keep_out: Vec<String>,
    /// Photographic or 3D: cartoon and clip art are kept out.
    #[serde(default)]
    pub realistic: bool,
    /// Full of pink or violet: cut out of green rather than magenta.
    #[serde(default)]
    pub green_key: bool,
    /// The providers' own switches for it.
    #[serde(default)]
    pub native: Native,
}

impl Treatment {
    /// A built-in style's treatment.
    pub fn of(style: &Style) -> Treatment {
        Treatment {
            id: style.id.clone(),
            words: style.fragment.clone(),
            lettering: Some(style.lettering.clone()),
            keep_out: style.keep_out.clone(),
            realistic: style.realistic,
            green_key: style.green_key,
            native: style.native.clone(),
        }
    }

    /// Someone's own words for a style, as the command line takes them: "linocut in two inks".
    pub fn words(words: &str) -> Option<Treatment> {
        let words = trim_sentence(words);
        (!words.is_empty()).then(|| Treatment {
            id: String::new(),
            words: words.to_string(),
            ..Treatment::default()
        })
    }

    /// The treatment a command line or a saved chat names: a built-in style by id (or an id it
    /// had before), "none" for none, or someone's own words.
    pub fn named(style: &str) -> Option<Treatment> {
        let style = style.trim();
        if style.is_empty() || style.eq_ignore_ascii_case("none") {
            return None;
        }
        match crate::styles::style(style) {
            Some(s) => Some(Treatment::of(s)),
            None => Treatment::words(style),
        }
    }

    /// Its words without their technique: "a classical oil painting". What the local model gets
    /// when the whole of it would crowd out the idea.
    pub fn medium(&self) -> &str {
        self.words
            .split_once(':')
            .map_or(self.words.as_str(), |(medium, _)| medium)
            .trim()
    }
}

/// What a picture sent with the prompt is for. The pictures go in this order, and the prompt
/// numbers them the same way: the shape's template (or a free icon's canvas) first, then the
/// subject, then the style, then the palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// The shape's blank template, to repaint.
    Template,
    /// A flat canvas in the key colour, for a free icon to be painted in the middle of.
    Canvas,
    /// Who or what to paint, kept recognisable.
    Subject,
    /// A look to match, taking none of its content.
    Style,
    /// Colours to use, and nothing else.
    Palette,
}

impl Role {
    pub fn id(self) -> &'static str {
        match self {
            Role::Template => "template",
            Role::Canvas => "canvas",
            Role::Subject => "subject",
            Role::Style => "style",
            Role::Palette => "palette",
        }
    }

    /// A role someone can give their own picture: subject, style or palette. Anything else is a
    /// subject, which is what a picture handed in usually is.
    pub fn of_picture(id: &str) -> Role {
        match id.trim() {
            "style" => Role::Style,
            "palette" => Role::Palette,
            _ => Role::Subject,
        }
    }

    /// Whether FolderSkin makes it, rather than the person bringing it.
    pub fn is_made(self) -> bool {
        matches!(self, Role::Template | Role::Canvas)
    }
}

/// The words the idea asks to be lettered, and how and where they go.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lettering {
    /// Each phrase the idea quotes, as written, in order.
    pub words: Vec<String>,
    /// How the letters look: the style's lettering, or plain bold letters.
    pub look: String,
    /// Where they go on the shape: "across the middle of the picture".
    pub place: String,
}

/// How letters look when no style says.
pub const PLAIN_LETTERS: &str = "bold, clean letters that suit the picture";

impl Lettering {
    /// The lettering `idea` asks for on `base`, looking the way `treatment` letters: `None` when
    /// it quotes nothing.
    pub fn of(
        idea: &str,
        treatment: Option<&Treatment>,
        base: &Base,
        shape: Shape,
    ) -> Option<Lettering> {
        let words = quoted(idea);
        if words.is_empty() {
            return None;
        }
        let look = treatment
            .and_then(|t| t.lettering.as_deref())
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .unwrap_or(PLAIN_LETTERS)
            .to_string();
        Some(Lettering {
            words,
            look,
            place: place(base, shape),
        })
    }

    /// The words quoted, joined: `"ESCAPE"` or `"TAXES" and "2025"`.
    pub fn quoted(&self) -> String {
        join_and(
            &self
                .words
                .iter()
                .map(|w| format!("\"{w}\""))
                .collect::<Vec<_>>(),
        )
    }

    /// The words quoted, each spelled out after it: `"ESCAPE" (E-S-C-A-P-E)`. A word in a script
    /// without letters to spell is only quoted.
    pub fn spelled(&self) -> String {
        join_and(
            &self
                .words
                .iter()
                .map(|w| match spell(w) {
                    Some(spelled) if spelled != *w => format!("\"{w}\" ({spelled})"),
                    _ => format!("\"{w}\""),
                })
                .collect::<Vec<_>>(),
        )
    }
}

/// "a", "a and b", "a, b and c".
pub(crate) fn join_and(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// Every phrase `idea` puts in double quotes, straight, curly or its language's own («», 「」,
/// 『』, „“), trimmed, in the order given.
pub fn quoted(idea: &str) -> Vec<String> {
    const PAIRS: [(char, char); 6] = [
        ('"', '"'),
        ('\u{201C}', '\u{201D}'),
        ('\u{00AB}', '\u{00BB}'),
        ('\u{300C}', '\u{300D}'),
        ('\u{300E}', '\u{300F}'),
        ('\u{201E}', '\u{201C}'),
    ];
    let mut words = Vec::new();
    let mut rest = idea;
    while let Some((at, open, close)) = PAIRS
        .iter()
        .filter_map(|&(o, c)| rest.find(o).map(|at| (at, o, c)))
        .min_by_key(|&(at, _, _)| at)
    {
        let after = &rest[at + open.len_utf8()..];
        let Some(end) = after.find(close) else {
            break;
        };
        let phrase = after[..end]
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if !phrase.is_empty() {
            words.push(phrase);
        }
        rest = &after[end + close.len_utf8()..];
    }
    words
}

/// `phrase` letter by letter, its words apart: "TAX 2025" is "T-A-X, 2-0-2-5". `None` when it
/// has a letter from a script that isn't spelled that way.
pub fn spell(phrase: &str) -> Option<String> {
    let spellable = |c: char| c.is_ascii_digit() || (c.is_alphabetic() && (c as u32) < 0x0250);
    let words: Vec<String> = phrase
        .split_whitespace()
        .map(|w| {
            let letters: Vec<String> = w
                .chars()
                .filter(|c| c.is_alphanumeric())
                .map(String::from)
                .collect();
            letters.join("-")
        })
        .filter(|w| !w.is_empty())
        .collect();
    (!words.is_empty()
        && phrase
            .chars()
            .filter(|c| c.is_alphanumeric())
            .all(spellable))
    .then(|| words.join(", "))
}

/// Where lettering goes on `base` painted as `shape`: the middle band of the artwork (a little
/// low where a tab hides a corner), the middle of the part the subject is on, or on a free icon's
/// object.
pub fn place(base: &Base, shape: Shape) -> String {
    match (shape.on(base), base.anatomy) {
        (Shape::Icon, _) => "on the object itself, or beneath it as a word mark".into(),
        (Shape::Folder, Some(a)) => format!("across the middle of {}", a.middle),
        (Shape::Folder, None) => "across the middle of its face".into(),
        (Shape::Skin, Some(a)) if !a.corner.is_empty() => {
            "across the middle of the picture, a little below centre".into()
        }
        (Shape::Skin, _) => "across the middle of the picture".into(),
    }
}

/// `text` trimmed, without the full stop that ends it: it goes mid-sentence.
pub fn trim_sentence(text: &str) -> &str {
    text.trim().trim_end_matches('.').trim_end()
}

/// The size a picture is asked for in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
}

/// Aspect ratios every provider that takes one lists.
pub const RATIOS: &[&str] = &[
    "1:1", "5:4", "4:5", "4:3", "3:4", "3:2", "2:3", "16:9", "9:16",
];

impl Frame {
    /// `width` x `height` rounded to the nearest multiples of 16, which every model that takes
    /// an exact size accepts.
    pub fn exact(width: u32, height: u32) -> Frame {
        let round = |v: u32| (((v + 8) / 16) * 16).max(16);
        Frame {
            width: round(width),
            height: round(height),
        }
    }

    /// "1024x960", as the size parameters write it.
    pub fn size(&self) -> String {
        format!("{}x{}", self.width, self.height)
    }

    /// Width over height.
    pub fn ratio(&self) -> f32 {
        self.width as f32 / self.height.max(1) as f32
    }

    /// The ratio among `ratios` ("5:4", or "5x4" as Ideogram writes it) closest to this frame's.
    pub fn aspect_among<'a>(&self, ratios: &[&'a str]) -> Option<&'a str> {
        let want = self.ratio().ln();
        ratios
            .iter()
            .filter_map(|r| {
                let (w, h) = r.split_once([':', 'x'])?;
                let (w, h): (f32, f32) = (w.trim().parse().ok()?, h.trim().parse().ok()?);
                (w > 0.0 && h > 0.0).then(|| (*r, ((w / h).ln() - want).abs()))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(r, _)| r)
    }

    /// The closest of the common [`RATIOS`].
    pub fn aspect(&self) -> &'static str {
        self.aspect_among(RATIOS).unwrap_or("1:1")
    }

    /// The size among `sizes` ("1536x1024") closest to this frame's shape.
    pub fn nearest<'a>(&self, sizes: &[&'a str]) -> Option<&'a str> {
        self.aspect_among(sizes)
    }
}

/// The colour a picture is cut out of when the model can't return transparency.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Key {
    #[default]
    Magenta,
    Green,
}

impl Key {
    pub fn rgb(self) -> [u8; 3] {
        match self {
            Key::Magenta => folderskin_core::matte::MAGENTA,
            Key::Green => [0, 255, 0],
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Key::Magenta => "magenta",
            Key::Green => "green",
        }
    }

    pub fn hex(self) -> &'static str {
        match self {
            Key::Magenta => "#FF00FF",
            Key::Green => "#00FF00",
        }
    }
}

/// The key for a request: green for a provider whose pictures come back with a dark rim on
/// magenta ([`crate::ProviderInfo::key_colour`]: Google's), and for a look or an idea with pink or
/// violet in it, which a magenta cut would eat; magenta otherwise.
pub fn key_for(provider: &str, idea: &str, treatment: Option<&Treatment>) -> Key {
    let green = crate::provider(provider).is_some_and(|p| p.key_colour == Key::Green.rgb())
        || treatment.is_some_and(|t| t.green_key || pinkish(&t.words))
        || pinkish(idea);
    if green {
        Key::Green
    } else {
        Key::Magenta
    }
}

/// The start of a word for pink or violet, in the languages FolderSkin speaks. An idea is written
/// in the person's own language, and a pink one is pink whatever the word.
const PINK_WORDS: &[&str] = &[
    // English
    "pink", "magenta", "fuchsia", "violet", "purple", "lilac", "lavender", "mauve",
    // Français
    "rose", "fuchsia", "pourpre", "lilas", "lavande", // Español
    "rosa", "fucsia", "morad", "púrpura", "purpura", "lila", "lavanda",
];

/// The same in the languages written without spaces between words, found anywhere in the text.
const PINK_SIGNS: &[&str] = &[
    // 日本語
    "ピンク",
    "マゼンタ",
    "桃色",
    "紫",
    "藤色",
    "ラベンダー",
    "ライラック",
    "フューシャ",
    // 한국어
    "분홍",
    "핑크",
    "마젠타",
    "자주",
    "보라",
    "바이올렛",
    "라벤더",
    "라일락",
    "자홍",
    // 简体中文
    "粉色",
    "粉红",
    "品红",
    "洋红",
    "玫红",
    "桃红",
    "紫",
    "薰衣草",
    "丁香色",
];

/// Whether `text` asks for pink or violet: a word that starts like one of [`PINK_WORDS`] ("pinkish",
/// "violets", "rosado"), or one of [`PINK_SIGNS`] anywhere in it.
fn pinkish(text: &str) -> bool {
    let text = text.to_lowercase();
    text.split(|c: char| !c.is_alphanumeric())
        .any(|word| PINK_WORDS.iter().any(|w| word.starts_with(w)))
        || PINK_SIGNS.iter().any(|w| text.contains(w))
}

/// What goes around the subject, and how the model is told.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Around {
    /// Artwork fills its frame: nothing goes around it.
    Nothing,
    /// The model returns real transparency.
    Alpha,
    /// Image 1 is the template or a canvas on the key colour, and the model leaves it as it is.
    /// The colour is never named: a model told about magenta paints with it.
    Kept,
    /// The model is told the key colour, by name and value.
    Named(Key),
}

/// Everything a prompt is made of.
#[derive(Clone, Copy, Debug)]
pub struct Recipe<'a> {
    /// The person's own words, as they wrote them.
    pub idea: &'a str,
    pub treatment: Option<&'a Treatment>,
    pub lettering: Option<&'a Lettering>,
    /// The roles of the pictures sent, in the order they are sent.
    pub pictures: &'a [Role],
    /// What it is painted for.
    pub base: &'a Base,
    /// What is painted: artwork, the whole shape or a free icon ([`Shape::on`] the base).
    pub shape: Shape,
    pub around: Around,
}

impl Recipe<'_> {
    /// What is painted, on this base.
    pub fn painted(&self) -> Shape {
        self.shape.on(self.base)
    }

    /// The picture numbers (from 1) of the pictures in `role`.
    pub fn numbers(&self, role: Role) -> Vec<usize> {
        self.pictures
            .iter()
            .enumerate()
            .filter(|(_, r)| **r == role)
            .map(|(i, _)| i + 1)
            .collect()
    }

    /// Whether image 1 is the template or a canvas the model paints on.
    pub fn paints_on_image_one(&self) -> bool {
        self.pictures.first().is_some_and(|r| r.is_made())
    }
}

/// What to keep out, for the providers with a negative prompt (Stability and Ideogram): borders
/// and frames always, the style's own keep-outs unless the idea asks for them, cartoon for a
/// realistic look, and what the shape mustn't grow. The provider layer adds text (unless words
/// are to be lettered), watermarks and signatures ([`crate::request::negative_prompt`]).
pub fn keep_out(recipe: &Recipe) -> Vec<String> {
    let mut out: Vec<String> = [
        "border",
        "frame",
        "paper margin",
        "vignette",
        "cropped subject",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if let Some(t) = recipe.treatment {
        // What the idea asks for is never kept out: a woodblock wave is still a wave.
        out.extend(
            t.keep_out
                .iter()
                .map(|k| k.trim().to_string())
                .filter(|k| !asked_for(recipe.idea, k)),
        );
        if t.realistic {
            out.extend(["cartoon", "clip art"].map(String::from));
        }
    }
    match recipe.painted() {
        Shape::Icon => out.extend(["scenery", "floor", "checkerboard pattern"].map(String::from)),
        Shape::Folder => out.extend(["extra tabs", "stacked copies"].map(String::from)),
        Shape::Skin => {}
    }
    let mut seen = std::collections::HashSet::new();
    out.retain(|k| !k.is_empty() && seen.insert(k.to_lowercase()));
    out
}

/// Whether `idea` asks for something `keep_out` names: any of its words of four letters or more,
/// singular or plural, is in the idea.
fn asked_for(idea: &str, keep_out: &str) -> bool {
    let stem = |w: &str| {
        let w = w.to_lowercase();
        w.strip_suffix("es")
            .filter(|s| s.ends_with(['s', 'x', 'h']))
            .or_else(|| w.strip_suffix('s'))
            .unwrap_or(&w)
            .to_string()
    };
    let words = |text: &str| -> Vec<String> {
        text.split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.chars().count() >= 4)
            .map(stem)
            .collect()
    };
    let idea = words(idea);
    words(keep_out).iter().any(|w| idea.contains(w))
}

/// The look as `provider`'s own style preset, where it has one: Stability's preset for the style,
/// and on Ideogram the style's preset, or else a style type (REALISTIC for a realistic look,
/// DESIGN when words are to be lettered, the style's own type, GENERAL otherwise). The provider
/// layer sends it only when it's one the provider documents.
pub fn style_preset(
    provider: &str,
    treatment: Option<&Treatment>,
    lettering: bool,
) -> Option<String> {
    let native = treatment.map(|t| &t.native);
    match provider {
        "stability" => native.and_then(|n| n.stability.clone()),
        "ideogram" => native.and_then(|n| n.ideogram_preset.clone()).or_else(|| {
            Some(if treatment.is_some_and(|t| t.realistic) {
                "REALISTIC".to_string()
            } else if lettering {
                "DESIGN".to_string()
            } else {
                native
                    .and_then(|n| n.ideogram_type.clone())
                    .unwrap_or_else(|| "GENERAL".to_string())
            })
        }),
        _ => None,
    }
}

/// A picture sent, as the record keeps it: its role and what it was.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PictureRecord {
    pub role: Role,
    /// The SHA-256 of the PNG sent, in hex.
    pub sha256: String,
}

/// What a skin was made from, kept with it: enough to trace a result to the prompt that made it
/// and to make it again.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    /// The templates' version ([`RECIPE_VERSION`]).
    pub recipe: u32,
    /// The prompt as it was sent.
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,
    /// The style, by id: a built-in style's, or a saved prompt's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// The words asked to be lettered.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lettering: Vec<String>,
    /// Every picture sent, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pictures: Vec<PictureRecord>,
    /// The template repainted and the version it was drawn in, "mac-folder/1", when one was sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    /// The colour it was cut out of, when it was cut by colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<Key>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    /// The prompt as the provider rewrote it, when it says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revised_prompt: Option<String>,
}

/// The SHA-256 of `bytes`, in lower-case hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::styles::style;
    use folderskin_core::base::{FREE, MAC_FOLDER, WINDOWS_FOLDER};

    #[test]
    fn quoted_words_are_found_in_every_kind_of_quote() {
        assert!(quoted("a fox").is_empty());
        assert_eq!(
            quoted("a poster that says \"ESCAPE\" in retro letters"),
            ["ESCAPE"]
        );
        assert_eq!(
            quoted("\u{201C}Taxes\u{201D} and \u{00AB} 2025 \u{00BB}, \"\" and \"open"),
            ["Taxes", "2025"]
        );
        assert_eq!(quoted("「夜」の森"), ["夜"]);
        assert_eq!(quoted("„Hallo  Welt“"), ["Hallo Welt"]);
    }

    #[test]
    fn words_are_spelled_letter_by_letter_where_a_script_has_letters() {
        assert_eq!(spell("ESCAPE").as_deref(), Some("E-S-C-A-P-E"));
        assert_eq!(spell("TAX 2025").as_deref(), Some("T-A-X, 2-0-2-5"));
        assert_eq!(spell("Café!").as_deref(), Some("C-a-f-é"));
        assert_eq!(spell("夜の森"), None);
        let l = Lettering {
            words: vec!["TAXES".into(), "2025".into(), "夜".into()],
            look: PLAIN_LETTERS.into(),
            place: "x".into(),
        };
        assert_eq!(l.quoted(), "\"TAXES\", \"2025\" and \"夜\"");
        assert_eq!(
            l.spelled(),
            "\"TAXES\" (T-A-X-E-S), \"2025\" (2-0-2-5) and \"夜\""
        );
        // A one-letter word spells as itself, so it isn't spelled twice.
        let one = Lettering {
            words: vec!["A".into()],
            ..l
        };
        assert_eq!(one.spelled(), "\"A\"");
    }

    #[test]
    fn lettering_looks_like_the_style_and_sits_where_the_shape_shows_it() {
        let screen = Treatment::of(style("screenprint").unwrap());
        let l = Lettering::of(
            "a lagoon with the word \"ESCAPE\"",
            Some(&screen),
            &MAC_FOLDER,
            Shape::Skin,
        )
        .unwrap();
        assert_eq!(l.look, "bold retro sans-serif letters in one flat ink");
        assert_eq!(l.place, "across the middle of the picture");
        let plain = Lettering::of("\"ZEN\"", None, &WINDOWS_FOLDER, Shape::Skin).unwrap();
        assert_eq!(plain.look, PLAIN_LETTERS);
        assert!(
            plain.place.contains("a little below centre"),
            "the tab hides a corner"
        );
        assert_eq!(
            Lettering::of("\"ZEN\"", None, &MAC_FOLDER, Shape::Folder)
                .unwrap()
                .place,
            "across the middle of the front panel"
        );
        assert!(Lettering::of("\"ZEN\"", None, &FREE, Shape::Skin)
            .unwrap()
            .place
            .starts_with("on the object itself"));
        assert!(Lettering::of("no quotes", None, &FREE, Shape::Icon).is_none());
    }

    #[test]
    fn a_treatment_is_a_style_a_script_or_someones_own_words() {
        let oil = Treatment::named("oil").unwrap();
        assert_eq!(oil.id, "oil");
        assert!(oil.realistic && oil.lettering.is_some());
        assert_eq!(oil.medium(), "a classical oil painting");
        assert_eq!(Treatment::named("ukiyoe").unwrap().id, "woodblock");
        assert_eq!(Treatment::named("none"), None);
        assert_eq!(Treatment::named("  "), None);
        let own = Treatment::named("linocut in two inks.").unwrap();
        assert_eq!(
            (own.id.as_str(), own.words.as_str()),
            ("", "linocut in two inks")
        );
        assert_eq!(own.medium(), "linocut in two inks");
    }

    #[test]
    fn frames_are_asked_for_in_sizes_and_ratios_providers_take() {
        let mac = Frame::exact(1024, 958);
        assert_eq!((mac.size(), mac.aspect()), ("1024x960".to_string(), "1:1"));
        let windows = Frame::exact(1024, 805);
        assert_eq!(
            (windows.size(), windows.aspect()),
            ("1024x800".to_string(), "5:4")
        );
        assert_eq!(
            windows.aspect_among(&["1:1", "4:3", "3:4", "16:9"]),
            Some("4:3")
        );
        assert_eq!(windows.aspect_among(&["5x4", "1x1"]), Some("5x4"));
        assert_eq!(
            windows.nearest(&["1024x1024", "1536x1024", "1024x1536"]),
            Some("1536x1024")
        );
        assert_eq!(mac.nearest(&["1024x1024", "1536x1024"]), Some("1024x1024"));
        assert_eq!(windows.aspect_among(&["nope"]), None);
        assert_eq!(Frame::exact(1, 1).size(), "16x16");
    }

    #[test]
    fn the_key_is_green_where_magenta_would_be_eaten_or_leave_halos() {
        assert_eq!(key_for("openai", "a koi pond", None), Key::Magenta);
        assert_eq!(key_for("google", "a koi pond", None), Key::Green);
        assert_eq!(key_for("xai", "a Pink flamingo", None), Key::Green);
        let synth = Treatment::of(style("synthwave").unwrap());
        assert_eq!(key_for("local", "a car", Some(&synth)), Key::Green);
        let own = Treatment::words("soft lavender watercolour").unwrap();
        assert_eq!(key_for("local", "a car", Some(&own)), Key::Green);
        assert_eq!((Key::Green.name(), Key::Green.hex()), ("green", "#00FF00"));
        assert_eq!(Key::Magenta.rgb(), [255, 0, 255]);
        // Pink in the person's own language is pink too.
        for idea in [
            "des roses sur un piano",
            "un flamenco rosado",
            "un gato morado",
            "ピンクの桜",
            "보라색 고래",
            "粉红色的云",
            "a pinkish sky",
        ] {
            assert_eq!(key_for("openai", idea, None), Key::Green, "{idea}");
        }
        // A word that only contains one isn't one.
        for idea in ["a page of prose", "a violin", "a green frog on a log"] {
            assert_eq!(key_for("openai", idea, None), Key::Magenta, "{idea}");
        }
    }

    fn recipe<'a>(
        treatment: Option<&'a Treatment>,
        lettering: Option<&'a Lettering>,
        shape: Shape,
        base: &'a Base,
    ) -> Recipe<'a> {
        Recipe {
            idea: "a fox",
            treatment,
            lettering,
            pictures: &[],
            base,
            shape,
            around: Around::Nothing,
        }
    }

    #[test]
    fn what_is_kept_out_is_what_the_look_and_the_shape_drag_in() {
        let photo = Treatment::of(style("photo").unwrap());
        let n = keep_out(&recipe(Some(&photo), None, Shape::Skin, &MAC_FOLDER)).join(", ");
        assert!(
            n.starts_with("border, frame, paper margin, vignette, cropped subject"),
            "{n}"
        );
        assert!(
            n.contains("softboxes") && n.ends_with("cartoon, clip art"),
            "{n}"
        );
        // Text, watermarks and signatures are the provider layer's to add.
        assert!(!n.contains("text") && !n.contains("watermark"), "{n}");
        let icon = keep_out(&recipe(None, None, Shape::Icon, &FREE)).join(", ");
        assert!(
            icon.ends_with("scenery, floor, checkerboard pattern"),
            "{icon}"
        );
        let whole = keep_out(&recipe(None, None, Shape::Folder, &MAC_FOLDER)).join(", ");
        assert!(whole.ends_with("extra tabs, stacked copies"), "{whole}");
    }

    #[test]
    fn what_the_idea_asks_for_is_never_kept_out() {
        let woodblock = Treatment::of(style("woodblock").unwrap());
        let mut r = recipe(Some(&woodblock), None, Shape::Skin, &MAC_FOLDER);
        r.idea = "a fox surfing a wave";
        let n = keep_out(&r).join(", ");
        assert!(!n.contains("great waves"), "{n}");
        assert!(
            n.contains("Mount Fuji") && n.contains("cherry blossom"),
            "{n}"
        );
        r.idea = "Lilies on a pond";
        let nouveau = Treatment::of(style("art-nouveau").unwrap());
        r.treatment = Some(&nouveau);
        assert!(!keep_out(&r).join(", ").contains("lilies"));
        assert!(asked_for("two lanterns", "a paper lantern"));
        assert!(asked_for("glass boxes", "boxes"));
        assert!(!asked_for("a koi", "koi"), "short words don't count");
    }

    #[test]
    fn presets_follow_the_look_and_the_lettering_on_the_providers_that_have_them() {
        let photo = Treatment::of(style("photo").unwrap());
        assert_eq!(
            style_preset("stability", Some(&photo), true).as_deref(),
            Some("photographic")
        );
        assert_eq!(
            style_preset("ideogram", Some(&photo), true).as_deref(),
            Some("REALISTIC")
        );
        let anime = Treatment::of(style("anime").unwrap());
        assert_eq!(
            style_preset("ideogram", Some(&anime), false).as_deref(),
            Some("FICTION")
        );
        assert_eq!(
            style_preset("ideogram", Some(&anime), true).as_deref(),
            Some("DESIGN")
        );
        assert_eq!(
            style_preset("ideogram", None, false).as_deref(),
            Some("GENERAL")
        );
        assert_eq!(style_preset("stability", None, false), None);
        let woodblock = Treatment::of(style("woodblock").unwrap());
        assert_eq!(
            style_preset("ideogram", Some(&woodblock), true).as_deref(),
            Some("WOODBLOCK_PRINT"),
            "a style's own preset over the lettering's type"
        );
        for provider in ["openai", "google", "xai", "bfl", "recraft", "local"] {
            assert_eq!(
                style_preset(provider, Some(&photo), true),
                None,
                "{provider}"
            );
        }
    }

    #[test]
    fn pictures_are_numbered_in_the_order_they_are_sent() {
        let roles = [Role::Template, Role::Subject, Role::Subject, Role::Style];
        let r = Recipe {
            pictures: &roles,
            ..recipe(None, None, Shape::Folder, &MAC_FOLDER)
        };
        assert_eq!(r.numbers(Role::Subject), [2, 3]);
        assert_eq!(r.numbers(Role::Style), [4]);
        assert!(r.paints_on_image_one());
        assert_eq!(Role::of_picture("style"), Role::Style);
        assert_eq!(Role::of_picture("whatever"), Role::Subject);
        assert!(Role::Canvas.is_made() && !Role::Palette.is_made());
        assert_eq!(sha256_hex(b"abc").len(), 64);
    }
}
