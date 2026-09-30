//! The official collection: FolderSkin's own skins, standalone rather than in a pack, and the
//! `collection.json` contract they are listed in.
//!
//! The collection is a folder, `collection/`, beside `packs/` in folderskin-community, holding the
//! pictures and `collection.json`. It has one licence for all of its skins, one of those a pack can
//! have, and FolderSkin is its author. Unlike a pack it has no limit of 50 skins or 64 MB: it grows
//! as skins are added, and people use its skins one at a time rather than adding it whole. Each skin is held to the rules a skin in a pack is, and each picture to the
//! rules for a picture going into a pack now ([`pack::check_new_picture`]), so everything it
//! publishes the app can use. `folderskin-tools collection check` holds a checkout to this, and
//! `packs check` does too whenever there is a collection.
//!
//! A checkout with no `collection/` has an empty collection, which is fine.

use crate::pack;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// The folder the collection is, beside `packs/`.
pub const COLLECTION_DIR: &str = "collection";
/// The list of its skins, inside that folder.
pub const COLLECTION_FILE: &str = "collection.json";
/// The contract version `collection.json` declares in `"version"`.
pub const COLLECTION_VERSION: u32 = 1;
/// The licence `collection add` gives a new collection: MIT, which keeps FolderSkin's name with
/// the pictures.
pub const DEFAULT_LICENSE: &str = "MIT";
/// Most skins the collection lists. There's no limit meant here, only a bound on what a mistake
/// can cost: a list that long is a script gone wrong, not a collection.
pub const MAX_COLLECTION_SKINS: usize = 20_000;
/// Largest `collection.json`: [`MAX_COLLECTION_SKINS`] entries of about 150 bytes, twice over.
pub const MAX_COLLECTION_FILE_BYTES: usize = 8 * 1024 * 1024;
/// File name extensions a collection picture can have: the lossless ones.
pub const COLLECTION_EXTENSIONS: &[&str] = &["png", "webp"];

/// `collection.json`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Collection {
    pub version: u32,
    /// The licence of every skin in it: one of [`pack::LICENSES`].
    pub license: String,
    /// Its skins, oldest first: each one added goes at the end.
    pub skins: Vec<CollectionSkin>,
}

/// One skin of the collection.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CollectionSkin {
    /// A picture in `collection/`.
    pub file: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// The day it was added, `YYYY-MM-DD`.
    pub added: String,
}

impl Collection {
    /// Reads a `collection.json` and checks every rule in [`Collection::problems`]. The error lists
    /// each problem as a sentence.
    pub fn parse(bytes: &[u8]) -> Result<Collection, Vec<String>> {
        if bytes.len() > MAX_COLLECTION_FILE_BYTES {
            return Err(vec![format!(
                "{COLLECTION_FILE} is over {} MB",
                MAX_COLLECTION_FILE_BYTES / (1024 * 1024)
            )]);
        }
        // The version first, so a collection from a newer FolderSkin says so instead of failing
        // on a field this version does not know.
        #[derive(Deserialize)]
        struct Head {
            version: Option<serde_json::Value>,
        }
        let head: Head = serde_json::from_slice(bytes)
            .map_err(|e| vec![format!("{COLLECTION_FILE} is not valid JSON: {e}")])?;
        match head.version.as_ref().and_then(serde_json::Value::as_u64) {
            Some(v) if v == u64::from(COLLECTION_VERSION) => {}
            Some(v) if v > u64::from(COLLECTION_VERSION) => {
                return Err(vec![format!(
                    "this collection is for a newer FolderSkin (collection format {v})"
                )])
            }
            _ => {
                return Err(vec![format!(
                    "{COLLECTION_FILE} needs \"version\": {COLLECTION_VERSION}"
                )])
            }
        }
        let collection: Collection =
            serde_json::from_slice(bytes).map_err(|e| vec![format!("{COLLECTION_FILE}: {e}")])?;
        let problems = collection.problems();
        if problems.is_empty() {
            Ok(collection)
        } else {
            Err(problems)
        }
    }

    /// Everything wrong with the list itself, one sentence each; empty when it is fine. The
    /// pictures are checked one at a time with [`pack::check_new_picture`], and against each
    /// other by what is in them, by whoever has the folder.
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.version != COLLECTION_VERSION {
            problems.push(format!(
                "\"version\" is {COLLECTION_VERSION}, not {}",
                self.version
            ));
        }
        if !pack::LICENSES.contains(&self.license.as_str()) {
            problems.push(format!(
                "\"license\" must be one of {}",
                pack::LICENSES.join(", ")
            ));
        }
        if self.skins.len() > MAX_COLLECTION_SKINS {
            problems.push(format!(
                "the collection lists {} skins, and the most it can is {MAX_COLLECTION_SKINS}",
                self.skins.len()
            ));
        }
        let mut files = HashSet::new();
        for skin in &self.skins {
            let label = format!("skin {:?}", skin.file);
            if !is_collection_file_name(&skin.file) {
                problems.push(format!(
                    "{label}: a file name is letters, digits, dots, dashes and underscores, ending \
                     in .{}",
                    COLLECTION_EXTENSIONS.join(" or .")
                ));
            }
            if !files.insert(skin.file.to_ascii_lowercase()) {
                problems.push(format!("{label} is listed twice"));
            }
            if !pack::has_text(&skin.name, pack::MAX_SKIN_NAME_CHARS) {
                problems.push(format!(
                    "{label}: \"name\" must be 1 to {} characters",
                    pack::MAX_SKIN_NAME_CHARS
                ));
            }
            pack::check_tags(&skin.tags, pack::MAX_SKIN_TAGS, &label, &mut problems);
            if parse_date(&skin.added).is_none() {
                problems.push(format!(
                    "{label}: \"added\" must be a day written YYYY-MM-DD, such as 2026-09-30, not \
                     {:?}",
                    skin.added
                ));
            }
        }
        problems
    }
}

impl CollectionSkin {
    /// When it was added: 00:00 UTC of its day, in Unix seconds; 0 when the day isn't one
    /// ([`Collection::problems`] says so).
    pub fn added_at(&self) -> i64 {
        parse_date(&self.added).unwrap_or(0)
    }
}

/// A picture's file name in the collection: a pack picture's name ([`pack::is_picture_file_name`])
/// ending in .png or .webp, since every picture in it is lossless.
pub fn is_collection_file_name(file: &str) -> bool {
    let ext = file
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default();
    pack::is_picture_file_name(file) && COLLECTION_EXTENSIONS.contains(&ext.as_str())
}

const DAY: i64 = 24 * 60 * 60;

/// 00:00 UTC of `date`, written `YYYY-MM-DD`, in Unix seconds. `None` unless it is written just so
/// and is a day there was or will be, from 1970 on: no 31st of April, no 29th of February outside
/// a leap year.
pub fn parse_date(date: &str) -> Option<i64> {
    let b = date.as_bytes();
    let digits = |r: std::ops::Range<usize>| -> Option<u32> {
        b[r].iter().try_fold(0u32, |n, c| {
            c.is_ascii_digit().then(|| n * 10 + u32::from(c - b'0'))
        })
    };
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let (year, month, day) = (digits(0..4)?, digits(5..7)?, digits(8..10)?);
    if year < 1970 || !(1..=12).contains(&month) || day < 1 || day > days_in(year, month) {
        return None;
    }
    Some(days_from_civil(i64::from(year), month, day) * DAY)
}

/// The day of Unix seconds `at`, in UTC, written `YYYY-MM-DD`: what [`parse_date`] reads.
pub fn date_of(at: i64) -> String {
    let (year, month, day) = civil_from_days(at.div_euclid(DAY));
    format!("{year:04}-{month:02}-{day:02}")
}

fn days_in(year: u32, month: u32) -> u32 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => {
            29
        }
        2 => 28,
        _ => 31,
    }
}

/// Days from 1970-01-01 to a day of the proleptic Gregorian calendar (Howard Hinnant's
/// `days_from_civil`).
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_from_march = i64::from((month + 9) % 12);
    let day_of_year = (153 * month_from_march + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The day that is `days` from 1970-01-01, as (year, month, day): [`days_from_civil`] backwards.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_from_march + 2) / 5 + 1) as u32;
    let month = if month_from_march < 10 {
        month_from_march + 3
    } else {
        month_from_march - 9
    } as u32;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listing(skins: &[&str]) -> String {
        format!(
            r#"{{ "version": 1, "license": "MIT", "skins": [{}] }}"#,
            skins.join(", ")
        )
    }

    const GIRAFFE: &str = r#"{ "file": "giraffe-cola.webp", "name": "Giraffe cola", "tags": ["pop art"], "added": "2026-09-30" }"#;

    #[test]
    fn a_good_collection_reads_and_writes_back_as_it_was() {
        let plain = r#"{ "file": "koi.png", "name": "Koi", "added": "2026-10-01" }"#;
        let c = Collection::parse(listing(&[GIRAFFE, plain]).as_bytes()).unwrap();
        assert_eq!(c.skins.len(), 2);
        assert_eq!(c.skins[0].tags, ["pop art"]);
        assert!(c.skins[1].tags.is_empty());
        assert_eq!(c.skins[0].added_at(), 1_790_726_400, "2026-09-30 00:00 UTC");
        let json = serde_json::to_string(&c).unwrap();
        assert!(!json.contains(r#""tags":[]"#), "no tags, no field: {json}");
        assert_eq!(Collection::parse(json.as_bytes()).unwrap(), c);
        assert!(Collection::parse(listing(&[]).as_bytes())
            .unwrap()
            .skins
            .is_empty());
    }

    #[test]
    fn every_problem_is_reported_at_once() {
        let bad = [
            r#"{ "file": "../x.png", "name": "X", "added": "2026-09-30" }"#,
            r#"{ "file": "photo.jpg", "name": "Photo", "added": "2026-09-30" }"#,
            r#"{ "file": "Koi.png", "name": "", "added": "2026-02-29" }"#,
            r#"{ "file": "koi.PNG", "name": "Koi", "tags": ["Pop Art", "a", "b", "c"], "added": "30/09/2026" }"#,
        ];
        let problems = Collection::parse(listing(&bad).as_bytes())
            .unwrap_err()
            .join("\n");
        for needle in [
            r#"skin "../x.png": a file name"#,
            r#"skin "photo.jpg": a file name is letters, digits, dots, dashes and underscores, ending in .png or .webp"#,
            r#"skin "Koi.png": "name" must be 1 to 60 characters"#,
            r#"skin "Koi.png": "added" must be a day written YYYY-MM-DD, such as 2026-09-30, not "2026-02-29""#,
            r#"skin "koi.PNG" is listed twice"#,
            r#"skin "koi.PNG" has 4 tags, and the most is 3"#,
            r#"write the tag "Pop Art" as "pop art""#,
            r#"not "30/09/2026""#,
        ] {
            assert!(problems.contains(needle), "no {needle:?} in:\n{problems}");
        }
    }

    #[test]
    fn the_collection_has_one_licence_a_pack_could_have() {
        let c = Collection::parse(listing(&[GIRAFFE]).as_bytes()).unwrap();
        assert_eq!(c.license, "MIT");
        assert!(serde_json::to_string(&c)
            .unwrap()
            .starts_with(r#"{"version":1,"license":"MIT","skins":["#));
        let other = listing(&[GIRAFFE]).replace("\"MIT\"", "\"GPL-3.0\"");
        assert_eq!(
            Collection::parse(other.as_bytes()).unwrap_err(),
            ["\"license\" must be one of CC0-1.0, CC-BY-4.0, MIT"]
        );
        let none = r#"{ "version": 1, "skins": [] }"#;
        assert!(
            Collection::parse(none.as_bytes()).unwrap_err()[0].contains("missing field `license`")
        );
        assert!(pack::LICENSES.contains(&DEFAULT_LICENSE));
    }

    #[test]
    fn versions_and_unknown_fields_are_explained() {
        let newer = r#"{ "version": 2, "skins": "something new" }"#;
        assert_eq!(
            Collection::parse(newer.as_bytes()).unwrap_err(),
            ["this collection is for a newer FolderSkin (collection format 2)"]
        );
        assert!(Collection::parse(br#"{ "skins": [] }"#).unwrap_err()[0]
            .contains("needs \"version\": 1"));
        let typo = listing(&[&GIRAFFE.replace("\"tags\"", "\"tag\"")]);
        assert!(Collection::parse(typo.as_bytes()).unwrap_err()[0].contains("unknown field"));
        let extra = r#"{ "version": 1, "license": "MIT", "skins": [], "author": "x" }"#;
        assert!(Collection::parse(extra.as_bytes()).unwrap_err()[0].contains("unknown field"));
        assert!(Collection::parse(b"not json").unwrap_err()[0].contains("not valid JSON"));
        let huge = vec![b' '; MAX_COLLECTION_FILE_BYTES + 1];
        assert_eq!(
            Collection::parse(&huge).unwrap_err(),
            ["collection.json is over 8 MB"]
        );
    }

    #[test]
    fn a_collection_can_be_big_but_not_endless() {
        let skin = |i: usize| CollectionSkin {
            file: format!("s{i}.webp"),
            name: format!("Skin {i}"),
            tags: Vec::new(),
            added: "2026-09-30".into(),
        };
        let mut c = Collection {
            version: 1,
            license: "MIT".into(),
            skins: (0..MAX_COLLECTION_SKINS).map(skin).collect(),
        };
        assert!(c.problems().is_empty(), "far more than a pack's 50");
        c.skins.push(skin(MAX_COLLECTION_SKINS));
        assert_eq!(
            c.problems(),
            ["the collection lists 20001 skins, and the most it can is 20000"]
        );
    }

    #[test]
    fn only_real_days_are_dates() {
        assert_eq!(parse_date("1970-01-01"), Some(0));
        assert_eq!(parse_date("2026-09-30"), Some(1_790_726_400));
        assert_eq!(parse_date("2024-02-29"), Some(1_709_164_800), "a leap year");
        assert_eq!(
            parse_date("2000-02-29"),
            Some(951_782_400),
            "every 400 years"
        );
        for bad in [
            "2026-02-29",
            "1900-02-29",
            "2026-04-31",
            "2026-13-01",
            "2026-00-10",
            "2026-01-00",
            "1969-12-31",
            "2026-9-30",
            "2026/09/30",
            " 2026-09-30",
            "２０２６-09-30",
            "",
        ] {
            assert_eq!(parse_date(bad), None, "{bad:?}");
        }
        for day in [
            "1970-01-01",
            "2000-02-29",
            "2026-09-30",
            "2026-12-31",
            "2100-03-01",
        ] {
            assert_eq!(date_of(parse_date(day).unwrap()), day);
        }
        assert_eq!(
            date_of(1_790_726_400 + DAY - 1),
            "2026-09-30",
            "any time that day"
        );
    }

    #[test]
    fn collection_pictures_are_lossless_kinds_only() {
        assert!(is_collection_file_name("giraffe-cola.webp"));
        assert!(is_collection_file_name("Koi_01.PNG"));
        for bad in [
            "koi.jpg",
            "koi.jpeg",
            "../koi.png",
            "con.png",
            "koi",
            "collection.json",
        ] {
            assert!(!is_collection_file_name(bad), "{bad:?}");
        }
    }
}
