//! Prompt composition.
//!
//! Two shapes of output are useful for a folder icon, and they need different instructions:
//!
//! * [`Shape::Skin`] asks for flat artwork that FolderSkin's own compositor wraps onto the
//!   folder. This is the normal path: the geometry stays ours, so every skin lines up.
//! * [`Shape::Folder`] asks the model to draw the whole folder as one object on a key colour.
//!   It gives up pixel-exact geometry in exchange for artwork that can sit in real relief and
//!   break over the folder's top edge. The caller cuts the key colour out.
//!
//! The wording below is FolderSkin's own. What makes it work is structural, not stylistic:
//! state the subject, forbid the failure modes by name, and end with a hard output contract.

/// What the model should draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// Flat artwork, 1024 x 958, that the compositor wraps onto the folder.
    Skin,
    /// A complete folder object on a key-colour backdrop, used as the icon directly.
    Folder,
}

impl Shape {
    pub fn id(self) -> &'static str {
        match self {
            Shape::Skin => "skin",
            Shape::Folder => "folder",
        }
    }

    pub fn from_id(id: &str) -> Option<Shape> {
        match id {
            "skin" => Some(Shape::Skin),
            "folder" => Some(Shape::Folder),
            _ => None,
        }
    }
}

/// A one-line starting point the user can pick instead of writing a prompt.
pub struct Preset {
    pub id: &'static str,
    pub label: &'static str,
    pub idea: &'static str,
}

/// Starting points offered in the Generate view, one per built-in collection plus a few extras.
pub const PRESETS: &[Preset] = &[
    Preset { id: "aurora", label: "Aurora", idea: "a night sky with green and violet aurora ribbons over dark mountains, faint stars" },
    Preset { id: "dunes", label: "Dunes", idea: "warm desert dunes at golden hour, long soft shadows, fine sand grain" },
    Preset { id: "risograph", label: "Risograph", idea: "two-colour risograph print, coarse grain, misregistered fluorescent pink and blue" },
    Preset { id: "terrazzo", label: "Terrazzo", idea: "pale terrazzo with scattered chips of teal, ochre and charcoal" },
    Preset { id: "wave", label: "Wave", idea: "a woodblock ocean wave with deep indigo troughs and white foam" },
    Preset { id: "circuit", label: "Circuit", idea: "an emerald circuit board macro, gold traces, soft bokeh highlights" },
    Preset { id: "linen", label: "Linen", idea: "undyed linen weave in raking light, visible slubs and thread texture" },
    Preset { id: "nebula", label: "Nebula", idea: "a magenta and cyan nebula with dust lanes and scattered stars" },
];

/// Style notes appended to a `Skin` prompt so a generated skin sits beside the built-in ten.
const SKIN_CONTRACT: &str = "\
Fill the entire frame with the artwork: a flat, evenly composed surface design with no border, \
no frame, no vignette and no letterboxing. Keep the composition readable when it is shrunk to a \
32 pixel icon: bold shapes, clear contrast, no fine detail that turns to mush. Put nothing \
important in the top eighth of the image or within 6% of any edge, because those bands are \
cropped or curve out of sight. No text, letters, numbers, logos, watermarks or signatures. No \
folder, no icon, no user interface, no device, no mockup and no drop shadow: this is the \
surface pattern only. Photographic or painterly is fine; the frame must stay flat and square-on.";

/// The folder construction rules for a `Folder` prompt.
///
/// The three-part description and the explicit "do not add layers" clause are what stop a model
/// drawing stacked folders, double tabs or extra paper sheets.
const FOLDER_CONTRACT: &str = "\
Draw one macOS-style folder, seen straight on, built from exactly three parts and no others: \
(1) a back panel with a single rounded tab at its top left, the only tab in the picture; \
(2) one plain paper sheet tucked inside, with just its top edge showing in the gap between the \
panels; (3) a front panel covering the lower two thirds, nearest the viewer. Exactly one tab, \
exactly one visible paper edge, exactly one front panel. Do not add extra flaps, folds, sheets, \
stacked folders, second tabs or any further layers; if you are unsure whether to add something, \
leave it out. Render the three parts as one solid object with real thickness: softly rounded \
edges, and one soft shadow cast by each part onto the part behind it. Wrap the artwork across \
the back panel and the front panel so the two read as one continuous surface; keep the paper \
sheet plain or give it only a faint tint. Where the artwork has depth, let its hero element \
stand out in relief and break over the folder's top edge; keep the folder itself square-on. \
No text, letters, numbers, logos or watermarks.";

/// The output contract: size, isolation and the key colour to be removed afterwards.
fn output_contract(width: u32, height: u32, key_hex: Option<&str>) -> String {
    let mut s = format!("Output: one high-resolution render, {width} by {height} pixels. ");
    match key_hex {
        Some(hex) => s.push_str(&format!(
            "The folder is the only object in the frame, centred, filling it edge to edge with a \
             few pixels of margin. Place it on a completely flat background of pure {hex} with no \
             gradient, texture, shading, reflection or noise anywhere in it, and keep that colour \
             out of the folder itself."
        )),
        None => s.push_str(
            "The subject is the only thing in the frame and reaches all four edges. Return the \
             image with a transparent background.",
        ),
    }
    s
}

/// Builds the prompt sent to the provider.
///
/// `idea` is the user's own words. `key_hex` is `Some("#FF00FF")` when the caller intends to key
/// the background out, `None` when the model returns a real alpha channel.
pub fn compose(shape: Shape, idea: &str, width: u32, height: u32, key_hex: Option<&str>) -> String {
    let idea = idea.trim();
    match shape {
        Shape::Skin => format!(
            "Create the artwork for a folder icon skin: {idea}.\n\n{SKIN_CONTRACT}\n\nOutput: one \
             high-resolution image, {width} by {height} pixels, filled edge to edge with the \
             artwork and nothing else."
        ),
        Shape::Folder => format!(
            "Create a folder icon illustration: {idea}.\n\n{FOLDER_CONTRACT}\n\n{}",
            output_contract(width, height, key_hex)
        ),
    }
}

/// Extra instructions for a run that also sends a reference picture.
pub fn compose_with_reference(shape: Shape, idea: &str, width: u32, height: u32, key_hex: Option<&str>) -> String {
    let base = compose(shape, idea, width, height, key_hex);
    let lead = match shape {
        Shape::Skin => "Use the supplied picture as the source of the artwork: keep its subject, \
                        palette and mood, and restyle it to fill the frame as described below.",
        Shape::Folder => "Use the supplied picture as the artwork that goes onto the folder: keep \
                          its subject, palette and mood, and wrap it across the panels as \
                          described below.",
    };
    format!("{lead}\n\n{base}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skin_prompts_forbid_drawing_a_folder() {
        let p = compose(Shape::Skin, "a copper patina", 1024, 958, None);
        assert!(p.contains("a copper patina"));
        assert!(p.contains("No folder"), "a skin prompt must rule out drawing the folder itself");
        assert!(p.contains("1024 by 958"));
        assert!(!p.contains("tab"), "skin prompts never mention folder construction: {p}");
    }

    #[test]
    fn folder_prompts_pin_the_construction_and_the_key_colour() {
        let p = compose(Shape::Folder, "a copper patina", 1166, 1091, Some("#FF00FF"));
        for needle in ["exactly three parts", "only tab", "pure #FF00FF", "1166 by 1091"] {
            assert!(p.contains(needle), "missing {needle:?} in: {p}");
        }
    }

    #[test]
    fn a_transparent_capable_model_is_not_told_about_a_key_colour() {
        let p = compose(Shape::Folder, "a copper patina", 1024, 1024, None);
        assert!(p.contains("transparent background"));
        assert!(!p.contains("#FF00FF"));
    }

    #[test]
    fn reference_runs_lead_with_the_picture() {
        let p = compose_with_reference(Shape::Skin, "keep it moody", 1024, 958, None);
        assert!(p.starts_with("Use the supplied picture"));
        assert!(p.contains("keep it moody"));
    }

    #[test]
    fn shape_ids_round_trip_and_presets_are_unique() {
        for s in [Shape::Skin, Shape::Folder] {
            assert_eq!(Shape::from_id(s.id()), Some(s));
        }
        assert_eq!(Shape::from_id("nope"), None);
        let mut ids: Vec<&str> = PRESETS.iter().map(|p| p.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), PRESETS.len());
        assert!(PRESETS.iter().all(|p| !p.idea.is_empty() && !p.label.is_empty()));
    }
}
