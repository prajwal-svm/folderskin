//! The shapes a skin is made for, which the app calls shapes: a folder in each system's look, and
//! a free icon with no base at all.
//!
//! A base with a template is one the compositor draws: artwork is wrapped onto it, its bare
//! picture is what a picker shows, and an image model that can work from a picture is shown it
//! blank to repaint ([`Base::blank`]), then cut out along its own silhouette
//! ([`Base::blank_cutout`]). A free icon has none of that: a mascot, an object or a character is
//! drawn standing on its own and used as it is.
//!
//! [`BASES`] is every base the app offers, in the order a picker lists them. The webview gets
//! them from one command and names each by its [`Base::id`]; [`Base::family`] is where its skins
//! go in the library (folder skins with folders, drive skins with drives, free icons anywhere).
//!
//! Each base with a template also says, in words, how it's built ([`Anatomy`]). Every prompt that
//! paints a base, for a provider or for the local model, is made from those words, so a base added
//! here is described to a model without a sentence written for it anywhere else.

use crate::compositor::{self, Artwork, IconSet, Style, SKIN_WIDTH};
use image::RgbaImage;

/// Which system's look a base has.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum System {
    Mac,
    Windows,
    Linux,
    /// Every system's: a free icon looks the same wherever it goes.
    Any,
}

impl System {
    pub fn id(self) -> &'static str {
        match self {
            System::Mac => "mac",
            System::Windows => "windows",
            System::Linux => "linux",
            System::Any => "any",
        }
    }
}

/// What a base is, and so where its skins go in the library.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Family {
    Folder,
    Drive,
    /// No base: the picture is the whole icon, and it goes on anything.
    Free,
}

impl Family {
    pub fn id(self) -> &'static str {
        match self {
            Family::Folder => "folder",
            Family::Drive => "drive",
            Family::Free => "free",
        }
    }
}

/// How a base is built, in the words an image model is given: what it is, its parts, what a
/// repaint of its template keeps and where artwork goes on it. It is written from the base's
/// geometry (`geometry.rs`, `geometry_windows.rs`), part by part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Anatomy {
    /// What it is, in a word: "folder".
    pub noun: &'static str,
    /// What it is as its system draws it: "macOS-style folder".
    pub kind: &'static str,
    /// Its parts from the back to the front, each as a model that draws it from words alone is
    /// told to build it.
    pub parts: &'static [&'static str],
    /// What a repaint of its template keeps, besides its outline, size and position.
    pub keeps: &'static str,
    /// The part that marks its shape, in a word, which a repaint keeps with its outline: "tab".
    /// Empty when its outline says it all.
    pub landmark: &'static str,
    /// What artwork is wrapped across.
    pub surface: &'static str,
    /// Where the main subject sits.
    pub middle: &'static str,
    /// How a part the artwork doesn't cover stays, as a sentence; empty when every part is painted.
    pub unpainted: &'static str,
}

/// One shape a skin can be made for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Base {
    /// How the webview and a saved skin name it: "mac-folder".
    pub id: &'static str,
    /// Its name in English, as the composer names the same shape ("Mac folder"). The app shows
    /// it in the language on show, by `id`.
    pub label: &'static str,
    pub system: System,
    pub family: Family,
    /// The compositor's template for it, or `None` for a free icon, which is used as it is drawn.
    pub template: Option<Style>,
    /// How it is built, for the prompts that paint it; `None` for a free icon.
    pub anatomy: Option<Anatomy>,
}

/// FolderSkin's own folder, as Finder shows it.
pub const MAC_FOLDER: Base = Base {
    id: "mac-folder",
    label: "Mac folder",
    system: System::Mac,
    family: Family::Folder,
    template: Some(Style::Mac),
    anatomy: Some(Anatomy {
        noun: "folder",
        kind: "macOS-style folder",
        parts: &[
            "a back panel with a single rounded tab at its top left, the only tab in the picture",
            "one plain paper sheet tucked inside, with just its top edge showing in the gap \
             between the panels",
            "a front panel covering the lower two thirds, nearest the viewer",
        ],
        keeps: "the only tab (rounded, at the top left), the pale paper strip showing between the \
                back and front panels",
        landmark: "tab",
        surface: "the back panel, the tab and the front panel",
        middle: "the front panel",
        unpainted: "The thin paper strip between the panels stays pale cream.",
    }),
};

/// The folder Windows 11 draws: two panels and no paper, the front's top edge dipping under the
/// tab (`geometry_windows.rs`).
pub const WINDOWS_FOLDER: Base = Base {
    id: "windows-folder",
    label: "Windows folder",
    system: System::Windows,
    family: Family::Folder,
    template: Some(Style::Windows),
    anatomy: Some(Anatomy {
        noun: "folder",
        kind: "Windows-style folder",
        parts: &[
            "a back panel with a single tab at its top left, the only tab in the picture, its top \
             edge stepping down in a gentle curve from the tab to the rest of the panel",
            "a front panel covering most of the back panel, nearest the viewer, whose top edge \
             dips lower under the tab and rises in a gentle curve to run straight across the rest",
        ],
        keeps: "the only tab (at the top left), the gentle curve where the front panel's top edge \
                dips under the tab",
        landmark: "tab",
        surface: "the back panel, the tab and the front panel",
        middle: "the front panel",
        unpainted: "Where the back panel shows above the front one it stays a shade darker than \
                    the front.",
    }),
};

/// No base at all: a mascot, an object or a character standing on its own.
pub const FREE: Base = Base {
    id: "free",
    label: "Free icon",
    system: System::Any,
    family: Family::Free,
    template: None,
    anatomy: None,
};

/// Every base, in the order a picker lists them: the folders, then the free icon.
pub const BASES: &[Base] = &[MAC_FOLDER, WINDOWS_FOLDER, FREE];

/// The base called `id`.
pub fn find(id: &str) -> Option<&'static Base> {
    BASES.iter().find(|b| b.id == id)
}

/// The folder of `style`, which is what a picture is made for when nobody picks: the folder the
/// app puts skins on.
pub fn folder_of(style: Style) -> &'static Base {
    BASES
        .iter()
        .find(|b| b.family == Family::Folder && b.template == Some(style))
        .unwrap_or(&MAC_FOLDER)
}

/// The base a saved skin names, or FolderSkin's own folder for one that names none (every skin
/// made before there were other shapes) or one this build doesn't know.
pub fn of_skin(id: Option<&str>) -> &'static Base {
    id.and_then(find).unwrap_or(&MAC_FOLDER)
}

impl Base {
    /// No template: the picture is the icon, as it is drawn.
    pub fn is_free(&self) -> bool {
        self.template.is_none()
    }

    /// The size artwork is best made in for it, so wrapping it on crops as little as it can
    /// ([`Style::artwork_size`]). A free icon is square.
    pub fn artwork_size(&self) -> (u32, u32) {
        self.template
            .map_or((SKIN_WIDTH, SKIN_WIDTH), Style::artwork_size)
    }

    /// The share of the artwork's height, from its top, that only shows above the front panel
    /// ([`Style::tab_share`]); nothing is hidden on a free icon.
    pub fn tab_share(&self) -> f32 {
        self.template.map_or(0.0, Style::tab_share)
    }

    /// The base as its system draws it, with nothing on it, `size` px square: what a picker shows.
    /// `None` for a free icon, which has no base to show.
    pub fn bare(&self, size: u32) -> Option<RgbaImage> {
        let style = self.template?;
        let art = compositor::default_folder_artwork_in(style);
        Some(
            compositor::render_icon_set_in(&art, &[size], style)
                .sizes
                .remove(0)
                .1,
        )
    }

    /// The base painted a flat light grey and centred on an opaque `backdrop`, `width` x
    /// `height` px: what an image model that can work from a picture is asked to repaint
    /// ([`compositor::blank_template_in`]).
    pub fn blank(&self, width: u32, height: u32, backdrop: [u8; 3]) -> Option<RgbaImage> {
        self.template
            .map(|style| compositor::blank_template_in(style, width, height, backdrop))
    }

    /// [`Base::blank`] on transparency: its alpha is the base's exact silhouette in that frame,
    /// which a whole painting is cut out along.
    pub fn blank_cutout(&self, width: u32, height: u32) -> Option<RgbaImage> {
        self.template
            .map(|style| compositor::blank_template_cutout_in(style, width, height))
    }

    /// How the compositor finishes artwork onto it: wrapped onto its template at every size.
    /// `None` for a free icon, whose picture is used as it is
    /// ([`compositor::icon_set_from_image`]).
    pub fn wrap(&self, art: &Artwork, sizes: &[u32]) -> Option<IconSet> {
        self.template
            .map(|style| compositor::render_icon_set_in(art, sizes, style))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_base_has_its_own_id_and_is_found_by_it() {
        let mut ids: Vec<&str> = BASES.iter().map(|b| b.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), BASES.len());
        for b in BASES {
            assert_eq!(find(b.id), Some(b));
            assert!(!b.label.is_empty());
            // Ids are what a saved skin and the webview keep: plain, lower case, no spaces.
            assert!(
                b.id.bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-'),
                "{}",
                b.id
            );
        }
        assert_eq!(find("nope"), None);
    }

    #[test]
    fn the_folder_skins_go_on_is_the_default() {
        assert_eq!(folder_of(Style::Mac), &MAC_FOLDER);
        assert_eq!(folder_of(Style::Windows), &WINDOWS_FOLDER);
        // A skin from before shapes, or from a build with shapes this one doesn't know.
        assert_eq!(of_skin(None), &MAC_FOLDER);
        assert_eq!(of_skin(Some("some-drive")), &MAC_FOLDER);
        assert_eq!(of_skin(Some("free")), &FREE);
    }

    #[test]
    fn folders_have_a_template_and_a_free_icon_has_none() {
        for b in [MAC_FOLDER, WINDOWS_FOLDER] {
            assert!(!b.is_free());
            let bare = b.bare(64).unwrap();
            assert_eq!(bare.dimensions(), (64, 64));
            assert_eq!(bare.get_pixel(0, 0).0[3], 0, "{}: transparent around", b.id);
            assert_eq!(bare.get_pixel(32, 44).0[3], 255, "{}: opaque inside", b.id);
            let blank = b.blank(256, 240, [255, 0, 255]).unwrap();
            let cut = b.blank_cutout(256, 240).unwrap();
            assert_eq!(crate::matte::flatten(&cut, [255, 0, 255]), blank);
            let art = compositor::default_folder_artwork();
            assert_eq!(
                b.wrap(&art, &[32]).unwrap().sizes[0].1.dimensions(),
                (32, 32)
            );
        }
        assert!(FREE.is_free());
        assert!(FREE.bare(64).is_none() && FREE.blank(64, 64, [0, 0, 0]).is_none());
        assert!(FREE.blank_cutout(64, 64).is_none());
        assert!(FREE
            .wrap(&compositor::default_folder_artwork(), &[32])
            .is_none());
        assert_eq!(FREE.artwork_size(), (1024, 1024));
        assert_eq!(FREE.tab_share(), 0.0);
    }

    #[test]
    fn every_base_with_a_template_says_how_it_is_built_in_words_a_model_can_paint() {
        for b in BASES {
            let Some(a) = b.anatomy else {
                assert!(b.is_free(), "{} has a template and no anatomy", b.id);
                continue;
            };
            assert!(!a.parts.is_empty(), "{}", b.id);
            for words in a
                .parts
                .iter()
                .chain([&a.noun, &a.kind, &a.keeps, &a.surface, &a.middle])
            {
                assert!(!words.trim().is_empty(), "{}", b.id);
                // The local model has no negative prompt: naming a thing paints it.
                for no in [" no ", "not ", "without", "don't"] {
                    assert!(!words.contains(no), "{}: {words:?} says {no:?}", b.id);
                }
            }
            assert!(
                a.unpainted.is_empty() || a.unpainted.ends_with('.'),
                "{}: a sentence",
                b.id
            );
        }
        assert_eq!(MAC_FOLDER.anatomy.unwrap().parts.len(), 3);
        assert_eq!(WINDOWS_FOLDER.anatomy.unwrap().parts.len(), 2, "no paper");
    }

    #[test]
    fn each_folder_keeps_its_own_artwork_shape_and_bare_picture() {
        assert_eq!(MAC_FOLDER.artwork_size(), (1024, 958));
        assert_eq!(WINDOWS_FOLDER.artwork_size(), (1024, 805));
        assert!(WINDOWS_FOLDER.tab_share() > MAC_FOLDER.tab_share());
        assert_ne!(MAC_FOLDER.bare(64), WINDOWS_FOLDER.bare(64));
        assert_eq!(
            (MAC_FOLDER.system.id(), MAC_FOLDER.family.id()),
            ("mac", "folder")
        );
        assert_eq!(
            (FREE.system.id(), FREE.family.id(), Family::Drive.id()),
            ("any", "free", "drive")
        );
        assert_eq!(System::Linux.id(), "linux");
    }
}
