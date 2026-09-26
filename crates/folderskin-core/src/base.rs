//! The shapes a skin is made for, which the app calls shapes: a folder in each system's look,
//! every drive shape each system shows ([`crate::drive`]), and a free icon with no base at all.
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
//! The AI view paints on them, the composer starts designs on them, and a drive picked on the
//! stage is one of them, so a shape added here shows up in all three.
//!
//! Each base with a template also says, in words, how it's built ([`Anatomy`]). Every prompt that
//! paints a base, for a provider or for the local model, is made from those words, so a base added
//! here is described to a model without a sentence written for it anywhere else.

use crate::compositor::{self, Artwork, IconSet, Style, SKIN_WIDTH};
use crate::drive::{DriveKind, DriveShape, DriveStyle};
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

/// What the compositor draws a base's skins on: a folder in one of its looks, or a drive shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Template {
    Folder(Style),
    Drive(DriveShape),
}

impl Template {
    /// The size artwork is best made in for it, so wrapping it on crops as little as it can.
    pub fn artwork_size(self) -> (u32, u32) {
        match self {
            Template::Folder(style) => style.artwork_size(),
            Template::Drive(shape) => shape.artwork_size(),
        }
    }

    /// The share of the artwork's height, from its top, that only shows above a folder's front
    /// panel. A drive has no tab: its face shows all of it.
    pub fn tab_share(self) -> f32 {
        match self {
            Template::Folder(style) => style.tab_share(),
            Template::Drive(_) => 0.0,
        }
    }

    /// Its width over its height, from its own outlines: the shape of a frame it fills.
    pub fn aspect(self) -> f32 {
        match self {
            Template::Folder(style) => style.aspect(),
            Template::Drive(shape) => shape.aspect(),
        }
    }

    /// Bare, as its system draws it, `size` px square.
    fn bare(self, size: u32) -> RgbaImage {
        let set = match self {
            Template::Folder(style) => compositor::render_icon_set_in(
                &compositor::default_folder_artwork_in(style),
                &[size],
                style,
            ),
            Template::Drive(shape) => compositor::render_drive_icon_set(None, &[size], shape),
        };
        set.sizes.into_iter().next().expect("the size asked for").1
    }

    fn blank(self, width: u32, height: u32, backdrop: [u8; 3]) -> RgbaImage {
        match self {
            Template::Folder(style) => {
                compositor::blank_template_in(style, width, height, backdrop)
            }
            Template::Drive(shape) => compositor::blank_drive_in(shape, width, height, backdrop),
        }
    }

    fn blank_cutout(self, width: u32, height: u32) -> RgbaImage {
        match self {
            Template::Folder(style) => compositor::blank_template_cutout_in(style, width, height),
            Template::Drive(shape) => compositor::blank_drive_cutout_in(shape, width, height),
        }
    }

    fn wrap(self, art: &Artwork, sizes: &[u32]) -> IconSet {
        match self {
            Template::Folder(style) => compositor::render_icon_set_in(art, sizes, style),
            Template::Drive(shape) => compositor::render_drive_icon_set(Some(art), sizes, shape),
        }
    }
}

/// How a base is built, in the words an image model is given: what it is, its parts, what a
/// repaint of its template keeps and where artwork goes on it. It is written from the base's
/// geometry (`geometry.rs`, `geometry_windows.rs`, `geometry_linux.rs` and the drives' drawings),
/// part by part, and every prompt that paints the base is made from it (folderskin_ai's recipe).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Anatomy {
    /// What it is, in a word: "folder".
    pub noun: &'static str,
    /// What it is as its system draws it: "macOS-style folder".
    pub kind: &'static str,
    /// Its parts from the back to the front, each as a model that draws it from words alone is
    /// told to build it.
    pub parts: &'static [&'static str],
    /// What a repaint of its template keeps, besides its outline, size and position: "its single
    /// tab at the top left, …".
    pub keeps: &'static str,
    /// What artwork is wrapped across.
    pub surface: &'static str,
    /// Where the main subject sits.
    pub middle: &'static str,
    /// How a part the painting doesn't cover stays, as a model that reads prose is told it; empty
    /// when every part is painted.
    pub paper: &'static str,
    /// The same, as a model that follows instructions is told it.
    pub paper_keep: &'static str,
    /// A corner of the artwork its tab hides beyond the band along the top ("upper-left"), which
    /// stays calm like the band; empty when the band hides it all.
    pub corner: &'static str,
}

/// One shape a skin can be made for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Base {
    /// How the webview and a saved skin name it: "mac-folder", "linux-removable". A drive's is
    /// its shape's id ([`DriveShape::id`]).
    pub id: &'static str,
    /// Its name in English, as the composer names the same shape ("Mac folder"). The app shows
    /// it in the language on show, by `id`.
    pub label: &'static str,
    pub system: System,
    pub family: Family,
    /// What the compositor draws it with, or `None` for a free icon, which is used as it is drawn.
    pub template: Option<Template>,
    /// How it is built, for the prompts that paint it; `None` for a free icon.
    pub anatomy: Option<Anatomy>,
}

/// FolderSkin's own folder, as Finder shows it.
pub const MAC_FOLDER: Base = Base {
    id: "mac-folder",
    label: "Mac folder",
    system: System::Mac,
    family: Family::Folder,
    template: Some(Template::Folder(Style::Mac)),
    anatomy: Some(Anatomy {
        noun: "folder",
        kind: "macOS-style folder",
        parts: &[
            "a back panel with a single rounded tab at its top left, the only tab in the picture",
            "one plain paper sheet tucked inside, with just its top edge showing in the gap \
             between the panels",
            "a front panel covering the lower two thirds, nearest the viewer",
        ],
        keeps: "its single tab at the top left, the pale paper strip between the back and front \
                panels",
        surface: "the back panel, the tab and the front panel",
        middle: "the front panel",
        paper: "The thin paper strip between the panels stays pale cream.",
        paper_keep: "Keep the paper strip plain or faintly tinted.",
        corner: "",
    }),
};

/// The folder Windows 11 draws: two panels and no paper, the front's top edge dipping under the
/// tab (`geometry_windows.rs`).
pub const WINDOWS_FOLDER: Base = Base {
    id: "windows-folder",
    label: "Windows folder",
    system: System::Windows,
    family: Family::Folder,
    template: Some(Template::Folder(Style::Windows)),
    anatomy: Some(Anatomy {
        noun: "folder",
        kind: "Windows-style folder",
        parts: &[
            "a back panel with a single tab at its top left, the only tab in the picture, its top \
             edge stepping down in a gentle curve from the tab to the rest of the panel",
            "a front panel covering most of the back panel, nearest the viewer, whose top edge \
             dips lower under the tab and rises in a gentle curve to run straight across the rest",
        ],
        keeps: "its tab at the top left and the curved step where the front panel rises to meet it",
        surface: "the back panel, the tab and the front panel",
        middle: "the front panel",
        // Windows' folder has no paper sheet.
        paper: "",
        paper_keep: "",
        corner: "upper-left",
    }),
};

/// The folder GNOME and KDE draw, in FolderSkin's own drawing: two panels and no paper, the tab
/// sloping down to the back panel's body (`geometry_linux.rs`).
pub const LINUX_FOLDER: Base = Base {
    id: "linux-folder",
    label: "Linux folder",
    system: System::Linux,
    family: Family::Folder,
    template: Some(Template::Folder(Style::Linux)),
    anatomy: Some(Anatomy {
        noun: "folder",
        kind: "Linux-style folder",
        parts: &[
            "a back panel with a single tab at its top left, the only tab in the picture, its top \
             edge sloping down from the tab to the rest of the panel",
            "a front panel covering most of the back panel, nearest the viewer, its top edge \
             straight across",
        ],
        keeps: "its tab at the top left and the slope from the tab down to the rest of the back \
                panel",
        surface: "the back panel, the tab and the front panel",
        middle: "the front panel",
        // Like Windows', it has no paper sheet.
        paper: "",
        paper_keep: "",
        corner: "upper-left",
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

/// A drive of `kind` as `style` draws it, named `id` (its shape's) and `label`.
const fn drive(
    id: &'static str,
    label: &'static str,
    style: DriveStyle,
    kind: DriveKind,
    anatomy: Anatomy,
) -> Base {
    Base {
        id,
        label,
        system: match style {
            DriveStyle::Mac => System::Mac,
            DriveStyle::Windows => System::Windows,
            DriveStyle::Linux => System::Linux,
        },
        family: Family::Drive,
        template: Some(Template::Drive(DriveShape::listed(style, kind))),
        anatomy: Some(anatomy),
    }
}

/// Words for a shape whose painting covers every part but its fittings: no paper, no tab.
const fn built(
    noun: &'static str,
    kind: &'static str,
    parts: &'static [&'static str],
    keeps: &'static str,
    surface: &'static str,
) -> Anatomy {
    Anatomy {
        noun,
        kind,
        parts,
        keeps,
        surface,
        middle: surface,
        paper: "",
        paper_keep: "",
        corner: "",
    }
}

/// Finder's disk (`drive/mac.rs`): an upright box on a strip, the kinds told apart by colour and a
/// mark on the front, which a painting covers.
const fn mac_disk(kind: &'static str) -> Anatomy {
    built(
        "drive",
        kind,
        &[
            "an upright box with rounded top corners and gently shaded sides, its whole front one \
             flat panel",
            "a narrow strip along its foot, joined to the box, with a small round light near its \
             right end",
        ],
        "the narrow strip along its foot with its small light, and the rounded top corners",
        "the front panel",
    )
}

/// A disc (`drive/mac.rs`, `drive/linux.rs`), printable between its clear hub and its rim.
const fn disc(kind: &'static str) -> Anatomy {
    Anatomy {
        noun: "disc",
        kind,
        parts: &[
            "a round silver disc lying flat to the viewer, with a small hole in its middle",
            "a clear hub ring around the hole, and a thin clear rim around its edge",
        ],
        keeps: "the hole in the middle, the clear hub ring around it and the thin rim at its edge",
        surface: "the printable ring between the clear hub and the rim",
        middle: "the upper half of that ring",
        paper: "",
        paper_keep: "",
        corner: "",
    }
}

/// Explorer's drive (`drive/windows.rs`): a slab seen from a little above, its top the face.
const WINDOWS_SLAB: &[&str] = &[
    "a wide flat drive seen from a little above, its top a light panel that narrows slightly \
     towards the back",
    "a grey front edge below the top, with a small blue light near its right end",
];
const WINDOWS_SLAB_KEEPS: &str =
    "the grey front edge with its blue light, and the top narrowing slightly towards the back";

/// A hard disk as GNOME and KDE draw one (`drive/linux.rs`).
const LINUX_DISK: &[&str] = &[
    "a wide box with a light front panel and a small screw in each of its four corners",
    "a dark band along its foot with a row of vents on the left and a blue light on the right",
];
const LINUX_DISK_KEEPS: &str =
    "the dark band along its foot with its vents and light, and the four screws";

/// The version of the blank templates a model is shown to repaint. Bumped when one is drawn
/// differently, so a skin's record says which one its model saw.
pub const TEMPLATE_VERSION: u32 = 1;

/// Every base, in the order a picker lists them: the folders, the drives system by system in the
/// order the composer lists them ([`DriveStyle::kinds`]), then the free icon.
pub const BASES: &[Base] = &[
    MAC_FOLDER,
    WINDOWS_FOLDER,
    LINUX_FOLDER,
    drive(
        "mac-startup",
        "Mac startup disk",
        DriveStyle::Mac,
        DriveKind::Startup,
        mac_disk("macOS-style startup disk"),
    ),
    drive(
        "mac-internal",
        "Mac internal drive",
        DriveStyle::Mac,
        DriveKind::Internal,
        mac_disk("macOS-style internal drive"),
    ),
    drive(
        "mac-external",
        "Mac external drive",
        DriveStyle::Mac,
        DriveKind::External,
        mac_disk("macOS-style external drive"),
    ),
    drive(
        "mac-removable",
        "Mac USB drive",
        DriveStyle::Mac,
        DriveKind::Removable,
        mac_disk("macOS-style USB drive"),
    ),
    drive(
        "mac-card",
        "Mac memory card",
        DriveStyle::Mac,
        DriveKind::Card,
        built(
            "memory card",
            "macOS-style memory card",
            &[
                "a flat dark memory card standing upright, its top right corner cut off at an \
                 angle",
                "a light label covering most of its front",
            ],
            "the corner cut off at the top right, and the dark edge of the card around the label",
            "the label",
        ),
    ),
    drive(
        "mac-optical",
        "Mac disc",
        DriveStyle::Mac,
        DriveKind::Optical,
        disc("macOS-style disc"),
    ),
    drive(
        "mac-disk-image",
        "Mac disk image",
        DriveStyle::Mac,
        DriveKind::DiskImage,
        mac_disk("macOS-style disk image"),
    ),
    drive(
        "mac-network",
        "Mac network drive",
        DriveStyle::Mac,
        DriveKind::Network,
        mac_disk("macOS-style network drive"),
    ),
    drive(
        "mac-time-machine",
        "Mac Time Machine disk",
        DriveStyle::Mac,
        DriveKind::TimeMachine,
        mac_disk("macOS-style Time Machine disk"),
    ),
    drive(
        "windows-startup",
        "Windows system drive",
        DriveStyle::Windows,
        DriveKind::Startup,
        built(
            "drive",
            "Windows-style system drive",
            WINDOWS_SLAB,
            WINDOWS_SLAB_KEEPS,
            "the top panel",
        ),
    ),
    drive(
        "windows-internal",
        "Windows local disk",
        DriveStyle::Windows,
        DriveKind::Internal,
        built(
            "drive",
            "Windows-style local disk",
            WINDOWS_SLAB,
            WINDOWS_SLAB_KEEPS,
            "the top panel",
        ),
    ),
    drive(
        "windows-removable",
        "Windows USB drive",
        DriveStyle::Windows,
        DriveKind::Removable,
        built(
            "drive",
            "Windows-style USB drive",
            &[
                "a metal USB plug sticking out of its right side, behind the drive",
                "a wide flat drive seen from a little above, its top a light panel that narrows \
                 slightly towards the back",
                "a grey front edge below the top, with a small blue light near its right end",
            ],
            "the USB plug at its right side, the grey front edge with its blue light, and the top \
             narrowing slightly towards the back",
            "the top panel",
        ),
    ),
    drive(
        "windows-card",
        "Windows SD card",
        DriveStyle::Windows,
        DriveKind::Card,
        built(
            "memory card",
            "Windows-style SD card",
            &[
                "a flat SD card standing upright, its top right corner cut off at an angle, with \
                 ridges along its top",
                "a light label covering most of its front",
            ],
            "the corner cut off at the top right, the ridges along its top and the card's edge \
             around the label",
            "the label",
        ),
    ),
    drive(
        "windows-optical",
        "Windows disc drive",
        DriveStyle::Windows,
        DriveKind::Optical,
        Anatomy {
            noun: "disc drive",
            kind: "Windows-style disc drive with a disc standing in it",
            parts: &[
                "a round silver disc standing upright, with a small hole in its middle",
                "a wide flat drive in front of the disc's lower half, seen from a little above",
            ],
            keeps: "the drive in front of the disc's lower half, and the disc's hole and rim",
            surface: "the disc's printable ring",
            middle: "the part of the disc above the drive",
            paper: "",
            paper_keep: "",
            corner: "",
        },
    ),
    drive(
        "windows-network",
        "Windows network drive",
        DriveStyle::Windows,
        DriveKind::Network,
        built(
            "drive",
            "Windows-style network drive",
            &[
                "a short stem under the drive, down to a green bar that runs across below it",
                "a wide flat drive seen from a little above, its top a light panel that narrows \
                 slightly towards the back",
                "a grey front edge below the top, with a small blue light near its right end",
            ],
            "the green bar on its stem below, the grey front edge with its blue light, and the top \
             narrowing slightly towards the back",
            "the top panel",
        ),
    ),
    drive(
        "linux-internal",
        "Linux hard disk",
        DriveStyle::Linux,
        DriveKind::Internal,
        built(
            "drive",
            "Linux-style hard disk",
            LINUX_DISK,
            LINUX_DISK_KEEPS,
            "the front panel",
        ),
    ),
    drive(
        "linux-solid-state",
        "Linux solid-state drive",
        DriveStyle::Linux,
        DriveKind::SolidState,
        built(
            "drive",
            "Linux-style solid-state drive card",
            &[
                "a long dark circuit board seen from above, with a half-round notch in its left end",
                "a row of gold contacts along its right end, parted by a narrow slot",
                "a light label covering most of the board",
            ],
            "the gold contacts at the right end, the notch at the left end, and the dark board \
             around the label",
            "the label",
        ),
    ),
    drive(
        "linux-external",
        "Linux USB hard disk",
        DriveStyle::Linux,
        DriveKind::External,
        built(
            "drive",
            "Linux-style USB hard disk",
            &[
                "a cable from the disk's right side curving down to a USB plug below it",
                "a wide box with a light front panel and a small screw in each of its four corners",
                "a dark band along its foot with a row of vents on the left and a blue light on \
                 the right",
            ],
            "the cable down to its plug, the dark band along its foot with its vents and light, \
             and the four screws",
            "the front panel",
        ),
    ),
    drive(
        "linux-removable",
        "Linux USB stick",
        DriveStyle::Linux,
        DriveKind::Removable,
        built(
            "USB stick",
            "Linux-style USB stick",
            &[
                "a metal USB plug at its top, with two square holes in it",
                "a blue plastic body below the plug, round at its bottom end with a ring hole \
                 through it",
                "a light label on the front of the body",
            ],
            "the metal plug at the top, the round end with its ring hole, and the blue body \
             around the label",
            "the label",
        ),
    ),
    drive(
        "linux-card",
        "Linux memory card",
        DriveStyle::Linux,
        DriveKind::Card,
        built(
            "memory card",
            "Linux-style memory card",
            &[
                "a flat dark memory card standing upright, its top right corner cut off, with \
                 ridges along its top",
                "a light label with a blue strip across its top",
            ],
            "the corner cut off at the top right, the ridges, and the blue strip across the top \
             of the label",
            "the label",
        ),
    ),
    drive(
        "linux-optical-drive",
        "Linux disc drive",
        DriveStyle::Linux,
        DriveKind::OpticalDrive,
        built(
            "disc drive",
            "Linux-style disc drive",
            &[
                "a wide box with a light top panel",
                "a dark front band with the tray's long slot and an eject button on its right",
            ],
            "the dark front band with its slot and eject button",
            "the top panel",
        ),
    ),
    drive(
        "linux-optical",
        "Linux disc",
        DriveStyle::Linux,
        DriveKind::Optical,
        disc("Linux-style disc"),
    ),
    drive(
        "linux-server",
        "Linux network server",
        DriveStyle::Linux,
        DriveKind::Server,
        built(
            "server",
            "Linux-style rack server",
            &[
                "three units stacked in a rack, each held by a small ear at either side",
                "the top unit tall, its front a light panel over a dark band of vents and two \
                 lights",
                "two thin dark units below it, each with vents and two lights",
            ],
            "the two thin units below, the ears at the sides, and the dark band with its lights",
            "the top unit's front panel",
        ),
    ),
    drive(
        "linux-network",
        "Linux network folder",
        DriveStyle::Linux,
        DriveKind::Network,
        built(
            "folder",
            "Linux-style network folder",
            &[
                "a back panel with a single tab at its top left, the only tab in the picture",
                "a lighter front panel covering most of the back panel, nearest the viewer",
                "a round white badge on the front panel's lower right corner, with three joined \
                 dots on it",
            ],
            "the tab at the top left, and the round badge with its three joined dots",
            "the front panel",
        ),
    ),
    drive(
        "linux-multi-disk",
        "Linux RAID set",
        DriveStyle::Linux,
        DriveKind::MultiDisk,
        built(
            "RAID set",
            "Linux-style RAID set of two hard disks",
            &[
                "a hard disk at the back, raised and to the right",
                "a second hard disk in front of it, lower and to the left",
                "on each disk, a light front with a screw in each corner over a dark band with \
                 vents and a light",
            ],
            "the disk at the back, and both disks' dark bands with their vents and lights",
            "the front disk's front panel",
        ),
    ),
    FREE,
];

/// The base called `id`.
pub fn find(id: &str) -> Option<&'static Base> {
    BASES.iter().find(|b| b.id == id)
}

/// The folder of `style`, which is what a picture is made for when nobody picks: the folder the
/// app puts skins on.
pub fn folder_of(style: Style) -> &'static Base {
    BASES
        .iter()
        .find(|b| b.template == Some(Template::Folder(style)))
        .unwrap_or(&MAC_FOLDER)
}

/// The base of the drive shape `shape`: every shape FolderSkin draws is one.
pub fn drive_of(shape: DriveShape) -> &'static Base {
    BASES
        .iter()
        .find(|b| b.template == Some(Template::Drive(shape)))
        .expect("every drive shape is a base")
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

    /// The drive shape it is, for a drive.
    pub fn drive(&self) -> Option<DriveShape> {
        match self.template {
            Some(Template::Drive(shape)) => Some(shape),
            _ => None,
        }
    }

    /// The size artwork is best made in for it, so wrapping it on crops as little as it can
    /// ([`Template::artwork_size`]). A free icon is square.
    pub fn artwork_size(&self) -> (u32, u32) {
        self.template
            .map_or((SKIN_WIDTH, SKIN_WIDTH), Template::artwork_size)
    }

    /// The share of the artwork's height, from its top, that only shows above the front panel
    /// ([`Template::tab_share`]); nothing is hidden on a free icon or a drive.
    pub fn tab_share(&self) -> f32 {
        self.template.map_or(0.0, Template::tab_share)
    }

    /// The base as its system draws it, with nothing on it, `size` px square: what a picker shows.
    /// `None` for a free icon, which has no base to show.
    pub fn bare(&self, size: u32) -> Option<RgbaImage> {
        self.template.map(|t| t.bare(size.max(1)))
    }

    /// The base painted a flat light grey and centred on an opaque `backdrop`, `width` x
    /// `height` px: what an image model that can work from a picture is asked to repaint
    /// ([`compositor::blank_template_in`], [`compositor::blank_drive_in`]).
    pub fn blank(&self, width: u32, height: u32, backdrop: [u8; 3]) -> Option<RgbaImage> {
        self.template.map(|t| t.blank(width, height, backdrop))
    }

    /// [`Base::blank`] on transparency: its alpha is the base's exact silhouette in that frame,
    /// which a whole painting is cut out along.
    pub fn blank_cutout(&self, width: u32, height: u32) -> Option<RgbaImage> {
        self.template.map(|t| t.blank_cutout(width, height))
    }

    /// How the compositor finishes artwork onto it: wrapped onto its template at every size.
    /// `None` for a free icon, whose picture is used as it is
    /// ([`compositor::icon_set_from_image`]).
    pub fn wrap(&self, art: &Artwork, sizes: &[u32]) -> Option<IconSet> {
        self.template.map(|t| t.wrap(art, sizes))
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
        assert_eq!(folder_of(Style::Linux), &LINUX_FOLDER);
        // A skin from before shapes, or from a build with shapes this one doesn't know.
        assert_eq!(of_skin(None), &MAC_FOLDER);
        assert_eq!(of_skin(Some("some-drive")), &MAC_FOLDER);
        assert_eq!(of_skin(Some("free")), &FREE);
        assert_eq!(of_skin(Some("linux-removable")).family, Family::Drive);
    }

    #[test]
    fn folders_have_a_template_and_a_free_icon_has_none() {
        for b in [MAC_FOLDER, WINDOWS_FOLDER, LINUX_FOLDER] {
            assert!(!b.is_free());
            assert_eq!(b.drive(), None);
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
            let optional = [&a.paper, &a.paper_keep, &a.corner];
            for words in a
                .parts
                .iter()
                .chain([&a.noun, &a.kind, &a.keeps, &a.surface, &a.middle])
                .chain(optional.into_iter().filter(|w| !w.is_empty()))
            {
                assert!(!words.trim().is_empty(), "{}", b.id);
                // The local model has no negative prompt: naming a thing paints it.
                for no in [" no ", "not ", "without", "don't"] {
                    assert!(!words.contains(no), "{}: {words:?} says {no:?}", b.id);
                }
            }
            for sentence in [a.paper, a.paper_keep] {
                assert!(
                    sentence.is_empty() || sentence.ends_with('.'),
                    "{}: a sentence",
                    b.id
                );
            }
            assert_eq!(a.paper.is_empty(), a.paper_keep.is_empty(), "{}", b.id);
        }
        assert_eq!(MAC_FOLDER.anatomy.unwrap().parts.len(), 3);
        assert_eq!(WINDOWS_FOLDER.anatomy.unwrap().parts.len(), 2, "no paper");
        assert_eq!(LINUX_FOLDER.anatomy.unwrap().parts.len(), 2, "no paper");
    }

    #[test]
    fn each_folder_keeps_its_own_artwork_shape_and_bare_picture() {
        assert_eq!(MAC_FOLDER.artwork_size(), (1024, 958));
        assert_eq!(WINDOWS_FOLDER.artwork_size(), (1024, 805));
        assert_eq!(LINUX_FOLDER.artwork_size(), (1024, 852));
        assert!(WINDOWS_FOLDER.tab_share() > MAC_FOLDER.tab_share());
        assert_ne!(MAC_FOLDER.bare(64), WINDOWS_FOLDER.bare(64));
        assert_ne!(WINDOWS_FOLDER.bare(64), LINUX_FOLDER.bare(64));
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

    /// Every drive shape FolderSkin draws is a base, named by the shape's own id, in its system,
    /// in the order the composer lists them, with nothing of the artwork hidden behind a tab.
    #[test]
    fn every_drive_shape_is_a_base_of_its_own() {
        let drives: Vec<&Base> = BASES.iter().filter(|b| b.family == Family::Drive).collect();
        let shapes = DriveShape::all();
        assert_eq!(drives.len(), shapes.len());
        for (b, shape) in drives.into_iter().zip(shapes) {
            assert_eq!(b.drive(), Some(shape), "{}", b.id);
            assert_eq!(b.id, shape.id());
            assert_eq!(DriveShape::new(shape.style(), shape.kind()), Some(shape));
            assert_eq!(b.system.id(), shape.style().id(), "{}", b.id);
            assert_eq!(drive_of(shape), b);
            assert_eq!(b.tab_share(), 0.0, "{}", b.id);
            // Artwork in the face's own shape, its longer side the folder's width.
            let (w, h) = b.artwork_size();
            let face = shape.face_box();
            assert_eq!(w.max(h), SKIN_WIDTH, "{}", b.id);
            let ratio = (w as f32 / h as f32) / (face.width() / face.height());
            assert!((ratio - 1.0).abs() < 0.01, "{}: {w} x {h}", b.id);
            assert!(b.label.contains(match b.system {
                System::Mac => "Mac",
                System::Windows => "Windows",
                _ => "Linux",
            }));
        }
        let stick = find("linux-removable").unwrap();
        let (w, h) = stick.artwork_size();
        assert!(
            h == SKIN_WIDTH && w * 3 < h * 2,
            "a USB stick's label is tall: {w} x {h}"
        );
    }

    /// A drive's template is the drive with its face in the template's grey and the rest of it as
    /// drawn, centred in the frame, its alpha the drive's silhouette.
    #[test]
    fn a_drive_is_repainted_from_its_own_blank_template() {
        let stick = find("linux-removable").unwrap();
        let (w, h) = (240, 480);
        let cut = stick.blank_cutout(w, h).unwrap();
        assert_eq!(cut.dimensions(), (w, h));
        assert_eq!(
            crate::matte::flatten(&cut, [0, 255, 0]),
            stick.blank(w, h, [0, 255, 0]).unwrap()
        );
        assert_eq!(cut.get_pixel(2, 2).0[3], 0, "the margin is clear");
        // The label is the template's grey, the body the stick's own blue: the middle of the
        // frame is the middle of the label, and 100 units right and 328 down, the round end.
        let label = cut.get_pixel(w / 2, h / 2).0;
        assert_eq!(&label[..3], &[0xCC, 0xCC, 0xCC]);
        let body = cut.get_pixel(173, 415).0;
        assert!(
            body[2] > body[0] + 40 && body[3] == 255,
            "blue plastic: {body:?}"
        );
        // A picker shows each drive bare, and wraps artwork onto its face.
        let bare = stick.bare(64).unwrap();
        assert_eq!(bare.get_pixel(32, 40).0[3], 255);
        assert_eq!(bare.get_pixel(4, 40).0[3], 0, "a stick is narrow");
        let art = compositor::default_folder_artwork();
        assert_eq!(
            stick.wrap(&art, &[48]).unwrap().sizes[0].1.dimensions(),
            (48, 48)
        );
        assert!((stick.template.unwrap().aspect() - 312.0 / 844.0).abs() < 0.02);
    }

    /// Every base plain and with a picture on it, at 256 px, and at 32 and 16 px as they are and
    /// shown four times over, for looking at: `FOLDERSKIN_SHEET=<folder> cargo test -p
    /// folderskin-core --lib -- --ignored base_sheet`. Each base's bare shape is written at 1024 px
    /// beside the sheets.
    #[test]
    #[ignore = "writes pictures to look at"]
    fn base_sheet() {
        let Some(dir) = std::env::var_os("FOLDERSKIN_SHEET") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let art = Artwork {
            rgba: RgbaImage::from_fn(1024, 958, |x, y| {
                let (u, v) = (x as f32 / 1023.0, y as f32 / 957.0);
                let sky = [
                    (40.0 + 200.0 * v) as u8,
                    (90.0 + 120.0 * v) as u8,
                    (200.0 - 60.0 * v) as u8,
                    255,
                ];
                let sun = ((u - 0.62).powi(2) + (v - 0.42).powi(2)).sqrt() < 0.13;
                let hill = v > 0.72 - 0.12 * (u * 6.0).sin().abs();
                image::Rgba(if sun {
                    [255, 214, 90, 255]
                } else if hill {
                    [30, 90, 60, 255]
                } else {
                    sky
                })
            }),
            focus: (0.5, 0.5),
        };
        let bases: Vec<&Base> = BASES.iter().filter(|b| !b.is_free()).collect();
        let (cell, cols) = (256u32, 7u32);
        let rows = bases.len().div_ceil(cols as usize) as u32;
        for (name, with_art) in [("sheet.png", false), ("sheet-art.png", true)] {
            let mut sheet = RgbaImage::from_pixel(
                cols * cell,
                rows * (cell + 72),
                image::Rgba([236, 237, 240, 255]),
            );
            for (i, base) in bases.iter().enumerate() {
                let sizes = [1024, 256, 32, 16];
                let template = base.template.unwrap();
                let set = if with_art {
                    template.wrap(&art, &sizes)
                } else {
                    match template {
                        Template::Folder(style) => compositor::render_icon_set_in(
                            &compositor::default_folder_artwork_in(style),
                            &sizes,
                            style,
                        ),
                        Template::Drive(shape) => {
                            compositor::render_drive_icon_set(None, &sizes, shape)
                        }
                    }
                };
                if !with_art {
                    set.sizes[0]
                        .1
                        .save(dir.join(format!("{}.png", base.id)))
                        .unwrap();
                }
                let (x, y) = ((i as u32 % cols) * cell, (i as u32 / cols) * (cell + 72));
                let (x, y) = (i64::from(x), i64::from(y));
                image::imageops::overlay(&mut sheet, &set.sizes[1].1, x, y);
                let big = |img: &RgbaImage| {
                    image::imageops::resize(img, 64, 64, image::imageops::FilterType::Nearest)
                };
                image::imageops::overlay(&mut sheet, &set.sizes[2].1, x + 8, y + 276);
                image::imageops::overlay(&mut sheet, &set.sizes[3].1, x + 48, y + 284);
                image::imageops::overlay(&mut sheet, &big(&set.sizes[2].1), x + 100, y + 260);
                image::imageops::overlay(&mut sheet, &big(&set.sizes[3].1), x + 176, y + 260);
            }
            sheet.save(dir.join(name)).unwrap();
        }
    }
}
