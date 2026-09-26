//! The folders inside a folder, for a skin that goes on a whole tree of them.
//!
//! Only the user's own folders count. Anything else is skipped, and never looked inside:
//! symlinks (and Windows junctions and mount points, which are the same thing to the standard
//! library), names starting with a dot, folders the OS hides, packages — apps, libraries and
//! documents that are folders on disk but a single item in Finder — the system locations
//! [`validate_folder`](super::validate_folder) refuses, and another volume mounted inside the
//! folder, such as a disk or a network share.
//!
//! A tree can be any size. [`Walk`] reads it one folder at a time, whenever whoever drives it asks
//! for the next, into a [`Tree`] that keeps each folder as its name and the number of the folder
//! it's in, so a run can start on the first folders while the rest are still being found, and a
//! million folders take a few tens of megabytes. A run reads breadth first ([`Order::Nearest`]),
//! so one that stops part way has done whole levels of the tree rather than one deep branch.
//! Counting reads depth first ([`Order::Deepest`]), which finishes each folder's insides before
//! the next one's, so the counts of the folders inside are known one by one. [`Choice`] is the
//! folders a run takes when not all of them.

use super::paths::is_system_location;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::fs::{DirEntry, Metadata};
use std::ops::Range;
use std::path::{Component, Path, PathBuf};

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

/// True when a folder called `name` is a package by its extension, whatever its case.
pub fn is_package_name(name: &str) -> bool {
    match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => PACKAGE_EXTENSIONS
            .iter()
            .any(|known| known.eq_ignore_ascii_case(extension)),
        _ => false,
    }
}

/// True when `folder` is a package or inside one: the folders in there belong to the app that
/// made it, not to the user, so as far as a tree goes it has nothing inside.
pub fn is_in_package(folder: &Path) -> bool {
    folder
        .components()
        .any(|part| is_package_name(&part.as_os_str().to_string_lossy()))
        || is_package(folder)
}

/// The user's own folders directly inside `dir`, by name as a file manager lists them (ignoring
/// case, then exactly, so the order is always the same). Nothing when `dir` can't be read.
///
/// Doesn't look at whether `dir` is a package itself: see [`is_in_package`].
pub fn folders_in(dir: &Path) -> Vec<OsString> {
    read_folders(dir, volume_of(dir))
}

/// True when `dir` has at least one of the user's own folders directly inside it. Stops at the
/// first, so it costs a folder full of files no more than [`folders_in`] would.
pub fn has_folders(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    let volume = volume_of(dir);
    entries
        .flatten()
        .any(|entry| is_users_folder(&entry, volume))
}

/// [`folders_in`] with the volume the folders have to be on: that of `dir` itself.
fn read_folders(dir: &Path, volume: Option<u64>) -> Vec<OsString> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<OsString> = entries
        .flatten()
        .filter(|entry| is_users_folder(entry, volume))
        .map(|entry| entry.file_name())
        .collect();
    // One lower-case copy per name rather than two per comparison: a folder can hold thousands.
    names.sort_by_cached_key(|name| (name.to_string_lossy().to_lowercase(), name.clone()));
    names
}

/// Folder order within one folder: by name ignoring case, as a file manager lists them, then
/// exactly, so names that differ only in case still come in one order.
fn by_name(a: &OsStr, b: &OsStr) -> Ordering {
    let fold = |name: &OsStr| name.to_string_lossy().to_lowercase();
    fold(a).cmp(&fold(b)).then_with(|| a.cmp(b))
}

/// True for an entry that is one of the user's own folders: a real folder (not a link to one) on
/// `volume`, not hidden either way, not a package and not the system's.
fn is_users_folder(entry: &DirEntry, volume: Option<u64>) -> bool {
    let Ok(kind) = entry.file_type() else {
        return false;
    };
    // `is_dir` is false for a symlink to a folder, and `is_symlink` covers junctions and a
    // Windows volume mounted in a folder too.
    if kind.is_symlink() || !kind.is_dir() {
        return false;
    }
    let name = entry.file_name();
    let name = name.to_string_lossy();
    if name.starts_with('.') || is_package_name(&name) {
        return false;
    }
    // An entry that can't be looked at is as good as hidden: it is most likely gone.
    let Ok(meta) = entry.metadata() else {
        return false;
    };
    if !on_volume(&meta, volume) || is_hidden_by_os(&meta) {
        return false;
    }
    let path = entry.path();
    !is_system_location(&path) && !is_package(&path)
}

/// The volume a folder is on, for leaving out another one mounted inside it. Only where the
/// standard library says (Unix); on Windows a volume mounted in a folder is a link, left out
/// with the rest of them.
#[cfg(unix)]
fn volume_of(dir: &Path) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(dir).ok().map(|meta| meta.dev())
}

#[cfg(not(unix))]
fn volume_of(_: &Path) -> Option<u64> {
    None
}

/// True when the entry `meta` describes is on `volume`, or there's no volume to be on.
#[cfg(unix)]
fn on_volume(meta: &Metadata, volume: Option<u64>) -> bool {
    use std::os::unix::fs::MetadataExt;
    volume.is_none_or(|dev| meta.dev() == dev)
}

#[cfg(not(unix))]
fn on_volume(_: &Metadata, _: Option<u64>) -> bool {
    true
}

/// macOS: the Finder's hidden flag (`chflags hidden`, `UF_HIDDEN`).
#[cfg(target_os = "macos")]
fn is_hidden_by_os(meta: &Metadata) -> bool {
    use std::os::macos::fs::MetadataExt;
    meta.st_flags() & libc::UF_HIDDEN != 0
}

/// Windows: the hidden or system attribute, which Explorer hides by default.
#[cfg(windows)]
fn is_hidden_by_os(meta: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_SYSTEM};
    meta.file_attributes() & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0
}

/// Elsewhere a leading dot is the only way to hide a folder.
#[cfg(not(any(target_os = "macos", windows)))]
fn is_hidden_by_os(_: &Metadata) -> bool {
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

// ---------- the tree ----------

/// The folders found inside one, each kept as its name and the folder it's in.
///
/// Folders are numbered as they're found: 0 is the folder the tree is of, and the folders
/// directly inside any one folder are numbered together, in name order, when that folder is read.
/// Nothing is ever taken out, so a number always means the same folder.
#[derive(Debug)]
pub struct Tree {
    root: PathBuf,
    /// Every folder's name but the root's, as the platform's bytes, one after another.
    names: Vec<u8>,
    /// Where each folder's name ends in `names`. It starts where the folder before it ends.
    ends: Vec<usize>,
    /// The folder each one is in. The root is in itself.
    parents: Vec<u32>,
    /// The first folder inside each one, once it has been read.
    firsts: Vec<u32>,
    /// How many folders are directly inside each one, or [`UNREAD`] until it has been read.
    counts: Vec<u32>,
}

/// A folder's count while it hasn't been read.
const UNREAD: u32 = u32::MAX;

/// Where a folder is in a [`Tree`], as far as it has been read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lookup {
    /// The folder with this number.
    At(u32),
    /// A folder on the way to it hasn't been read, so it isn't known yet.
    Unread,
    /// It isn't one of the tree's folders: the folder it would be in was read without it, or the
    /// path isn't inside the tree at all.
    Missing,
}

impl Tree {
    /// A tree of `root` alone, not read yet.
    pub fn new(root: PathBuf) -> Tree {
        Tree {
            root,
            names: Vec::new(),
            ends: vec![0],
            parents: vec![0],
            firsts: vec![0],
            counts: vec![UNREAD],
        }
    }

    /// The folder the tree is of.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// How many folders are in it so far, the root included.
    pub fn len(&self) -> usize {
        self.parents.len()
    }

    /// Never true: the root is always there.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// The folder's name; the root's is its whole path.
    pub fn name(&self, folder: u32) -> &OsStr {
        let k = folder as usize;
        if k == 0 {
            return self.root.as_os_str();
        }
        let bytes = &self.names[self.ends[k - 1]..self.ends[k]];
        // SAFETY: `bytes` is exactly one whole name `add` stored with `as_encoded_bytes`, in this
        // process, which is what `from_encoded_bytes_unchecked` takes.
        unsafe { OsStr::from_encoded_bytes_unchecked(bytes) }
    }

    /// The folder it's in, or `None` for the root.
    pub fn parent(&self, folder: u32) -> Option<u32> {
        (folder != 0).then(|| self.parents[folder as usize])
    }

    /// The folder's path: the root's, then the name of each folder down to it.
    pub fn path(&self, folder: u32) -> PathBuf {
        let mut chain = Vec::new();
        let mut at = folder;
        while at != 0 {
            chain.push(at);
            at = self.parents[at as usize];
        }
        let mut path = self.root.clone();
        for &k in chain.iter().rev() {
            path.push(self.name(k));
        }
        path
    }

    /// Whether the folder has been read, so the folders inside it are known.
    pub fn is_read(&self, folder: u32) -> bool {
        self.counts[folder as usize] != UNREAD
    }

    /// The numbers of the folders directly inside it, in name order: none until it's read.
    pub fn children(&self, folder: u32) -> Range<u32> {
        let k = folder as usize;
        match self.counts[k] {
            UNREAD => 0..0,
            count => self.firsts[k]..self.firsts[k] + count,
        }
    }

    /// Where the folder at `path` is in the tree, going down from the root by name.
    pub fn find(&self, path: &Path) -> Lookup {
        let Ok(rest) = path.strip_prefix(&self.root) else {
            return Lookup::Missing;
        };
        let mut at = 0;
        for part in rest.components() {
            let Component::Normal(name) = part else {
                return Lookup::Missing;
            };
            if !self.is_read(at) {
                return Lookup::Unread;
            }
            // The folders inside one are in name order, so it's a binary search among them.
            let Range {
                start: mut low,
                end: mut high,
            } = self.children(at);
            loop {
                if low >= high {
                    return Lookup::Missing;
                }
                let middle = low + (high - low) / 2;
                match by_name(self.name(middle), name) {
                    Ordering::Less => low = middle + 1,
                    Ordering::Greater => high = middle,
                    Ordering::Equal => break at = middle,
                }
            }
        }
        Lookup::At(at)
    }

    /// Puts the folders just read in `folder`, in the order given (name order), and returns
    /// their numbers.
    fn add(&mut self, folder: u32, names: impl ExactSizeIterator<Item = OsString>) -> Range<u32> {
        let first = self.number(self.len());
        let count = self.number(names.len());
        for name in names {
            self.names.extend_from_slice(name.as_encoded_bytes());
            self.ends.push(self.names.len());
            self.parents.push(folder);
            self.firsts.push(0);
            self.counts.push(UNREAD);
        }
        let k = folder as usize;
        self.firsts[k] = first;
        self.counts[k] = count;
        first..first + count
    }

    /// A folder's number. There's room for four billion folders, far more than any disk holds.
    fn number(&self, n: usize) -> u32 {
        u32::try_from(n).expect("fewer than four billion folders")
    }
}

// ---------- the walk ----------

/// The order a [`Walk`] reads a tree in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// Breadth first: the folder, then everything one level down, then two, as a run goes.
    Nearest,
    /// Depth first: each folder's insides, all the way down, before the next folder's.
    Deepest,
}

/// A tree read one folder at a time: [`next_folder`](Walk::next_folder) says which folder to
/// read next, the caller reads it (with [`folders_in`], and without holding anything, since it
/// can take a while on a network disk), and [`found`](Walk::found) puts what it found in the tree.
#[derive(Debug)]
pub struct Walk {
    tree: Tree,
    order: Order,
    /// Whether each folder is to be read when its turn comes. A run leaves folders out.
    look: Vec<bool>,
    /// [`Order::Nearest`]: the next folder whose turn it is, by number.
    next: usize,
    /// [`Order::Deepest`]: the folders still to read, the next one last.
    stack: Vec<u32>,
    /// Folders to read that haven't been handed out.
    left: usize,
    /// The folder handed out and not yet found.
    reading: Option<u32>,
}

impl Walk {
    /// A walk of `root` (canonical, as [`validate_folder`](super::validate_folder) returns it),
    /// with nothing read yet. A package, or a folder inside one, has nothing to read.
    pub fn new(root: PathBuf, order: Order) -> Walk {
        let look = !is_in_package(&root);
        Walk {
            tree: Tree::new(root),
            order,
            look: vec![look],
            next: 0,
            stack: if look && order == Order::Deepest {
                vec![0]
            } else {
                Vec::new()
            },
            left: usize::from(look),
            reading: None,
        }
    }

    /// What has been found so far.
    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    /// The tree, for keeping once the walk is over.
    pub fn into_tree(self) -> Tree {
        self.tree
    }

    /// Everything there is to read has been read.
    pub fn is_done(&self) -> bool {
        self.left == 0 && self.reading.is_none()
    }

    /// The next folder to read, and its path: `None` when there's none left, or one is being
    /// read (a walk reads one folder at a time).
    pub fn next_folder(&mut self) -> Option<(u32, PathBuf)> {
        if self.reading.is_some() || self.left == 0 {
            return None;
        }
        let folder = match self.order {
            Order::Nearest => loop {
                let k = self.next;
                self.next += 1;
                if self.look[k] {
                    break self.tree.number(k);
                }
            },
            Order::Deepest => self.stack.pop()?,
        };
        self.left -= 1;
        self.reading = Some(folder);
        Some((folder, self.tree.path(folder)))
    }

    /// What reading `folder` found: the folders directly inside it in name order, each with
    /// whether to read it in turn. Returns their numbers.
    pub fn found(&mut self, folder: u32, inside: Vec<(OsString, bool)>) -> Range<u32> {
        debug_assert_eq!(self.reading, Some(folder), "found the folder handed out");
        self.reading = None;
        let looks: Vec<bool> = inside.iter().map(|(_, look)| *look).collect();
        let added = self
            .tree
            .add(folder, inside.into_iter().map(|(name, _)| name));
        self.left += looks.iter().filter(|&&look| look).count();
        if self.order == Order::Deepest {
            // Last on the stack is read first, so the first by name goes on last.
            for (k, &look) in added.clone().zip(&looks).rev() {
                if look {
                    self.stack.push(k);
                }
            }
        }
        self.look.extend(looks);
        added
    }
}

// ---------- which folders ----------

/// Which folders inside one a run takes when not all of them, as rules rather than a list:
/// ticking or clearing a folder decides it and everything inside it, unless a rule further down
/// says otherwise. So a choice is as small as the ticks that made it, however big the tree.
#[derive(Clone, Debug)]
pub struct Choice {
    /// Whether the folders inside are taken where no rule says otherwise.
    all: bool,
    /// Folders ticked (true) or cleared (false), by path.
    rules: HashMap<PathBuf, bool>,
    /// Every folder with a ticked folder somewhere inside it, which a run reads even when it's
    /// cleared.
    ticked_inside: HashSet<PathBuf>,
}

impl Choice {
    /// Every folder inside.
    pub fn everything() -> Choice {
        Choice::new(true, [])
    }

    /// The folders inside taken or not by default (`all`), and the rules that say otherwise.
    pub fn new(all: bool, rules: impl IntoIterator<Item = (PathBuf, bool)>) -> Choice {
        let rules: HashMap<PathBuf, bool> = rules.into_iter().collect();
        let ticked_inside = rules
            .iter()
            .filter(|(_, &on)| on)
            .flat_map(|(path, _)| path.ancestors().skip(1).map(Path::to_path_buf))
            .collect();
        Choice {
            all,
            rules,
            ticked_inside,
        }
    }

    /// Whether the folders directly inside the root are taken where no rule says otherwise.
    pub fn all(&self) -> bool {
        self.all
    }

    /// Whether `folder` is taken, given whether the folder it's in takes what's inside it (for a
    /// folder directly inside the root, [`all`](Choice::all)): its own rule, or that.
    pub fn takes(&self, folder: &Path, inside_taken: bool) -> bool {
        self.rules.get(folder).copied().unwrap_or(inside_taken)
    }

    /// Whether a run has to look inside `folder` (taken or not, as `taken` says): when it's
    /// taken, what's inside is too unless a rule says otherwise, and when it isn't, only a
    /// folder ticked further down is.
    pub fn looks_inside(&self, folder: &Path, taken: bool) -> bool {
        taken || self.ticked_inside.contains(folder)
    }
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

    /// Reads the whole tree of `root` in `order`, as a count or a run does.
    fn walk(root: &Path, order: Order) -> Walk {
        let mut walk = Walk::new(root.to_path_buf(), order);
        while let Some((folder, path)) = walk.next_folder() {
            let inside = folders_in(&path).into_iter().map(|n| (n, true)).collect();
            walk.found(folder, inside);
        }
        assert!(walk.is_done());
        walk
    }

    /// Every folder found, the root left out, as `/`-separated paths relative to it, in the
    /// order they were numbered.
    fn found(walk: &Walk) -> Vec<String> {
        let tree = walk.tree();
        (1..tree.len() as u32)
            .map(|k| relative(tree.root(), &tree.path(k)))
            .collect()
    }

    /// The order a walk in `order` reads the folders in, the root left out.
    fn read_order(root: &Path, order: Order) -> Vec<String> {
        let mut walk = Walk::new(root.to_path_buf(), order);
        let mut read = Vec::new();
        while let Some((folder, path)) = walk.next_folder() {
            if folder != 0 {
                read.push(relative(root, &path));
            }
            let inside = folders_in(&path).into_iter().map(|n| (n, true)).collect();
            walk.found(folder, inside);
        }
        read
    }

    fn relative(root: &Path, path: &Path) -> String {
        path.strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/")
    }

    #[test]
    fn nearer_folders_come_first_and_each_level_is_in_name_order() {
        let tmp = tempfile_dir();
        let root = validate_folder(&tmp).unwrap();
        make(&root, &["b/z", "b/A", "a/y/deep", "C"]);
        let walk = walk(&root, Order::Nearest);
        assert_eq!(
            found(&walk),
            ["a", "b", "C", "a/y", "b/A", "b/z", "a/y/deep"]
        );
        assert_eq!(
            read_order(&root, Order::Nearest),
            ["a", "b", "C", "a/y", "b/A", "b/z", "a/y/deep"]
        );
        let tree = walk.tree();
        assert!(
            (0..tree.len() as u32).all(|k| tree.path(k).starts_with(&root)),
            "the paths are under the canonical root"
        );
    }

    #[test]
    fn deepest_first_reads_each_folders_insides_before_the_next_folder() {
        let tmp = tempfile_dir();
        let root = validate_folder(&tmp).unwrap();
        make(&root, &["b/z", "b/A", "a/y/deep", "C"]);
        assert_eq!(
            read_order(&root, Order::Deepest),
            ["a", "a/y", "a/y/deep", "b", "b/A", "b/z", "C"]
        );
        // The folders inside one folder are still numbered together, in name order.
        let walk = walk(&root, Order::Deepest);
        let tree = walk.tree();
        let names = |k: u32| -> Vec<String> {
            tree.children(k)
                .map(|c| tree.name(c).to_string_lossy().into_owned())
                .collect()
        };
        assert_eq!(names(0), ["a", "b", "C"]);
        let Lookup::At(b) = tree.find(&root.join("b")) else {
            panic!("b is in the tree");
        };
        assert_eq!(names(b), ["A", "z"]);
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

        // Read from disk, a folder's names come in the same order.
        let tmp = tempfile_dir();
        let root = validate_folder(&tmp).unwrap();
        make(&root, &["b", "a", "C", "10", "9"]);
        assert_eq!(folders_in(&root), ["10", "9", "a", "b", "C"]);
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
        assert_eq!(
            found(&walk(&root, Order::Nearest)),
            ["Mine", "Photos", "Photos/2024"]
        );
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
        assert_eq!(found(&walk(&root, Order::Nearest)), ["Mine"]);
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
        assert_eq!(found(&walk(&root, Order::Nearest)), ["Mine"]);
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_on_another_volume_is_left_out() {
        use std::os::unix::fs::MetadataExt;
        let tmp = tempfile_dir();
        let root = validate_folder(&tmp).unwrap();
        make(&root, &["Mine/inside", "Also mine"]);
        let here = fs::metadata(&root).unwrap().dev();
        assert_eq!(read_folders(&root, Some(here)), ["Also mine", "Mine"]);
        // Seen from another volume, none of them is on it: a disk or a share mounted inside the
        // folder is left out the same way, with everything on it.
        assert!(read_folders(&root, Some(here.wrapping_add(1))).is_empty());
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
        assert_eq!(found(&walk(&root, Order::Nearest)), ["Shown"]);
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
        assert_eq!(found(&walk(&root, Order::Nearest)), ["Shown"]);
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
        assert_eq!(found(&walk(&root, Order::Nearest)), ["Plain"]);
    }

    #[test]
    fn files_never_count() {
        let tmp = tempfile_dir();
        let root = validate_folder(&tmp).unwrap();
        make(&root, &["Docs"]);
        fs::write(root.join("notes.txt"), b"x").unwrap();
        fs::write(root.join("Report"), b"x").unwrap();
        fs::write(root.join("Docs").join("Draft"), b"x").unwrap();
        assert_eq!(found(&walk(&root, Order::Nearest)), ["Docs"]);
        assert!(has_folders(&root));
        assert!(!has_folders(&root.join("Docs")), "a folder of files");
    }

    #[test]
    fn nothing_inside_a_package_counts_even_when_it_is_the_root() {
        let tmp = tempfile_dir();
        let root = validate_folder(&tmp).unwrap();
        make(&root, &["Old.photoslibrary/originals/0"]);
        let package = root.join("Old.photoslibrary");
        for inside in [package.clone(), package.join("originals")] {
            let mut walk = Walk::new(inside, Order::Nearest);
            assert!(walk.is_done(), "nothing to read");
            assert_eq!(walk.next_folder(), None);
            assert_eq!(walk.tree().len(), 1);
        }
        assert!(is_in_package(&package.join("originals")));
        assert!(!is_in_package(&root));
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
        let walk = walk(&root, Order::Nearest);
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        if !readable {
            assert_eq!(found(&walk), ["Locked", "Open", "Open/inside"]);
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

    #[test]
    fn a_tree_finds_its_folders_by_path_as_far_as_it_has_been_read() {
        let mut tree = Tree::new(PathBuf::from("/work/Projects"));
        let names = |list: &[&str]| -> Vec<OsString> { list.iter().map(OsString::from).collect() };
        assert_eq!(tree.find(Path::new("/work/Projects")), Lookup::At(0));
        assert_eq!(
            tree.find(Path::new("/work/Projects/Clients")),
            Lookup::Unread
        );
        assert_eq!(
            tree.add(0, names(&["Clients", "Design", "Été"]).into_iter()),
            1..4
        );
        assert_eq!(tree.add(3, names(&["Photos"]).into_iter()), 4..5);
        assert_eq!(tree.add(1, Vec::new().into_iter()), 5..5);

        assert_eq!(tree.len(), 5);
        assert_eq!(tree.name(3), "Été");
        assert_eq!(tree.parent(4), Some(3));
        assert_eq!(tree.parent(0), None);
        assert_eq!(tree.path(4), Path::new("/work/Projects/Été/Photos"));
        assert_eq!(tree.children(0), 1..4);
        assert!(tree.is_read(1) && tree.children(1).is_empty());
        assert!(!tree.is_read(2));

        let at = |p: &str| tree.find(&Path::new("/work/Projects").join(p));
        assert_eq!(at("Été/Photos"), Lookup::At(4));
        assert_eq!(at("Design"), Lookup::At(2));
        assert_eq!(at("Design/Mockups"), Lookup::Unread, "Design isn't read");
        assert_eq!(
            at("Clients/Acme"),
            Lookup::Missing,
            "Clients was read without it"
        );
        assert_eq!(at("Nope"), Lookup::Missing);
        assert_eq!(tree.find(Path::new("/elsewhere")), Lookup::Missing);
    }

    #[test]
    fn a_walk_reads_only_the_folders_it_is_told_to_look_inside() {
        let tmp = tempfile_dir();
        let root = validate_folder(&tmp).unwrap();
        make(&root, &["Keep/in", "Skip/never", "Last"]);
        let mut walk = Walk::new(root.clone(), Order::Nearest);
        let mut read = Vec::new();
        while let Some((folder, path)) = walk.next_folder() {
            assert_eq!(walk.next_folder(), None, "one folder at a time");
            read.push(relative(&root, &path));
            let inside = folders_in(&path)
                .into_iter()
                .map(|name| {
                    let look = name != "Skip";
                    (name, look)
                })
                .collect();
            walk.found(folder, inside);
        }
        assert!(walk.is_done());
        assert_eq!(read, ["", "Keep", "Last", "Keep/in"]);
        assert_eq!(found(&walk), ["Keep", "Last", "Skip", "Keep/in"]);
    }

    #[test]
    fn a_million_folders_take_tens_of_megabytes_not_hundreds() {
        let tmp = tempfile_dir();
        let root = validate_folder(&tmp).unwrap();
        let mut walk = Walk::new(root, Order::Nearest);
        // A hundred folders in the folder, a hundred in each of those and a hundred in each of
        // theirs, named a dozen letters long ("Project 0042"), made up rather than read.
        while let Some((folder, _)) = walk.next_folder() {
            let tree = walk.tree();
            let depth = std::iter::successors(Some(folder), |&k| tree.parent(k)).count();
            let inside = (0..100)
                .map(|k| (OsString::from(format!("Project {k:04}")), depth < 3))
                .collect();
            walk.found(folder, inside);
        }
        assert!(walk.is_done());
        let tree = walk.tree();
        assert_eq!(tree.len(), 1 + 100 + 10_000 + 1_000_000);
        let words = tree.parents.capacity() + tree.firsts.capacity() + tree.counts.capacity();
        let bytes = tree.names.capacity()
            + tree.ends.capacity() * size_of::<usize>()
            + words * size_of::<u32>()
            + walk.look.capacity()
            + walk.stack.capacity() * size_of::<u32>();
        // About 35 MB.
        let megabytes = bytes as f64 / 1e6;
        assert!(megabytes < 48.0, "{megabytes:.1} MB");
    }

    #[test]
    fn a_choice_takes_a_folder_by_its_own_rule_or_the_one_above() {
        let p = |s: &str| PathBuf::from("/r").join(s);
        let choice = Choice::new(
            true,
            [(p("Photos"), false), (p("Photos/2021/Holiday"), true)],
        );
        assert!(choice.all());
        assert!(choice.takes(&p("Clients"), true));
        assert!(!choice.takes(&p("Photos"), true));
        assert!(
            !choice.takes(&p("Photos/2020"), false),
            "inside a cleared folder"
        );
        assert!(choice.takes(&p("Photos/2021/Holiday"), false));
        // A cleared folder is looked inside only where something further down is ticked.
        assert!(choice.looks_inside(&p("Photos"), false));
        assert!(choice.looks_inside(&p("Photos/2021"), false));
        assert!(!choice.looks_inside(&p("Photos/2020"), false));
        assert!(choice.looks_inside(&p("Clients"), true));

        let none = Choice::new(false, [(p("Invoices/2026"), true)]);
        assert!(!none.takes(&p("Clients"), none.all()));
        assert!(none.looks_inside(&p("Invoices"), false));
        assert!(!none.looks_inside(&p("Clients"), false));
        assert!(Choice::everything().takes(&p("Anything"), true));
    }
}
