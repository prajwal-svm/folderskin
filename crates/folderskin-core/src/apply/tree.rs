//! The folders inside a folder, for a skin that goes on a whole tree of them.
//!
//! Only the user's own folders count. Anything else is skipped, and never looked inside:
//! symlinks (and Windows junctions, which are the same thing to the standard library), names
//! starting with a dot, folders the OS hides, packages — apps, libraries and documents that are
//! folders on disk but a single item in Finder — and the system locations
//! [`validate_folder`](super::validate_folder) refuses. The folders come nearest first, so a
//! run that stops part way has done whole levels of the tree rather than one deep branch.

use super::paths::is_system_location;
use std::cmp::Ordering;
use std::ffi::{OsStr, OsString};
use std::fs::DirEntry;
use std::path::{Path, PathBuf};

/// The most folders one tree run changes, not counting the folder it starts from.
pub const MAX_TREE: usize = 5_000;

/// Extensions of the folders Finder shows as one item (apps, plug-ins, photo and music
/// libraries, projects, documents), compared without regard to case.
const PACKAGE_EXTENSIONS: &[&str] = &[
    "app",
    "appex",
    "bundle",
    "framework",
    "plugin",
    "kext",
    "xpc",
    "pkg",
    "mpkg",
    "photoslibrary",
    "photolibrary",
    "musiclibrary",
    "tvlibrary",
    "imovielibrary",
    "fcpbundle",
    "logicx",
    "band",
    "xcodeproj",
    "xcworkspace",
    "playground",
    "xcassets",
    "rtfd",
    "pages",
    "numbers",
    "key",
    "scptd",
    "mlmodelc",
    "docc",
    "dsym",
    "sparsebundle",
    "lpdf",
];

/// What [`subfolders`] found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Subfolders {
    /// Nearest first, and within one folder by name; at most the limit asked for.
    pub folders: Vec<PathBuf>,
    /// True when there were more than the limit, so `folders` is not all of them.
    pub more: bool,
}

/// Every folder inside `root`, not `root` itself, at most `limit` of them.
///
/// Breadth-first, so nearer folders come first, and the folders inside one folder are in name
/// order ignoring case (then exactly, so the order is always the same). Folders that aren't the
/// user's own are skipped along with everything inside them (see the module notes), and so is
/// everything inside `root` when it is a package or inside one. A folder that can't be read
/// counts, but has no folders inside it as far as this goes.
///
/// `root` should be canonical, as [`validate_folder`](super::validate_folder) returns it: the
/// system locations are recognised by their canonical paths.
pub fn subfolders(root: &Path, limit: usize) -> Subfolders {
    let mut folders: Vec<PathBuf> = Vec::new();
    if is_in_package(root) {
        return Subfolders::default();
    }
    // The folders found so far double as the queue: the next one to look inside is `next`.
    let mut next = 0;
    let mut children = visible_children(root);
    loop {
        for child in children {
            if folders.len() == limit {
                return Subfolders {
                    folders,
                    more: true,
                };
            }
            folders.push(child);
        }
        let Some(dir) = folders.get(next) else {
            break;
        };
        next += 1;
        children = visible_children(dir);
    }
    Subfolders {
        folders,
        more: false,
    }
}

/// True when a folder called `name` is a package by its extension, whatever its case.
pub fn is_package_name(name: &str) -> bool {
    match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => PACKAGE_EXTENSIONS
            .iter()
            .any(|known| known.eq_ignore_ascii_case(extension)),
        _ => false,
    }
}

/// The user's own folders directly inside `dir`, in name order. None when `dir` can't be read.
fn visible_children(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut children: Vec<(OsString, PathBuf)> = entries
        .flatten()
        .filter(is_users_folder)
        .map(|entry| (entry.file_name(), entry.path()))
        .collect();
    children.sort_by(|(a, _), (b, _)| by_name(a, b));
    children.into_iter().map(|(_, path)| path).collect()
}

/// True for an entry that is one of the user's own folders: a real folder (not a link to one),
/// not hidden either way, not a package and not the system's.
fn is_users_folder(entry: &DirEntry) -> bool {
    let Ok(kind) = entry.file_type() else {
        return false;
    };
    // `is_dir` is false for a symlink to a folder, and `is_symlink` covers junctions too.
    if kind.is_symlink() || !kind.is_dir() {
        return false;
    }
    let name = entry.file_name();
    let name = name.to_string_lossy();
    if name.starts_with('.') || is_package_name(&name) || is_hidden_by_os(entry) {
        return false;
    }
    let path = entry.path();
    !is_system_location(&path) && !is_package(&path)
}

/// Folder order within one folder: by name ignoring case, as a file manager lists them, then
/// exactly, so names that differ only in case still come in one order.
fn by_name(a: &OsStr, b: &OsStr) -> Ordering {
    let fold = |name: &OsStr| name.to_string_lossy().to_lowercase();
    fold(a).cmp(&fold(b)).then_with(|| a.cmp(b))
}

/// True when `root` is a package or inside one: the folders in there belong to the app that
/// made it, not to the user.
fn is_in_package(root: &Path) -> bool {
    root.components()
        .any(|part| is_package_name(&part.as_os_str().to_string_lossy()))
        || is_package(root)
}

/// macOS: the Finder's hidden flag (`chflags hidden`, `UF_HIDDEN`). An entry that can't be
/// looked at is as good as hidden; it is most likely gone.
#[cfg(target_os = "macos")]
fn is_hidden_by_os(entry: &DirEntry) -> bool {
    use std::os::macos::fs::MetadataExt;
    entry
        .metadata()
        .map_or(true, |meta| meta.st_flags() & libc::UF_HIDDEN != 0)
}

/// Windows: the hidden or system attribute, which Explorer hides by default.
#[cfg(windows)]
fn is_hidden_by_os(entry: &DirEntry) -> bool {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_SYSTEM};
    entry.metadata().map_or(true, |meta| {
        meta.file_attributes() & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0
    })
}

/// Elsewhere a leading dot is the only way to hide a folder.
#[cfg(not(any(target_os = "macos", windows)))]
fn is_hidden_by_os(_: &DirEntry) -> bool {
    false
}

/// macOS: whatever else Launch Services counts as a package, such as a folder with the bundle
/// bit set or an extension an installed app declared.
#[cfg(target_os = "macos")]
fn is_package(path: &Path) -> bool {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::NSString;
    objc2::rc::autoreleasepool(|_| {
        NSWorkspace::sharedWorkspace()
            .isFilePackageAtPath(&NSString::from_str(&path.to_string_lossy()))
    })
}

/// Elsewhere the extensions are all there is.
#[cfg(not(target_os = "macos"))]
fn is_package(_: &Path) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apply::{tempfile_dir, validate_folder};
    use std::fs;

    /// Makes each `/`-separated folder path under `root`, parents included.
    fn make(root: &Path, dirs: &[&str]) {
        for dir in dirs {
            fs::create_dir_all(root.join(dir)).unwrap();
        }
    }

    /// The folders found, as `/`-separated paths relative to `root`.
    fn relative(root: &Path, found: &Subfolders) -> Vec<String> {
        found
            .folders
            .iter()
            .map(|p| {
                p.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect()
    }

    #[test]
    fn nearer_folders_come_first_and_each_level_is_in_name_order() {
        let tmp = tempfile_dir();
        let root = validate_folder(&tmp).unwrap();
        make(&root, &["b/z", "b/A", "a/y/deep", "C"]);
        let found = subfolders(&root, MAX_TREE);
        assert_eq!(
            relative(&root, &found),
            ["a", "b", "C", "a/y", "b/A", "b/z", "a/y/deep"]
        );
        assert!(!found.more);
        assert!(
            found.folders.iter().all(|f| f.starts_with(&root)),
            "the paths are under the canonical root"
        );
    }

    #[test]
    fn names_are_ordered_ignoring_case_then_exactly() {
        let mut names: Vec<&OsStr> = ["b", "B", "a", "C", "ä", "10", "9"]
            .into_iter()
            .map(OsStr::new)
            .collect();
        names.sort_by(|a, b| by_name(a, b));
        let expected: Vec<&OsStr> = ["10", "9", "a", "B", "b", "C", "ä"]
            .into_iter()
            .map(OsStr::new)
            .collect();
        assert_eq!(names, expected);
    }

    #[test]
    fn dot_folders_and_packages_are_skipped_with_everything_inside_them() {
        let tmp = tempfile_dir();
        let root = validate_folder(&tmp).unwrap();
        make(
            &root,
            &[
                "Photos/2024",
                ".git/objects",
                "Thing.app/Contents",
                "Kit.FRAMEWORK/Versions",
                "Old.photoslibrary/originals",
                "Mine",
            ],
        );
        let found = subfolders(&root, MAX_TREE);
        assert_eq!(relative(&root, &found), ["Mine", "Photos", "Photos/2024"]);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_to_a_folder_is_not_followed() {
        let tmp = tempfile_dir();
        let root = validate_folder(&tmp).unwrap();
        let elsewhere = tempfile_dir();
        make(&elsewhere, &["Theirs/inside"]);
        make(&root, &["Mine"]);
        std::os::unix::fs::symlink(&*elsewhere, root.join("Link")).unwrap();
        std::os::unix::fs::symlink(root.join("Mine"), root.join("Mine too")).unwrap();
        assert_eq!(relative(&root, &subfolders(&root, MAX_TREE)), ["Mine"]);
    }

    #[cfg(windows)]
    #[test]
    fn a_symlink_to_a_folder_is_not_followed() {
        let tmp = tempfile_dir();
        let root = validate_folder(&tmp).unwrap();
        let elsewhere = tempfile_dir();
        make(&elsewhere, &["Theirs/inside"]);
        make(&root, &["Mine"]);
        // Creating a symlink needs Developer Mode or an elevated shell; without either there is
        // nothing to skip, and the rest of the test still holds.
        let _ = std::os::windows::fs::symlink_dir(&*elsewhere, root.join("Link"));
        assert_eq!(relative(&root, &subfolders(&root, MAX_TREE)), ["Mine"]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn folders_the_finder_hides_are_skipped() {
        use std::os::unix::ffi::OsStrExt;
        let tmp = tempfile_dir();
        let root = validate_folder(&tmp).unwrap();
        make(&root, &["Hidden/inside", "Shown"]);
        let hidden = std::ffi::CString::new(root.join("Hidden").as_os_str().as_bytes()).unwrap();
        // SAFETY: `hidden` is a NUL-terminated path.
        assert_eq!(
            unsafe { libc::chflags(hidden.as_ptr(), libc::UF_HIDDEN) },
            0
        );
        assert_eq!(relative(&root, &subfolders(&root, MAX_TREE)), ["Shown"]);
    }

    #[cfg(windows)]
    #[test]
    fn folders_windows_hides_are_skipped() {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            SetFileAttributesW, FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_SYSTEM,
        };
        let tmp = tempfile_dir();
        let root = validate_folder(&tmp).unwrap();
        make(&root, &["Hidden/inside", "System/inside", "Shown"]);
        for (name, attribute) in [
            ("Hidden", FILE_ATTRIBUTE_HIDDEN),
            ("System", FILE_ATTRIBUTE_SYSTEM),
        ] {
            let wide: Vec<u16> = root
                .join(name)
                .as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect();
            // SAFETY: `wide` is a NUL-terminated UTF-16 path that outlives the call.
            assert_ne!(unsafe { SetFileAttributesW(wide.as_ptr(), attribute) }, 0);
        }
        assert_eq!(relative(&root, &subfolders(&root, MAX_TREE)), ["Shown"]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_folder_launch_services_calls_a_package_is_skipped() {
        use std::os::unix::ffi::OsStrExt;
        let tmp = tempfile_dir();
        let root = validate_folder(&tmp).unwrap();
        make(&root, &["Bundle/Contents", "Plain"]);
        // The bundle bit (kHasBundle, 0x2000) in the Finder flags, which are the big-endian u16
        // at bytes 8 and 9 of the folder's Finder info: a package, whatever its name.
        let mut info = [0u8; 32];
        info[8] = 0x20;
        let path = std::ffi::CString::new(root.join("Bundle").as_os_str().as_bytes()).unwrap();
        // SAFETY: both names are NUL-terminated and `info` is readable for its whole length.
        let status = unsafe {
            libc::setxattr(
                path.as_ptr(),
                c"com.apple.FinderInfo".as_ptr(),
                info.as_ptr().cast(),
                info.len(),
                0,
                0,
            )
        };
        assert_eq!(status, 0, "setxattr failed");
        assert_eq!(relative(&root, &subfolders(&root, MAX_TREE)), ["Plain"]);
    }

    #[test]
    fn at_most_the_limit_and_more_says_whether_there_were_others() {
        let tmp = tempfile_dir();
        let root = validate_folder(&tmp).unwrap();
        make(&root, &["a/deep", "b", "c"]);

        let all = subfolders(&root, 4);
        assert_eq!(relative(&root, &all), ["a", "b", "c", "a/deep"]);
        assert!(!all.more, "exactly the limit is not more");

        let some = subfolders(&root, 3);
        assert_eq!(relative(&root, &some), ["a", "b", "c"]);
        assert!(some.more, "a/deep was left out");

        let none = subfolders(&root, 0);
        assert!(none.folders.is_empty() && none.more);

        assert_eq!(
            subfolders(&root.join("b"), 0),
            Subfolders::default(),
            "a folder with nothing inside is never more"
        );
    }

    #[test]
    fn files_never_count() {
        let tmp = tempfile_dir();
        let root = validate_folder(&tmp).unwrap();
        make(&root, &["Docs"]);
        fs::write(root.join("notes.txt"), b"x").unwrap();
        fs::write(root.join("Report"), b"x").unwrap();
        fs::write(root.join("Docs").join("Draft"), b"x").unwrap();
        assert_eq!(relative(&root, &subfolders(&root, MAX_TREE)), ["Docs"]);
    }

    #[test]
    fn nothing_inside_a_package_counts_even_when_it_is_the_root() {
        let tmp = tempfile_dir();
        let root = validate_folder(&tmp).unwrap();
        make(&root, &["Old.photoslibrary/originals/0"]);
        let package = root.join("Old.photoslibrary");
        assert_eq!(subfolders(&package, MAX_TREE), Subfolders::default());
        assert_eq!(
            subfolders(&package.join("originals"), MAX_TREE),
            Subfolders::default()
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_that_cannot_be_read_has_nothing_inside() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile_dir();
        let root = validate_folder(&tmp).unwrap();
        make(&root, &["Locked/inside", "Open/inside"]);
        let locked = root.join("Locked");
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        // Root reads through permissions, which would make this test meaningless.
        let readable = fs::read_dir(&locked).is_ok();
        let found = subfolders(&root, MAX_TREE);
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        if !readable {
            assert_eq!(relative(&root, &found), ["Locked", "Open", "Open/inside"]);
        }
    }

    #[test]
    fn packages_are_known_by_their_extension_in_any_case() {
        for name in [
            "Thing.app",
            "THING.APP",
            "Photos Library.photoslibrary",
            "Kit.framework",
            "Song.band",
            "Model.mlmodelc",
            "Talk.key",
            "a.b.xcodeproj",
            "Disk.sparsebundle",
        ] {
            assert!(is_package_name(name), "{name} is a package");
        }
        for name in [
            "app",
            ".app",
            "Thing.apple",
            "Thing.app.old",
            "Thing.",
            "Keys",
            "Photos",
        ] {
            assert!(!is_package_name(name), "{name} is not a package");
        }
    }
}
