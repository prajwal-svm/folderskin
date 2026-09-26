//! The prompt each provider family is sent, rendered from one recipe ([`crate::recipe::Recipe`]).
//!
//! Every family gets the same things in its own form: the person's idea first and word for word,
//! then the style as treatment only, the words to letter when there are any, a sentence for each
//! picture sent by its role, and what the shape needs. Sizes, transparency and exclusions a
//! provider has a parameter for go in the parameter, never in the prose.
//!
//! * [`Family::Instruct`] (OpenAI, Google) and [`Family::Grok`] (xAI) read instructions the way a
//!   person does: short labelled lines, and what to leave out in one "Constraints" line at the
//!   end, as OpenAI recommends and Google's own icon template does. Grok names pictures
//!   `<IMAGE_0>`, `<IMAGE_1>` and so on, counting from zero.
//! * [`Family::Flux`] (Black Forest Labs in the cloud, and the local model) weighs earlier words
//!   more and has no negative prompt: prose, subject first, and nothing named that shouldn't be
//!   painted, except the one border sentence kept on evidence.
//! * [`Family::Design`] (Ideogram, Recraft) wants a one-sentence summary first, lettering early
//!   and a short prompt; what to keep out goes in its negative prompt.
//! * [`Family::Stability`] wants a short comma list and a real negative prompt.
//!
//! Three things can be painted ([`Shape`]):
//!
//! * [`Shape::Skin`], artwork that FolderSkin's compositor wraps onto the shape, so the geometry
//!   stays ours. It is described as a picture "seen very small", never as a folder or an icon:
//!   those nouns are what a model draws when it is told about them.
//! * [`Shape::Folder`], the whole shape painted. A model that can take a picture repaints the
//!   shape's own blank template, image 1, which is cut out along our silhouette afterwards; one
//!   that can't builds it from its parts in words, on transparency or a named key colour.
//! * [`Shape::Icon`], a free icon: one object standing on its own, used as it is.

use crate::recipe::{join_and, trim_sentence, Around, Lettering, Recipe, Role};
use folderskin_core::base::{Anatomy, Base, Family as BaseFamily};

/// What the model should draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// Flat artwork, at the base's own artwork size, that the compositor wraps onto the base.
    Skin,
    /// The whole base as one object, used as the icon directly: a folder, or whatever the base is.
    Folder,
    /// A free icon: one subject standing on its own, used as the icon directly.
    Icon,
}

impl Shape {
    pub fn id(self) -> &'static str {
        match self {
            Shape::Skin => "skin",
            Shape::Folder => "folder",
            Shape::Icon => "icon",
        }
    }

    pub fn from_id(id: &str) -> Option<Shape> {
        match id {
            "skin" => Some(Shape::Skin),
            "folder" => Some(Shape::Folder),
            "icon" => Some(Shape::Icon),
            _ => None,
        }
    }

    /// What is painted for `base` when `self` is asked for: a free icon has no base to paint art
    /// for or to repaint, so it is always an [`Shape::Icon`], and a base with a template paints
    /// its art or its whole self, never a free icon.
    pub fn on(self, base: &Base) -> Shape {
        match (base.is_free(), self) {
            (true, _) => Shape::Icon,
            (false, Shape::Icon) => Shape::Folder,
            (false, shape) => shape,
        }
    }
}

/// A one-line starting point the user can pick instead of writing a prompt.
pub struct Preset {
    pub id: &'static str,
    pub label: &'static str,
    pub idea: &'static str,
}

/// Starting points offered in the chat's "/" menu: a spread of subjects that make good skins.
/// Each is what the picture shows, never how it is rendered: a style goes in its own slot.
pub const PRESETS: &[Preset] = &[
    Preset {
        id: "aurora",
        label: "Aurora",
        idea: "a night sky with green and violet aurora ribbons over dark mountains, faint stars",
    },
    Preset {
        id: "dunes",
        label: "Dunes",
        idea: "warm desert dunes at golden hour, long soft shadows, fine sand grain",
    },
    Preset {
        id: "lighthouse",
        label: "Lighthouse",
        idea: "a lighthouse on a rocky point at dusk, its beam sweeping over a calm sea",
    },
    Preset {
        id: "terrazzo",
        label: "Terrazzo",
        idea: "pale terrazzo with scattered chips of teal, ochre and charcoal",
    },
    Preset {
        id: "wave",
        label: "Wave",
        idea: "a towering ocean wave with deep indigo troughs and white foam",
    },
    Preset {
        id: "circuit",
        label: "Circuit",
        idea: "an emerald circuit board macro, gold traces, soft bokeh highlights",
    },
    Preset {
        id: "linen",
        label: "Linen",
        idea: "undyed linen weave in raking light, visible slubs and thread texture",
    },
    Preset {
        id: "nebula",
        label: "Nebula",
        idea: "a violet and cyan nebula with dust lanes and scattered stars",
    },
];

/// How a provider's models want to be asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// OpenAI and Google: labelled lines, and what to leave out at the end.
    Instruct,
    /// xAI: the same, with its own names for the pictures.
    Grok,
    /// Black Forest Labs and the local model: prose, subject first, no negations.
    Flux,
    /// Ideogram and Recraft: a summary sentence, lettering early, short.
    Design,
    /// Stability: a short comma list.
    Stability,
}

/// The family `provider` belongs to ("local" is the local model).
pub fn family(provider: &str) -> Family {
    match provider {
        "xai" => Family::Grok,
        "bfl" | "local" => Family::Flux,
        "ideogram" | "recraft" => Family::Design,
        "stability" => Family::Stability,
        _ => Family::Instruct,
    }
}

/// The prompt `recipe` is sent to `family` with.
pub fn render(recipe: &Recipe, family: Family) -> String {
    match family {
        Family::Instruct | Family::Grok => instruct(recipe, family),
        Family::Flux => flux(recipe),
        Family::Design => design(recipe),
        Family::Stability => stability(recipe),
    }
}

// ---------------------------------------------------------------- the shape's words

/// How a shape is described, from the registry's words for it ([`Anatomy`]), or plainly by its
/// name for one the registry describes no further.
struct Words {
    noun: &'static str,
    kind: String,
    keeps: &'static str,
    surface: &'static str,
    middle: &'static str,
    paper: &'static str,
    paper_keep: &'static str,
    corner: &'static str,
    parts: Vec<&'static str>,
}

fn words(base: &Base) -> Words {
    match base.anatomy {
        Some(Anatomy {
            noun,
            kind,
            parts,
            keeps,
            surface,
            middle,
            paper,
            paper_keep,
            corner,
        }) => Words {
            noun,
            kind: kind.to_string(),
            keeps,
            surface,
            middle,
            paper,
            paper_keep,
            corner,
            parts: parts.to_vec(),
        },
        None => Words {
            noun: match base.family {
                BaseFamily::Folder => "folder",
                BaseFamily::Drive => "drive",
                BaseFamily::Free => "object",
            },
            kind: base.label.to_lowercase(),
            keeps: "every part of its shape",
            surface: "its whole face",
            middle: "its face",
            paper: "",
            paper_keep: "",
            corner: "",
            parts: Vec::new(),
        },
    }
}

/// The band at the top of the artwork the shape's tab and its strip hide, in words.
fn band(base: &Base) -> &'static str {
    match base.tab_share() {
        s if s <= 0.14 => "eighth",
        s if s <= 0.18 => "sixth",
        s if s <= 0.22 => "fifth",
        _ => "quarter",
    }
}

/// "one", "two" ... for the number of parts a shape is built from.
fn number_word(n: usize) -> String {
    const WORDS: [&str; 10] = [
        "no", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
    ];
    WORDS
        .get(n)
        .map_or_else(|| n.to_string(), |w| w.to_string())
}

/// "(1) a back panel …; (2) …", back to front.
fn part_list(parts: &[&str]) -> String {
    parts
        .iter()
        .enumerate()
        .map(|(i, p)| format!("({}) {p}", i + 1))
        .collect::<Vec<_>>()
        .join("; ")
}

/// The first letter in upper case.
fn capitalised(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

// ---------------------------------------------------------------- pictures

/// "Image 2", "images 2 and 3", or for Grok "<IMAGE_1>", "<IMAGE_1> and <IMAGE_2>".
fn named(numbers: &[usize], family: Family, capital: bool) -> String {
    if family == Family::Grok {
        return join_and(
            &numbers
                .iter()
                .map(|n| format!("<IMAGE_{}>", n - 1))
                .collect::<Vec<_>>(),
        );
    }
    let word = match (numbers.len() > 1, capital) {
        (false, true) => "Image",
        (false, false) => "image",
        (true, true) => "Images",
        (true, false) => "images",
    };
    let list = join_and(&numbers.iter().map(|n| n.to_string()).collect::<Vec<_>>());
    format!("{word} {list}")
}

/// Image 1, as the family names it: "image 1" or "<IMAGE_0>".
fn first(family: Family) -> String {
    named(&[1], family, false)
}

/// One sentence for each role among the person's pictures, in the order they are sent.
fn references(r: &Recipe, family: Family) -> Vec<String> {
    let mut out = Vec::new();
    let subject = r.numbers(Role::Subject);
    if !subject.is_empty() {
        let verb = if subject.len() > 1 { "show" } else { "shows" };
        let how = if r.treatment.is_some() {
            "in the style described"
        } else {
            "as described"
        };
        out.push(format!(
            "{} {verb} the subject: keep it recognisably the same, with its shape, colours and \
             markings, while painting it {how}.",
            named(&subject, family, true)
        ));
    }
    let style = r.numbers(Role::Style);
    if !style.is_empty() {
        let (verb, its) = if style.len() > 1 {
            ("are", "their")
        } else {
            ("is", "its")
        };
        out.push(format!(
            "{} {verb} a style reference only: match {its} medium, palette, light and texture, \
             and take none of {its} content or layout.",
            named(&style, family, true)
        ));
    }
    let palette = r.numbers(Role::Palette);
    if !palette.is_empty() {
        let (verb, its, it) = if palette.len() > 1 {
            ("are", "their", "them")
        } else {
            ("is", "its", "it")
        };
        out.push(format!(
            "{} {verb} a palette reference: use {its} colours, and nothing else from {it}.",
            named(&palette, family, true)
        ));
    }
    out
}

/// ", taking the subject from image 2" when subject pictures follow the one painted on.
fn subject_ref(r: &Recipe, family: Family) -> String {
    let subject = r.numbers(Role::Subject);
    if subject.is_empty() {
        String::new()
    } else {
        format!(
            ", taking the subject from {}",
            named(&subject, family, false)
        )
    }
}

/// The style and palette sentences for a prose prompt, each with its leading space.
fn look_refs(r: &Recipe, family: Family) -> String {
    references(r, family)
        .into_iter()
        .filter(|s| !s.contains("the subject:"))
        .map(|s| format!(" {s}"))
        .collect()
}

// ---------------------------------------------------------------- lettering

/// The family's lettering sentence.
fn lettering(l: &Lettering, family: Family) -> String {
    let many = l.words.len() > 1;
    match family {
        Family::Instruct | Family::Grok => format!(
            "Lettering: the words {} {} exactly once, in {}, large and easy to read, {}. There is \
             no other text anywhere.",
            l.spelled(),
            if many { "each appear" } else { "appear" },
            l.look,
            l.place
        ),
        Family::Flux => format!(
            " The words {} are {}written once in {}, large and centred {}.",
            l.quoted(),
            if many { "each " } else { "" },
            l.look,
            l.place
        ),
        Family::Design => format!(
            " The words {} in {}, large, {}.",
            l.quoted(),
            l.look,
            l.place
        ),
        Family::Stability => format!(", the words {} in {}", l.quoted(), l.look),
    }
}

fn flux_lettering(r: &Recipe) -> String {
    r.lettering
        .map(|l| lettering(l, Family::Flux))
        .unwrap_or_default()
}

// ---------------------------------------------------------------- instruction following

fn instruct(r: &Recipe, family: Family) -> String {
    let idea = trim_sentence(r.idea);
    let w = words(r.base);
    let style = r.treatment.map(|t| trim_sentence(&t.words));
    let no_text = if r.lettering.is_some() {
        ""
    } else {
        "; no text, letters or numbers"
    };
    let mut paragraphs: Vec<String> = Vec::new();
    let constraints: String;
    let mut background: Option<String> = None;

    match r.painted() {
        Shape::Skin => {
            paragraphs.push(format!("Create a picture of {idea}."));
            if let Some(style) = style {
                paragraphs.push(format!(
                    "Style: {style}. The style sets the medium, light, colour and texture only; \
                     what the picture shows stays exactly as described above."
                ));
            }
            let top = if w.corner.is_empty() {
                format!("the top {} of the picture holds", band(r.base))
            } else {
                format!(
                    "the top {} of the picture and its {} corner hold",
                    band(r.base),
                    w.corner
                )
            };
            paragraphs.push(format!(
                "Composition: One continuous picture that fills the whole frame, edge to edge. \
                 The main subject is large, complete and in the centre, with open space above it; \
                 {top} only sky, background or soft texture. The subject has a bold, clear \
                 silhouette and strong light-against-dark contrast with what is behind it, so it \
                 reads at a glance even when very small; fine texture and detail inside the \
                 shapes are welcome. It will be seen very small, down to 16 pixels, as well as \
                 large."
            ));
            constraints = format!(
                "Constraints: no borders, frames, paper margins or vignettes; no watermark or \
                 signature{no_text}."
            );
        }
        Shape::Folder if r.around == Around::Kept => {
            let one = first(family);
            let paper = if w.paper_keep.is_empty() {
                String::new()
            } else {
                format!("{} ", w.paper_keep)
            };
            paragraphs.push(format!(
                "{} is a blank {kind}: the exact shape to paint on. Keep its outline, {keeps}, \
                 its size and its position in the frame exactly as they are, and leave the flat \
                 background around the {noun} exactly as it is. Paint only the {noun}'s surface: \
                 {idea}, wrapped across {surface} as one continuous surface that follows the \
                 {noun}'s edges and rounded corners, with the main subject in the middle of \
                 {middle}. {paper}Soft light and shading that follow the {noun}'s form are welcome.",
                capitalised(&one),
                kind = w.kind,
                keeps = w.keeps,
                noun = w.noun,
                surface = w.surface,
                middle = w.middle,
            ));
            if let Some(style) = style {
                paragraphs.push(format!(
                    "Style: {style}. The style sets the medium, light, colour and texture only; \
                     the {noun}'s shape and what the painting shows stay exactly as described.",
                    noun = w.noun
                ));
            }
            constraints = format!(
                "Constraints: change only the {noun}'s surface; keep everything else in {one} the \
                 same; no extra tabs, sheets, flaps or stacked copies; no watermark or \
                 signature{no_text}.",
                noun = w.noun
            );
        }
        Shape::Folder => {
            let built = if w.parts.is_empty() {
                "It is one solid object with softly rounded corners and real thickness.".to_string()
            } else {
                format!(
                    "It is built from exactly {} parts, back to front: {}. It is one solid object \
                     with softly rounded edges and real thickness.",
                    number_word(w.parts.len()),
                    part_list(&w.parts)
                )
            };
            let paper = if w.paper_keep.is_empty() {
                String::new()
            } else {
                format!(" {}", w.paper_keep)
            };
            paragraphs.push(format!(
                "Create one {kind}, seen straight on, as a finished icon. {built} Paint {idea} \
                 across {surface} as one continuous surface that follows the {noun}'s edges and \
                 rounded corners, with the main subject in the middle of {middle}.{paper}",
                kind = w.kind,
                noun = w.noun,
                surface = w.surface,
                middle = w.middle,
            ));
            if let Some(style) = style {
                paragraphs.push(format!(
                    "Style: {style}. The style sets the medium, light, colour and texture only; \
                     the {noun}'s shape and what the painting shows stay exactly as described.",
                    noun = w.noun
                ));
            }
            background = Some(background_sentence(r, w.noun, family));
            constraints = format!(
                "Constraints: one {noun} only, centred and filling most of the frame; no extra \
                 tabs, sheets, flaps or stacked copies; no scenery, border or checkerboard \
                 pattern; no watermark or signature{no_text}.",
                noun = w.noun
            );
        }
        Shape::Icon => {
            paragraphs.push(format!(
                "Create {idea} as a single standalone icon: one complete object, centred, seen \
                 from a slight three-quarter front view, filling about four fifths of the frame \
                 with an even margin on every side and nothing cropped. It stands alone, with no \
                 floor, scenery or frame; at most a soft contact shadow directly beneath it."
            ));
            if let Some(style) = style {
                paragraphs.push(format!(
                    "Style: {style}. The style sets the medium, light, colour and texture only."
                ));
            }
            background = Some(background_sentence(r, "object", family));
            constraints = format!(
                "Constraints: one object only; no scenery, border or checkerboard pattern; no \
                 watermark or signature{no_text}."
            );
        }
    }

    // The last block, a line each: the pictures, the lettering, the background, the constraints.
    let mut lines = references(r, family);
    lines.extend(r.lettering.map(|l| lettering(l, family)));
    lines.extend(background.filter(|b| !b.is_empty()));
    lines.push(constraints);
    paragraphs.push(lines.join("\n"));
    paragraphs.join("\n\n")
}

/// What surrounds a whole shape or a free icon, by what the model can do: real transparency,
/// image 1's flat backdrop kept as it is, or a key colour named.
fn background_sentence(r: &Recipe, thing: &str, family: Family) -> String {
    match r.around {
        Around::Alpha => "The background is fully transparent.".to_string(),
        Around::Kept => format!(
            "Paint it in the middle of {} and leave the flat background around it exactly as it \
             is.",
            first(family)
        ),
        Around::Named(key) => format!(
            "Everything around the {thing} is one perfectly flat {}, {}, with no gradient, \
             texture, shadow or reflection, and the {thing} itself uses none of that colour.",
            key.name(),
            key.hex()
        ),
        Around::Nothing => String::new(),
    }
}

// ---------------------------------------------------------------- FLUX

fn flux(r: &Recipe) -> String {
    let idea = trim_sentence(r.idea);
    let w = words(r.base);
    let style_tail = r
        .treatment
        .map(|t| format!(", as {}", trim_sentence(&t.words)))
        .unwrap_or_default();
    let letters = flux_lettering(r);
    let family = Family::Flux;
    match r.painted() {
        Shape::Skin => {
            let refs: String = references(r, family)
                .into_iter()
                .map(|s| format!(" {s}"))
                .collect();
            let top = if w.corner.is_empty() {
                format!("the top {} of the picture", band(r.base))
            } else {
                format!(
                    "the top {} of the picture, and its {} corner,",
                    band(r.base),
                    w.corner
                )
            };
            // The one negation stays on evidence. Against "the painted scene continues past all
            // four edges", klein framed the same 1 picture in 6 (pop art and oil, three seeds
            // each) either way, but the positive wording signed 2 of the 3 oils and this one none.
            format!(
                "{}{style_tail}.{refs}{letters} The painted scene bleeds off all four edges of the \
                 image: no white border, no margin, no frame line and no paper edge anywhere \
                 around it. The main subject is large and sits in the centre, fully visible, with \
                 open space above it; {top} is only sky or plain background. Bold shapes and \
                 strong contrast that still read from across a room.",
                capitalised(idea)
            )
        }
        Shape::Folder if r.around == Around::Kept => {
            let paper = if w.paper.is_empty() {
                String::new()
            } else {
                format!("{} ", w.paper)
            };
            // The picture handed in is the template on the key colour. The colour is never
            // named: an edit model told about magenta paints the folder magenta. It is told to
            // leave the background alone instead, and the cut-out uses our silhouette.
            format!(
                "Turn the plain grey {noun} in {one} into a {noun} painted all over as \
                 {idea}{subject}{style_tail}. The painting covers the {noun}'s entire surface edge \
                 to edge, {surface}, like a printed wrap rather than a picture placed on it, with \
                 the main subject in the middle of {middle}.{looks}{letters} {paper}Keep the \
                 {noun}'s exact outline, {keeps}, size and position, and leave the background \
                 around it exactly as it is.",
                noun = w.noun,
                one = first(family),
                subject = subject_ref(r, family),
                looks = look_refs(r, family),
                surface = w.surface,
                middle = w.middle,
                keeps = w.keeps,
            )
        }
        Shape::Folder => {
            let built = if w.parts.is_empty() {
                format!("The {} is one solid object", w.noun)
            } else {
                format!(
                    "The {} is built from exactly {} parts, back to front: {}, one solid object",
                    w.noun,
                    number_word(w.parts.len()),
                    part_list(&w.parts)
                )
            };
            let paper = if w.paper.is_empty() {
                String::new()
            } else {
                format!("{} ", w.paper)
            };
            format!(
                "{}{subject}{style_tail}, painted all over one {kind} seen straight on. {built} \
                 with softly rounded edges and real thickness, and the painting covers {surface} \
                 as one continuous surface, with the main subject in the middle of \
                 {middle}.{looks}{letters} {paper}The {noun} is the only object in the picture, \
                 centred and filling most of it. {background}",
                capitalised(idea),
                subject = subject_ref(r, family),
                kind = w.kind,
                surface = w.surface,
                middle = w.middle,
                looks = look_refs(r, family),
                noun = w.noun,
                background = background_sentence(r, w.noun, family),
            )
        }
        Shape::Icon => {
            let (where_, around) = if r.around == Around::Kept {
                (
                    format!("in the middle of {}", first(family)),
                    "Leave the flat background around it exactly as it is.".to_string(),
                )
            } else {
                (
                    "in the middle of the picture".to_string(),
                    background_sentence(r, "object", family),
                )
            };
            format!(
                "{}{subject}{style_tail}. It is one single, complete object {where_}, filling \
                 about four fifths of it with an even margin on every side, nothing cropped, \
                 standing alone with only a soft contact shadow directly beneath \
                 it.{looks}{letters} {around}",
                capitalised(idea),
                subject = subject_ref(r, family),
                looks = look_refs(r, family),
            )
        }
    }
    .trim_end()
    .to_string()
}

// ---------------------------------------------------------------- design models

fn design(r: &Recipe) -> String {
    // A template to repaint reads best as FLUX's prose, which is how an edit model is asked.
    if r.painted() == Shape::Folder && r.around == Around::Kept {
        return flux(r).replace(
            &flux_lettering(r),
            &r.lettering
                .map(|l| lettering(l, Family::Design))
                .unwrap_or_default(),
        );
    }
    let idea = capitalised(trim_sentence(r.idea));
    let w = words(r.base);
    let style_tail = r
        .treatment
        .map(|t| format!(", as {}", trim_sentence(&t.words)))
        .unwrap_or_default();
    let letters = r
        .lettering
        .map(|l| lettering(l, Family::Design))
        .unwrap_or_default();
    let refs: String = references(r, Family::Design)
        .into_iter()
        .map(|s| format!(" {s}"))
        .collect();
    match r.painted() {
        Shape::Skin => {
            let top = if w.corner.is_empty() {
                format!("the top {}", band(r.base))
            } else {
                format!("the top {} and its {} corner", band(r.base), w.corner)
            };
            format!(
                "{idea}{style_tail}.{letters}{refs} The main subject is large, complete and \
                 centred, with open space above it and only calm background in {top}; the \
                 picture fills the frame edge to edge with bold shapes and strong contrast."
            )
        }
        Shape::Folder => {
            let built = if w.parts.is_empty() {
                String::new()
            } else {
                format!(
                    ", built from exactly {} parts, back to front: {}",
                    number_word(w.parts.len()),
                    part_list(&w.parts)
                )
            };
            let paper = if w.paper.is_empty() {
                String::new()
            } else {
                format!(" {}", w.paper)
            };
            format!(
                "{idea}{style_tail}, painted all over one {kind} seen straight on, as a finished \
                 icon{built}.{letters}{refs} The painting covers {surface} as one continuous \
                 surface, with the main subject in the middle of {middle}.{paper} The {noun} is \
                 the only object in the picture, centred and filling most of it. {background}",
                kind = w.kind,
                surface = w.surface,
                middle = w.middle,
                noun = w.noun,
                background = background_sentence(r, w.noun, Family::Design),
            )
            .trim_end()
            .to_string()
        }
        Shape::Icon => format!(
            "{idea}{style_tail}, as a single standalone icon: one complete object, centred, seen \
             from a slight three-quarter front view, filling about four fifths of the frame with \
             an even margin on every side, nothing cropped, standing alone with only a soft \
             contact shadow directly beneath it.{letters}{refs} {}",
            background_sentence(r, "object", Family::Design)
        )
        .trim_end()
        .to_string(),
    }
}

// ---------------------------------------------------------------- Stability

fn stability(r: &Recipe) -> String {
    let idea = trim_sentence(r.idea);
    let w = words(r.base);
    let style = r
        .treatment
        .map(|t| format!(", {}", trim_sentence(&t.words)))
        .unwrap_or_default();
    let letters = r
        .lettering
        .map(|l| lettering(l, Family::Stability))
        .unwrap_or_default();
    let background = match r.around {
        Around::Named(key) => format!(
            ", on a perfectly flat {} background, {}",
            key.name(),
            key.hex()
        ),
        Around::Alpha => ", on a transparent background".to_string(),
        Around::Kept | Around::Nothing => String::new(),
    };
    match r.painted() {
        Shape::Skin => format!(
            "{idea}{style}, large centred subject, open space above, full-bleed, bold shapes, \
             strong contrast{letters}"
        ),
        Shape::Folder => format!(
            "{idea}{style}, painted all over one {kind} seen straight on, the painting wrapped \
             across {surface}, the main subject in the middle of {middle}, one solid object, \
             centred{letters}{background}",
            kind = w.kind,
            surface = w.surface,
            middle = w.middle,
        ),
        Shape::Icon => format!(
            "{idea}{style}, a single standalone icon, one complete object centred with an even \
             margin, soft contact shadow{letters}{background}"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe::{Key, Treatment};
    use crate::styles::style;
    use folderskin_core::base::{BASES, FREE, MAC_FOLDER, WINDOWS_FOLDER};

    fn recipe<'a>(base: &'a Base, shape: Shape, idea: &'a str) -> Recipe<'a> {
        Recipe {
            idea,
            treatment: None,
            lettering: None,
            pictures: &[],
            base,
            shape,
            around: Around::Nothing,
        }
    }

    const FAMILIES: [Family; 5] = [
        Family::Instruct,
        Family::Grok,
        Family::Flux,
        Family::Design,
        Family::Stability,
    ];

    #[test]
    fn families_are_found_by_provider() {
        assert_eq!(family("openai"), Family::Instruct);
        assert_eq!(family("google"), Family::Instruct);
        assert_eq!(family("xai"), Family::Grok);
        assert_eq!(family("bfl"), Family::Flux);
        assert_eq!(family("local"), Family::Flux);
        assert_eq!(family("ideogram"), Family::Design);
        assert_eq!(family("recraft"), Family::Design);
        assert_eq!(family("stability"), Family::Stability);
    }

    #[test]
    fn the_idea_comes_first_word_for_word_and_the_style_after_it() {
        let woodblock = Treatment::of(style("woodblock").unwrap());
        let mut r = recipe(&MAC_FOLDER, Shape::Skin, "a koi pond at night.");
        r.treatment = Some(&woodblock);
        let instruct = render(&r, Family::Instruct);
        assert!(instruct.starts_with("Create a picture of a koi pond at night.\n\nStyle: a traditional woodblock-printed illustration: carved black keylines"), "{instruct}");
        assert!(instruct.contains("what the picture shows stays exactly as described above"));
        let flux = render(&r, Family::Flux);
        assert!(flux.starts_with("A koi pond at night, as a traditional woodblock-printed illustration: carved black keylines"), "{flux}");
        let design = render(&r, Family::Design);
        assert!(
            design.starts_with("A koi pond at night, as a traditional woodblock"),
            "{design}"
        );
        let stability = render(&r, Family::Stability);
        assert!(
            stability.starts_with("a koi pond at night, a traditional woodblock"),
            "{stability}"
        );
        // Without a style, no style at all.
        r.treatment = None;
        assert!(!render(&r, Family::Instruct).contains("Style:"));
        assert!(render(&r, Family::Flux).starts_with("A koi pond at night. The painted scene"));
    }

    #[test]
    fn artwork_is_never_called_a_folder_or_an_icon() {
        for base in [&MAC_FOLDER, &WINDOWS_FOLDER] {
            for f in FAMILIES {
                let p = render(&recipe(base, Shape::Skin, "a copper patina"), f);
                let lower = p.to_lowercase();
                assert!(
                    !lower.contains("folder") && !lower.contains("icon"),
                    "{f:?}: {p}"
                );
                assert!(
                    !p.contains("1024") && !p.contains(" by "),
                    "no sizes in prose: {p}"
                );
            }
        }
    }

    #[test]
    fn artwork_keeps_the_band_the_tab_hides_calm() {
        let mac = render(&recipe(&MAC_FOLDER, Shape::Skin, "koi"), Family::Instruct);
        assert!(
            mac.contains("the top eighth of the picture holds only sky"),
            "{mac}"
        );
        let windows = render(
            &recipe(&WINDOWS_FOLDER, Shape::Skin, "koi"),
            Family::Instruct,
        );
        assert!(
            windows
                .contains("the top sixth of the picture and its upper-left corner hold only sky"),
            "{windows}"
        );
        let flux = render(&recipe(&WINDOWS_FOLDER, Shape::Skin, "koi"), Family::Flux);
        assert!(
            flux.contains("the top sixth of the picture, and its upper-left corner, is only sky"),
            "{flux}"
        );
        assert!(
            render(&recipe(&MAC_FOLDER, Shape::Skin, "koi"), Family::Design)
                .contains("only calm background in the top eighth;")
        );
        // Detail is welcome: a small icon needs a silhouette, not less of it.
        assert!(mac.contains("fine texture and detail inside the shapes are welcome"));
        assert!(!mac.contains("no fine detail"));
    }

    #[test]
    fn instruction_models_get_their_exclusions_in_one_line_at_the_end() {
        let p = render(&recipe(&MAC_FOLDER, Shape::Skin, "a fox"), Family::Instruct);
        assert!(p.ends_with("Constraints: no borders, frames, paper margins or vignettes; no watermark or signature; no text, letters or numbers."), "{p}");
        // FLUX is told nothing about text, and names nothing it shouldn't paint but the border.
        let flux = render(&recipe(&MAC_FOLDER, Shape::Skin, "a fox"), Family::Flux);
        assert!(
            !flux.contains("text") && !flux.contains("letters"),
            "{flux}"
        );
        for shape in [Shape::Folder, Shape::Icon] {
            let mut r = recipe(&MAC_FOLDER, shape, "a fox");
            r.around = Around::Kept;
            r.pictures = &[Role::Template];
            let p = render(&r, Family::Flux);
            for word in [" no ", "without", "don't", "avoid"] {
                assert!(!p.contains(word), "{shape:?} says {word:?}: {p}");
            }
        }
    }

    #[test]
    fn a_whole_folder_repaints_image_one_and_never_names_the_key() {
        let roles = [Role::Template];
        let mut r = recipe(&MAC_FOLDER, Shape::Folder, "a copper patina");
        r.around = Around::Kept;
        r.pictures = &roles;
        let p = render(&r, Family::Instruct);
        assert!(p.starts_with("Image 1 is a blank macOS-style folder: the exact shape to paint on. Keep its outline, its single tab at the top left, the pale paper strip between the back and front panels, its size"), "{p}");
        assert!(p.contains("Paint only the folder's surface: a copper patina, wrapped across the back panel, the tab and the front panel"), "{p}");
        assert!(p.contains("Keep the paper strip plain or faintly tinted."));
        assert!(p.ends_with("keep everything else in image 1 the same; no extra tabs, sheets, flaps or stacked copies; no watermark or signature; no text, letters or numbers."), "{p}");
        let grok = render(&r, Family::Grok);
        assert!(
            grok.starts_with("<IMAGE_0> is a blank macOS-style folder"),
            "{grok}"
        );
        assert!(
            grok.contains("keep everything else in <IMAGE_0> the same"),
            "{grok}"
        );
        let flux = render(&r, Family::Flux);
        assert!(flux.starts_with("Turn the plain grey folder in image 1 into a folder painted all over as a copper patina. The painting covers"), "{flux}");
        assert!(flux.contains("The thin paper strip between the panels stays pale cream. Keep the folder's exact outline, its single tab at the top left"), "{flux}");
        for f in FAMILIES {
            let p = render(&r, f).to_lowercase();
            for word in ["magenta", "#ff00ff", "green", "#00ff00"] {
                assert!(!p.contains(word), "{f:?} names {word}: {p}");
            }
        }
        // Windows' folder has its own parts, and no paper.
        r.base = &WINDOWS_FOLDER;
        let w = render(&r, Family::Instruct);
        assert!(
            w.contains("Image 1 is a blank Windows-style folder")
                && w.contains("the curved step where the front panel rises to meet it"),
            "{w}"
        );
        assert!(!w.contains("paper"), "{w}");
        assert!(!render(&r, Family::Flux).contains("paper"));
    }

    #[test]
    fn a_whole_folder_from_words_is_built_from_its_parts_on_its_background() {
        let mut r = recipe(&MAC_FOLDER, Shape::Folder, "koi");
        r.around = Around::Named(Key::Magenta);
        let p = render(&r, Family::Instruct);
        assert!(p.starts_with("Create one macOS-style folder, seen straight on, as a finished icon. It is built from exactly three parts, back to front: (1) a back panel"), "{p}");
        assert!(p.contains("Everything around the folder is one perfectly flat magenta, #FF00FF, with no gradient"), "{p}");
        r.around = Around::Alpha;
        assert!(render(&r, Family::Instruct).contains("The background is fully transparent."));
        assert!(render(&r, Family::Design).ends_with("The background is fully transparent."));
        r.around = Around::Named(Key::Green);
        let flux = render(&r, Family::Flux);
        assert!(flux.starts_with("Koi, painted all over one macOS-style folder seen straight on. The folder is built from exactly three parts"), "{flux}");
        assert!(flux.ends_with("one perfectly flat green, #00FF00, with no gradient, texture, shadow or reflection, and the folder itself uses none of that colour."), "{flux}");
        let stability = render(&r, Family::Stability);
        assert!(
            stability.ends_with("on a perfectly flat green background, #00FF00"),
            "{stability}"
        );
        let windows = render(
            &Recipe {
                base: &WINDOWS_FOLDER,
                ..r
            },
            Family::Instruct,
        );
        assert!(
            windows.contains("exactly two parts") && !windows.contains("paper"),
            "{windows}"
        );
    }

    #[test]
    fn a_free_icon_is_one_object_on_what_the_model_can_give() {
        let mut r = recipe(&FREE, Shape::Skin, "a cheerful fox mascot.");
        r.around = Around::Alpha;
        let p = render(&r, Family::Instruct);
        assert!(
            p.starts_with(
                "Create a cheerful fox mascot as a single standalone icon: one complete object"
            ),
            "{p}"
        );
        assert!(p.contains("The background is fully transparent.\nConstraints: one object only; no scenery, border or checkerboard pattern"), "{p}");
        r.around = Around::Named(Key::Magenta);
        assert!(render(&r, Family::Instruct)
            .contains("Everything around the object is one perfectly flat magenta, #FF00FF"));
        // The local model paints it on a canvas it is handed, and is never told the colour.
        r.around = Around::Kept;
        r.pictures = &[Role::Canvas];
        let flux = render(&r, Family::Flux);
        assert_eq!(flux, "A cheerful fox mascot. It is one single, complete object in the middle of image 1, filling about four fifths of it with an even margin on every side, nothing cropped, standing alone with only a soft contact shadow directly beneath it. Leave the flat background around it exactly as it is.");
        let design = render(
            &Recipe {
                around: Around::Alpha,
                pictures: &[],
                ..r
            },
            Family::Design,
        );
        assert!(
            design.starts_with("A cheerful fox mascot, as a single standalone icon")
                && design.ends_with("The background is fully transparent."),
            "{design}"
        );
        for f in FAMILIES {
            assert!(!render(&r, f).contains("tab"), "{f:?}");
        }
    }

    #[test]
    fn pictures_are_named_by_number_and_role() {
        let roles = [
            Role::Template,
            Role::Subject,
            Role::Subject,
            Role::Style,
            Role::Palette,
        ];
        let mut r = recipe(&MAC_FOLDER, Shape::Folder, "our dog Biscuit");
        r.around = Around::Kept;
        r.pictures = &roles;
        let p = render(&r, Family::Instruct);
        assert!(p.contains("Images 2 and 3 show the subject: keep it recognisably the same, with its shape, colours and markings, while painting it as described."), "{p}");
        assert!(p.contains("Image 4 is a style reference only: match its medium, palette, light and texture, and take none of its content or layout."), "{p}");
        assert!(
            p.contains(
                "Image 5 is a palette reference: use its colours, and nothing else from it."
            ),
            "{p}"
        );
        let grok = render(&r, Family::Grok);
        assert!(
            grok.contains("<IMAGE_1> and <IMAGE_2> show the subject"),
            "{grok}"
        );
        let flux = render(&r, Family::Flux);
        assert!(flux.contains("painted all over as our dog Biscuit, taking the subject from images 2 and 3. The painting"), "{flux}");
        assert!(
            flux.contains(" Image 4 is a style reference only"),
            "{flux}"
        );
        // Artwork has no template: the person's picture is image 1.
        let own = [Role::Subject];
        let art = Recipe {
            pictures: &own,
            shape: Shape::Skin,
            around: Around::Nothing,
            ..r
        };
        assert!(render(&art, Family::Instruct).contains("\n\nImage 1 shows the subject"));
        assert!(
            render(&art, Family::Flux).starts_with("Our dog Biscuit. Image 1 shows the subject")
        );
    }

    #[test]
    fn lettering_has_a_slot_of_its_own_in_every_family() {
        let screen = Treatment::of(style("screenprint").unwrap());
        let l = Lettering::of(
            "a lagoon that says \"ESCAPE\"",
            Some(&screen),
            &MAC_FOLDER,
            Shape::Skin,
        )
        .unwrap();
        let mut r = recipe(&MAC_FOLDER, Shape::Skin, "a lagoon that says \"ESCAPE\"");
        r.treatment = Some(&screen);
        r.lettering = Some(&l);
        let p = render(&r, Family::Instruct);
        assert!(p.contains("Lettering: the words \"ESCAPE\" (E-S-C-A-P-E) appear exactly once, in bold retro sans-serif letters in one flat ink, large and easy to read, across the middle of the picture. There is no other text anywhere."), "{p}");
        assert!(
            p.ends_with("no watermark or signature."),
            "text is asked for: {p}"
        );
        let flux = render(&r, Family::Flux);
        assert!(flux.contains(". The words \"ESCAPE\" are written once in bold retro sans-serif letters in one flat ink, large and centred across the middle of the picture. The painted scene"), "{flux}");
        assert!(render(&r, Family::Design).contains(". The words \"ESCAPE\" in bold retro sans-serif letters in one flat ink, large, across the middle of the picture. The main subject"));
        assert!(render(&r, Family::Stability)
            .ends_with(", the words \"ESCAPE\" in bold retro sans-serif letters in one flat ink"));
        // A logo or a brand the idea names isn't banned: only text nobody asked for.
        assert!(!p.to_lowercase().contains("logo"));
    }

    #[test]
    fn every_base_and_shape_makes_a_whole_prompt_in_every_family() {
        for base in BASES {
            for shape in [Shape::Skin, Shape::Folder, Shape::Icon] {
                for around in [
                    Around::Nothing,
                    Around::Alpha,
                    Around::Kept,
                    Around::Named(Key::Magenta),
                ] {
                    for f in FAMILIES {
                        let pictures: &[Role] = match (shape.on(base), around) {
                            (Shape::Folder, Around::Kept) => &[Role::Template],
                            (Shape::Icon, Around::Kept) => &[Role::Canvas],
                            _ => &[],
                        };
                        let r = Recipe {
                            around,
                            pictures,
                            ..recipe(base, shape, "a lighthouse")
                        };
                        let p = render(&r, f);
                        assert!(
                            p.to_lowercase().contains("a lighthouse"),
                            "{} {shape:?} {f:?}: {p}",
                            base.id
                        );
                        assert!(
                            !p.contains('{') && !p.contains("  "),
                            "{} {shape:?} {f:?}: {p}",
                            base.id
                        );
                        assert_eq!(p, p.trim(), "{} {shape:?} {f:?}", base.id);
                    }
                }
            }
        }
    }

    #[test]
    fn a_base_the_registry_grows_is_described_by_its_name() {
        let drive = Base {
            id: "mac-drive",
            label: "Mac drive",
            system: folderskin_core::base::System::Mac,
            family: BaseFamily::Drive,
            template: Some(folderskin_core::compositor::Style::Mac),
            anatomy: None,
        };
        let mut r = recipe(&drive, Shape::Folder, "koi");
        r.around = Around::Kept;
        r.pictures = &[Role::Template];
        let p = render(&r, Family::Instruct);
        assert!(p.starts_with("Image 1 is a blank mac drive: the exact shape to paint on. Keep its outline, every part of its shape"), "{p}");
        assert!(
            p.contains("Paint only the drive's surface: koi, wrapped across its whole face"),
            "{p}"
        );
        r.around = Around::Named(Key::Magenta);
        r.pictures = &[];
        assert!(render(&r, Family::Instruct)
            .contains("It is one solid object with softly rounded corners"));
        assert_eq!(number_word(12), "12");
    }

    #[test]
    fn shape_ids_round_trip_and_presets_are_subjects() {
        for s in [Shape::Skin, Shape::Folder, Shape::Icon] {
            assert_eq!(Shape::from_id(s.id()), Some(s));
        }
        assert_eq!(Shape::from_id("nope"), None);
        let mut ids: Vec<&str> = PRESETS.iter().map(|p| p.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), PRESETS.len());
        for p in PRESETS {
            assert!(!p.idea.is_empty() && !p.label.is_empty());
            // No colour the magenta cut-out would eat, and no style: that goes in its own slot.
            for word in [
                "magenta",
                "pink",
                "print",
                "painting",
                "poster",
                "woodblock",
                "style",
            ] {
                assert!(!p.idea.contains(word), "{}: {word}", p.id);
            }
        }
    }
}
