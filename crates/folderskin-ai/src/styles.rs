//! The built-in styles: every look the chat's "/" menu offers, in one table.
//!
//! The table is `styles.json`, beside this file, and the app's window reads the same file
//! (src/lib/styles.ts), so a style is added or changed in one place. Each row has:
//!
//! * `id`, which requests, saved prompts and the window's translations (`ai.styles.<id>`,
//!   `ai.styleDescriptions.<id>`) name it by, and `aliases`, the ids it had before, so an older
//!   chat or a command line script still finds it;
//! * `group`, the heading it is listed under, and `name` and `description`, in English: what the
//!   chat shows, as `src/locales/en/ai.json` says it (a test there keeps the two the same);
//! * `fragment`: the words that paint it, which go in the recipe's style slot
//!   ([`crate::recipe::Treatment`]). It describes the treatment only (medium, technique, surface,
//!   light, palette), never a subject, and never a word that paints something around the art
//!   ("poster", "canvas", "frame", "print" on its own), because a model draws what it is told
//!   about. It says what to paint, never what to avoid: FLUX has no negative prompt;
//! * `lettering`: how words asked for look in it, so they belong to the medium;
//! * `checks`: three traits a judge must see in a picture made in it;
//! * `keep_out`: what the style tends to add that nobody asked for. It is only ever sent where a
//!   provider has a negative prompt, and checked by the judge;
//! * `realistic`: a photographic or 3D look, where cartoon and clip art are kept out;
//! * `green_key`: a look full of pink or violet, cut out of a green backdrop instead of magenta;
//! * `native`: the provider's own preset for the same look, where one exists (Stability's
//!   `style_preset`, Ideogram's `style_type` and `style_preset`);
//! * `tag`: what a picture made in it is tagged with, and `words`: how someone's own words give
//!   the style away, a pattern the window matches.

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// A provider's own switch for a look.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Native {
    /// Stability's `style_preset`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stability: Option<String>,
    /// Ideogram 3.0's `style_type`: REALISTIC, DESIGN, FICTION or GENERAL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ideogram_type: Option<String>,
    /// Ideogram 3.0's `style_preset`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ideogram_preset: Option<String>,
}

/// One built-in style.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Style {
    pub id: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub group: String,
    pub name: String,
    pub description: String,
    pub fragment: String,
    pub lettering: String,
    pub checks: Vec<String>,
    #[serde(default)]
    pub keep_out: Vec<String>,
    #[serde(default)]
    pub realistic: bool,
    #[serde(default)]
    pub green_key: bool,
    #[serde(default)]
    pub native: Native,
    pub tag: String,
    #[serde(default)]
    pub words: String,
}

/// A heading styles are listed under.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Group {
    pub id: String,
    pub name: String,
}

#[derive(Deserialize)]
struct Table {
    groups: Vec<Group>,
    styles: Vec<Style>,
}

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        serde_json::from_str::<Table>(include_str!("styles.json")).unwrap_or(Table {
            groups: Vec::new(),
            styles: Vec::new(),
        })
    })
}

/// The table, read once. It is compiled in, and a test proves it reads, so this can't fail at
/// run time.
pub fn styles() -> &'static [Style] {
    &table().styles
}

/// The headings, in the order the menu lists them.
pub fn groups() -> &'static [Group] {
    &table().groups
}

/// The style called `id`, or that was called `id` before.
pub fn style(id: &str) -> Option<&'static Style> {
    let id = id.trim();
    styles()
        .iter()
        .find(|s| s.id == id)
        .or_else(|| styles().iter().find(|s| s.aliases.iter().any(|a| a == id)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_reads_and_every_row_is_whole() {
        let table: Table = serde_json::from_str(include_str!("styles.json")).unwrap();
        assert_eq!(table.styles.len(), styles().len());
        assert_eq!(styles().len(), 30, "the research's thirty");
        let mut ids: Vec<&str> = styles()
            .iter()
            .flat_map(|s| {
                std::iter::once(s.id.as_str()).chain(s.aliases.iter().map(String::as_str))
            })
            .collect();
        let all = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), all, "ids and aliases are all different");
        for s in styles() {
            for (field, value) in [
                ("id", &s.id),
                ("name", &s.name),
                ("description", &s.description),
                ("fragment", &s.fragment),
                ("lettering", &s.lettering),
                ("tag", &s.tag),
                ("words", &s.words),
            ] {
                assert!(!value.trim().is_empty(), "{} has no {field}", s.id);
            }
            assert_eq!(s.checks.len(), 3, "{}: three traits to check", s.id);
            assert!(
                groups().iter().any(|g| g.id == s.group),
                "{}: no group {}",
                s.id,
                s.group
            );
            assert!(
                s.id.bytes().all(|b| b.is_ascii_lowercase() || b == b'-'),
                "{}",
                s.id
            );
            assert_eq!(style(&s.id), Some(s));
            for alias in &s.aliases {
                assert_eq!(style(alias), Some(s), "{alias}");
            }
        }
        assert_eq!(style("nope"), None);
        // The ids chats and scripts used before find their styles.
        for (old, new) in [
            ("travel", "screenprint"),
            ("travel-poster", "screenprint"),
            ("ukiyoe", "woodblock"),
            ("nouveau", "art-nouveau"),
            ("diorama", "miniature"),
            ("riso", "risograph"),
            ("sketch", "pencil"),
            (" oil ", "oil"),
        ] {
            assert_eq!(style(old).map(|s| s.id.as_str()), Some(new), "{old}");
        }
    }

    #[test]
    fn fragments_describe_a_treatment_and_nothing_else() {
        for s in styles() {
            let f = s.fragment.to_lowercase();
            let words = f.split_whitespace().count();
            assert!(
                (15..=35).contains(&words),
                "{}: {words} words, not 15 to 35",
                s.id
            );
            // "a <medium>: <technique>, …", the pattern every template drops it into.
            let (medium, _) = f
                .split_once(": ")
                .unwrap_or_else(|| panic!("{}: no colon", s.id));
            assert!(!medium.is_empty() && !medium.contains(','), "{}", s.id);
            assert!(!f.ends_with('.'), "{}: a fragment goes mid-sentence", s.id);
            // Naming a thing paints it, on a model with no negative prompt.
            for word in [" no ", "without", "avoid", "don't", "not ", "never"] {
                assert!(!f.contains(word), "{} says {word:?}", s.id);
            }
            // Words that paint an object around the art ("full-frame" is the film look's own).
            for word in [
                " poster", " canvas", " frame", " border", " card", " sticker", " badge", " label",
                " sign ", " page", " sheet", " diorama", " museum", " gallery",
            ] {
                assert!(!f.contains(word), "{} says {word:?}", s.id);
            }
            // The magenta cut-out would eat these.
            for word in ["magenta", "fuchsia", "pink"] {
                assert!(!f.contains(word), "{} asks for {word}", s.id);
            }
            for word in [" no ", "without", "never"] {
                assert!(!s.lettering.contains(word), "{}'s lettering", s.id);
            }
            // The words the window tags a picture by find the style in its own fragment. The
            // patterns are plain alternatives, with at most a word boundary and an optional space.
            let found = s
                .words
                .split('|')
                .any(|alt| f.contains(&alt.replace("\\b", "").replace(" ?", " ").to_lowercase()));
            assert!(
                found,
                "{}: {:?} finds nothing in {:?}",
                s.id, s.words, s.fragment
            );
        }
    }

    #[test]
    fn violet_and_pink_looks_are_keyed_on_green() {
        for s in styles() {
            let f = s.fragment.to_lowercase();
            if f.contains("violet") || f.contains("purple") {
                assert!(s.green_key, "{} paints violet on a magenta key", s.id);
            }
        }
        assert!(
            style("pop-art").unwrap().green_key,
            "its dots come out magenta"
        );
        assert!(!style("photo").unwrap().green_key);
    }

    #[test]
    fn native_switches_name_presets_the_providers_take() {
        use crate::request::{IDEOGRAM_STYLE_PRESETS, IDEOGRAM_STYLE_TYPES, STABILITY_PRESETS};
        for s in styles() {
            if let Some(p) = &s.native.stability {
                assert!(STABILITY_PRESETS.contains(&p.as_str()), "{}: {p}", s.id);
            }
            if let Some(t) = &s.native.ideogram_type {
                assert!(IDEOGRAM_STYLE_TYPES.contains(&t.as_str()), "{}: {t}", s.id);
            }
            if let Some(p) = &s.native.ideogram_preset {
                assert!(
                    IDEOGRAM_STYLE_PRESETS.contains(&p.as_str()),
                    "{}: {p}",
                    s.id
                );
            }
        }
        // A cartoon preset never stands in for a render.
        assert_eq!(
            style("render").unwrap().native.stability.as_deref(),
            Some("3d-model")
        );
    }
}
