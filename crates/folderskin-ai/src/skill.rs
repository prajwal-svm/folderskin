//! Saved prompts, kept as skills: one format for what the chat's "/" menu lists as "Your prompts",
//! the prompt composer, the command line and the accuracy harness.
//!
//! A skill keeps what a picture shows apart from how it is rendered, in plain words that suit
//! every provider, with anything provider-specific pushed to optional overrides:
//!
//! ```jsonc
//! {
//!   "format": "folderskin.skill/1",
//!   "id": "night-prints-7k2q",          // made once from the name, never changes
//!   "name": "Night prints",             // what the "/" menu shows
//!   "command": "night-prints",          // what is typed after "/"
//!   "summary": "…",                     // a line under the name, optional
//!   "base_style": "woodblock",          // HOW IT LOOKS: a built-in style it uses or refines,
//!   "treatment": "a woodblock-…: …",    //   its own treatment words instead of the style's,
//!   "palette": [{"hex": "#1D2B53", "name": "deep indigo"}],
//!   "light": "cool moonlight from the upper left",
//!   "idea": "a koi pond at night",      // WHAT IT SHOWS: the words it puts in the box,
//!   "idea_template": "{idea}, at night",//   or a frame for the words typed with it
//!   "lettering": {"text": null, "look": "…", "placement": "centre"},
//!   "references": [{"role": "style", "file": "…", "sha256": "…"}],
//!   "shapes": ["artwork", "folder", "icon"],
//!   "keep_out": ["Mount Fuji"],         // only ever a negative prompt, and checked
//!   "checks": ["carved black outlines"],
//!   "providers": {"local": {"treatment": "…"}, "stability": {"style_preset": null}},
//!   "tested": [], "origin": {}, "created": "2026-09-20T10:00:00Z", "updated": "…"
//! }
//! ```
//!
//! `idea` is FolderSkin's own addition for a saved prompt: the words it fills the box with.
//! [`check`] is the one place the rules a skill must meet are enforced, whoever saves it.

use crate::recipe::{join_and, trim_sentence, Treatment};
use crate::styles;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// What a skill file says it is.
pub const FORMAT: &str = "folderskin.skill/1";
/// The longest a name can be, in characters, as the menu shows it.
pub const MAX_NAME_CHARS: usize = 60;
/// The most a skill can hold, as JSON: a longer treatment makes a style that overrides the idea.
pub const MAX_BYTES: usize = 4096;
/// The most colours a palette can have.
pub const MAX_COLOURS: usize = 6;
/// How long a treatment is, in words: long enough to pin a technique, short enough not to outvote
/// the idea.
pub const TREATMENT_WORDS: std::ops::RangeInclusive<usize> = 8..=60;

fn format_id() -> String {
    FORMAT.to_string()
}

/// One saved prompt.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Skill {
    #[serde(default = "format_id")]
    pub format: String,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub command: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_style: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub treatment: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub palette: Vec<Swatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idea: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idea_template: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lettering: Option<SkillLettering>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<SkillReference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shapes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keep_out: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks: Vec<String>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub providers: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tested: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<Value>,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: String,
}

/// A colour of a skill's palette.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Swatch {
    pub hex: String,
    #[serde(default)]
    pub name: String,
}

/// How a skill letters words.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillLettering {
    /// Words it always letters; `None` letters only what the person quotes.
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub look: Option<String>,
    #[serde(default)]
    pub placement: Option<String>,
}

/// A picture a skill carries, kept in the app's data folder.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillReference {
    pub role: String,
    pub file: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

impl Skill {
    /// A prompt saved from the chat: `idea`, the words in the box, under `name`, with the style
    /// chosen for it (a built-in style's id), made at `now`.
    pub fn saved(name: &str, idea: &str, base_style: Option<&str>, now: &str) -> Skill {
        let name = one_line(name);
        Skill {
            format: FORMAT.to_string(),
            id: new_id(&name),
            command: command_for(&name),
            name,
            base_style: base_style
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
            idea: Some(idea.trim().to_string()).filter(|i| !i.is_empty()),
            created: now.to_string(),
            updated: now.to_string(),
            ..Skill::default()
        }
    }

    /// The built-in style it uses or refines.
    pub fn style(&self) -> Option<&'static styles::Style> {
        self.base_style.as_deref().and_then(styles::style)
    }

    /// Whether it has a look of its own beyond a built-in style: its own treatment, light,
    /// palette, lettering look or keep-outs.
    pub fn has_own_look(&self) -> bool {
        self.treatment.is_some()
            || self.light.is_some()
            || !self.palette.is_empty()
            || !self.keep_out.is_empty()
            || self.lettering.as_ref().is_some_and(|l| l.look.is_some())
    }

    /// How it renders a picture for `provider`: its own treatment (the provider's override first)
    /// or its style's, with its light and palette worked in, its lettering look and its
    /// keep-outs. `None` when it has no look at all, only words.
    pub fn treatment_for(&self, provider: &str) -> Option<Treatment> {
        let style = self.style();
        let own = self
            .override_str(provider, "treatment")
            .or(self.treatment.as_deref())
            .map(trim_sentence)
            .filter(|t| !t.is_empty());
        let base = style.map(|s| s.fragment.as_str());
        let mut words = own.or(base)?.to_string();
        if let Some(light) = self
            .light
            .as_deref()
            .map(trim_sentence)
            .filter(|l| !l.is_empty())
        {
            words = format!("{words}, {light}");
        }
        let colours: Vec<String> = self
            .palette
            .iter()
            .map(|c| {
                if c.name.trim().is_empty() {
                    name_of(&c.hex)
                } else {
                    c.name.trim().to_string()
                }
            })
            .filter(|n| !n.is_empty())
            .collect();
        if !colours.is_empty() {
            words = format!("{words}, in a palette of {}", join_and(&colours));
        }
        let mut treatment = match style {
            Some(s) => Treatment::of(s),
            None => Treatment::default(),
        };
        treatment.id = self.id.clone();
        treatment.words = words;
        if let Some(look) = self.lettering.as_ref().and_then(|l| l.look.as_deref()) {
            treatment.lettering = Some(look.trim().to_string()).filter(|l| !l.is_empty());
        }
        for k in &self.keep_out {
            if !treatment.keep_out.iter().any(|o| o.eq_ignore_ascii_case(k)) {
                treatment.keep_out.push(k.clone());
            }
        }
        if let Some(p) = self.override_str("stability", "style_preset") {
            treatment.native.stability = Some(p.to_string());
        }
        if let Some(t) = self.override_str("ideogram", "style_type") {
            treatment.native.ideogram_type = Some(t.to_string());
        }
        if let Some(p) = self.override_str("ideogram", "style_preset") {
            treatment.native.ideogram_preset = Some(p.to_string());
        }
        Some(treatment)
    }

    /// The idea it paints for what's typed: `typed` in its idea template when it has one.
    pub fn idea_for(&self, typed: &str) -> String {
        match &self.idea_template {
            Some(t) if t.contains("{idea}") => t.replacen("{idea}", typed.trim(), 1),
            _ => typed.trim().to_string(),
        }
    }

    fn override_str(&self, provider: &str, field: &str) -> Option<&str> {
        self.providers
            .get(provider)?
            .get(field)?
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }
}

/// What [`check`] noticed that doesn't stop a skill from being saved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Warning {
    /// It names someone as its style ("in the style of X"): a style made from a living artist's
    /// or a studio's name can't go in a pack (docs/PACK-TERMS.md, rule 3). Describing the
    /// technique works better anyway.
    NamesSomeone(String),
}

/// The rules every skill meets, whoever saves it: the app, the command line or the harness. It
/// tidies what it can (spaces, a missing command, tool syntax no provider reads, a colour's
/// missing name) and refuses what it can't, with a sentence to show. What it lets through with a
/// note comes back as warnings.
pub fn check(skill: &mut Skill) -> Result<Vec<Warning>, String> {
    skill.format = FORMAT.to_string();
    skill.name = one_line(&skill.name);
    if skill.name.is_empty() {
        return Err("give the prompt a name".into());
    }
    if skill.name.chars().count() > MAX_NAME_CHARS {
        return Err(format!(
            "that name is too long. Keep it to {MAX_NAME_CHARS} characters"
        ));
    }
    if skill.command.trim().is_empty() {
        skill.command = command_for(&skill.name);
    }
    if !skill
        .command
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err("a prompt's command can only have a to z, 0 to 9 and dashes".into());
    }
    skill.idea = skill
        .idea
        .as_deref()
        .map(|i| strip_tool_syntax(i.trim()))
        .filter(|i| !i.is_empty());
    skill.treatment = skill
        .treatment
        .as_deref()
        .map(|t| strip_tool_syntax(trim_sentence(t)))
        .filter(|t| !t.is_empty());
    if let Some(style) = &skill.base_style {
        match styles::style(style) {
            Some(s) => skill.base_style = Some(s.id.clone()),
            None => return Err(format!("FolderSkin has no style called {style:?}")),
        }
    }
    if skill.idea.is_none()
        && skill.idea_template.is_none()
        && skill.treatment.is_none()
        && skill.base_style.is_none()
    {
        return Err("there's nothing to save yet. Write the prompt first".into());
    }
    if let Some(t) = &skill.treatment {
        let words = t.split_whitespace().count();
        if !TREATMENT_WORDS.contains(&words) {
            return Err(format!(
                "a look is described in {} to {} words, and this one has {words}",
                TREATMENT_WORDS.start(),
                TREATMENT_WORDS.end()
            ));
        }
        if let Some(word) = negation(t) {
            return Err(format!(
                "describe the look by what it has, not by what it hasn't (\u{201C}{word}\u{201D}). \
                 Put what to leave out in its keep-outs"
            ));
        }
    }
    if let Some(template) = &skill.idea_template {
        if template.matches("{idea}").count() != 1 {
            return Err("an idea template has {idea} in it exactly once".into());
        }
    }
    if skill.palette.len() > MAX_COLOURS {
        return Err(format!("a palette has at most {MAX_COLOURS} colours"));
    }
    for colour in &mut skill.palette {
        if parse_hex(&colour.hex).is_none() {
            return Err(format!("{:?} isn't a colour like #1D2B53", colour.hex));
        }
        colour.hex = colour.hex.trim().to_uppercase();
        if colour.name.trim().is_empty() {
            colour.name = name_of(&colour.hex);
        }
    }
    for r in &skill.references {
        if !["style", "subject", "palette"].contains(&r.role.as_str()) {
            return Err(format!(
                "a picture is a subject, a style or a palette, not {:?}",
                r.role
            ));
        }
    }
    skill.keep_out = skill
        .keep_out
        .iter()
        .map(|k| one_line(k))
        .filter(|k| !k.is_empty())
        .collect();
    let size = serde_json::to_vec(skill).map_or(usize::MAX, |b| b.len());
    if size > MAX_BYTES {
        return Err("that prompt is too long to keep. Make it shorter".into());
    }
    let mut warnings = Vec::new();
    for text in [skill.treatment.as_deref(), skill.idea.as_deref()]
        .into_iter()
        .flatten()
    {
        if let Some(who) = names_someone(text) {
            warnings.push(Warning::NamesSomeone(who));
        }
    }
    Ok(warnings)
}

/// A word that says what a look hasn't: the local model has no negative prompt, and naming a
/// thing tends to paint it.
fn negation(text: &str) -> Option<&'static str> {
    let lower = format!(
        " {} ",
        text.to_lowercase().replace([',', '.', ';', ':'], " ")
    );
    ["no", "not", "without", "avoid", "don't", "never"]
        .into_iter()
        .find(|w| lower.contains(&format!(" {w} ")))
}

/// Syntax only some image tools read, which FolderSkin's providers would paint as text or ignore:
/// `(word:1.3)` weights, `--sref …` and other flags, `::` weights and `[[ ]]`.
pub fn strip_tool_syntax(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            // (word:1.3) → word
            '(' => {
                let rest: String = chars.clone().take_while(|&c| c != ')').collect();
                match rest.rsplit_once(':') {
                    Some((word, weight))
                        if !weight.is_empty()
                            && weight.chars().all(|c| c.is_ascii_digit() || c == '.')
                            && chars.clone().nth(rest.chars().count()) == Some(')') =>
                    {
                        out.push_str(word.trim());
                        for _ in 0..=rest.chars().count() {
                            chars.next();
                        }
                    }
                    _ => out.push(c),
                }
            }
            '[' if chars.peek() == Some(&'[') => {
                chars.next();
            }
            ']' if chars.peek() == Some(&']') => {
                chars.next();
            }
            ':' if chars.peek() == Some(&':') => {
                chars.next();
                // A weight after it, "::2", goes with it.
                while chars
                    .peek()
                    .is_some_and(|c| c.is_ascii_digit() || *c == '.')
                {
                    chars.next();
                }
            }
            _ => out.push(c),
        }
    }
    // --flags and what they're given: "--sref 123", "--ar 16:9", "--v 6".
    let mut words: Vec<&str> = Vec::new();
    let mut skip = false;
    for word in out.split_whitespace() {
        if skip {
            skip = false;
            continue;
        }
        if let Some(flag) = word.strip_prefix("--") {
            skip = !flag.is_empty();
            continue;
        }
        words.push(word);
    }
    words.join(" ")
}

/// Who a text names as its style: a capitalised name after "by" or "in the style of".
pub fn names_someone(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let capital = |w: &str| {
        w.chars().next().is_some_and(char::is_uppercase)
            && w.chars().filter(|c| c.is_alphabetic()).count() > 1
    };
    for (i, w) in words.iter().enumerate() {
        let lower = w.to_lowercase();
        let after = if lower == "by" {
            Some(i + 1)
        } else if lower == "style"
            && i >= 2
            && words[i - 2].eq_ignore_ascii_case("in")
            && words[i - 1].eq_ignore_ascii_case("the")
            && words
                .get(i + 1)
                .is_some_and(|o| o.eq_ignore_ascii_case("of"))
        {
            Some(i + 2)
        } else {
            None
        };
        let Some(at) = after else { continue };
        let name: Vec<&str> = words[at.min(words.len())..]
            .iter()
            .take_while(|w| capital(w))
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
            .collect();
        if !name.is_empty() {
            return Some(name.join(" "));
        }
    }
    None
}

/// `#RRGGBB` as its three channels.
fn parse_hex(hex: &str) -> Option<[u8; 3]> {
    let h = hex.trim().strip_prefix('#')?;
    if h.len() != 6 || !h.is_ascii() {
        return None;
    }
    let channel = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some([channel(0)?, channel(2)?, channel(4)?])
}

/// The nearest plain colour name for `hex`, since most models read names more reliably than hex.
pub fn name_of(hex: &str) -> String {
    const NAMES: &[(&str, [u8; 3])] = &[
        ("black", [0, 0, 0]),
        ("charcoal", [54, 69, 79]),
        ("grey", [128, 128, 128]),
        ("silver", [192, 192, 192]),
        ("white", [255, 255, 255]),
        ("cream", [255, 248, 220]),
        ("red", [200, 30, 30]),
        ("crimson", [150, 20, 40]),
        ("orange", [245, 130, 30]),
        ("tangerine", [255, 160, 60]),
        ("gold", [212, 175, 55]),
        ("yellow", [250, 220, 40]),
        ("ochre", [204, 119, 34]),
        ("brown", [120, 72, 40]),
        ("olive", [110, 115, 40]),
        ("green", [40, 150, 60]),
        ("teal", [0, 128, 128]),
        ("turquoise", [64, 208, 200]),
        ("sky blue", [120, 190, 240]),
        ("blue", [30, 80, 200]),
        ("deep indigo", [30, 40, 90]),
        ("navy", [20, 30, 70]),
        ("violet", [130, 70, 190]),
        ("purple", [110, 40, 120]),
        ("pink", [240, 140, 180]),
        ("magenta", [220, 20, 160]),
    ];
    let Some(c) = parse_hex(hex) else {
        return String::new();
    };
    let distance = |n: &[u8; 3]| {
        (0..3)
            .map(|i| (i32::from(c[i]) - i32::from(n[i])).pow(2))
            .sum::<i32>()
    };
    NAMES
        .iter()
        .min_by_key(|(_, n)| distance(n))
        .map(|(name, _)| name.to_string())
        .unwrap_or_default()
}

/// `text` with its runs of spaces and line breaks made one space, trimmed.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// What's typed after "/" for a skill called `name`: "Night prints" is "night-prints".
pub fn command_for(name: &str) -> String {
    let mut out = String::new();
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-');
    if out.is_empty() {
        "prompt".into()
    } else {
        out.chars()
            .take(40)
            .collect::<String>()
            .trim_end_matches('-')
            .to_string()
    }
}

/// An id made once from the name, with four characters no other has: "night-prints-7k2q".
fn new_id(name: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    let mut n = nanos.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(
        COUNTER
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_mul(1_442_695_040_888_963_407),
    );
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut tail = String::new();
    for _ in 0..4 {
        n = n.rotate_left(13) ^ (n >> 7);
        tail.push(DIGITS[(n % 36) as usize] as char);
    }
    format!("{}-{tail}", command_for(name))
}

/// Now, as the skill format writes times: "2026-09-20T10:00:00Z".
pub fn utc_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    utc(secs)
}

/// `secs` after 1970 as "YYYY-MM-DDTHH:MM:SSZ" (Howard Hinnant's days-to-civil).
fn utc(secs: i64) -> String {
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: &str = "2026-09-26T10:00:00Z";

    #[test]
    fn a_prompt_saved_from_the_chat_is_a_skill() {
        let s = Skill::saved(
            "  Night   prints ",
            "a koi pond at night ",
            Some("ukiyoe"),
            NOW,
        );
        assert_eq!(s.format, FORMAT);
        assert_eq!(s.name, "Night prints");
        assert_eq!(s.command, "night-prints");
        assert!(
            s.id.starts_with("night-prints-") && s.id.len() == "night-prints-".len() + 4,
            "{}",
            s.id
        );
        assert_eq!(s.idea.as_deref(), Some("a koi pond at night"));
        let mut checked = s.clone();
        assert_eq!(check(&mut checked), Ok(vec![]));
        assert_eq!(
            checked.base_style.as_deref(),
            Some("woodblock"),
            "the style's id today"
        );
        assert!(!checked.has_own_look());
        // Round trips through JSON with only what it has.
        let json = serde_json::to_string(&checked).unwrap();
        assert!(
            !json.contains("palette") && !json.contains("treatment"),
            "{json}"
        );
        assert_eq!(serde_json::from_str::<Skill>(&json).unwrap(), checked);
        assert_ne!(
            Skill::saved("Night prints", "x", None, NOW).id,
            s.id,
            "ids differ"
        );
    }

    #[test]
    fn a_skills_look_is_its_treatment_or_its_styles_with_its_light_and_palette() {
        let mut s = Skill::saved("Night prints", "a koi pond", Some("woodblock"), NOW);
        let plain = s.treatment_for("openai").unwrap();
        let woodblock = styles::style("woodblock").unwrap();
        assert_eq!(plain.words, woodblock.fragment);
        assert_eq!(plain.id, s.id);
        assert_eq!(
            plain.lettering.as_deref(),
            Some(woodblock.lettering.as_str())
        );
        s.treatment = Some("a traditional woodblock-printed illustration: carved black keylines, flat areas of deep indigo and old gold".into());
        s.light = Some("cool moonlight from the upper left.".into());
        s.palette = vec![
            Swatch {
                hex: "#1D2B53".into(),
                name: "deep indigo".into(),
            },
            Swatch {
                hex: "#C9A227".into(),
                name: String::new(),
            },
        ];
        s.keep_out = vec!["Mount Fuji".into(), "koi ponds".into()];
        s.lettering = Some(SkillLettering {
            look: Some("carved capitals".into()),
            ..Default::default()
        });
        s.providers = serde_json::from_str(r#"{"local": {"treatment": "a woodblock print: black keylines"}, "stability": {"style_preset": "line-art"}}"#).unwrap();
        let t = s.treatment_for("openai").unwrap();
        assert_eq!(t.words, "a traditional woodblock-printed illustration: carved black keylines, flat areas of deep indigo and old gold, cool moonlight from the upper left, in a palette of deep indigo and gold");
        assert_eq!(t.lettering.as_deref(), Some("carved capitals"));
        assert!(t.keep_out.contains(&"koi ponds".to_string()));
        assert_eq!(
            t.keep_out.iter().filter(|k| *k == "Mount Fuji").count(),
            1,
            "no doubles"
        );
        assert_eq!(t.native.stability.as_deref(), Some("line-art"));
        assert_eq!(
            t.native.ideogram_preset.as_deref(),
            Some("WOODBLOCK_PRINT"),
            "the style's own"
        );
        assert!(s
            .treatment_for("local")
            .unwrap()
            .words
            .starts_with("a woodblock print: black keylines,"));
        assert!(s.has_own_look());
        // Words and no look: nothing to render it in.
        assert_eq!(
            Skill::saved("Koi", "a koi pond", None, NOW).treatment_for("openai"),
            None
        );
    }

    #[test]
    fn an_idea_template_wraps_what_is_typed() {
        let mut s = Skill::saved("Night", "", None, NOW);
        s.idea_template = Some("{idea}, at night under a full moon".into());
        assert_eq!(s.idea_for(" a fox "), "a fox, at night under a full moon");
        s.idea_template = None;
        assert_eq!(s.idea_for("a fox"), "a fox");
    }

    #[test]
    fn the_rules_refuse_what_would_make_a_bad_skill() {
        let base = || Skill::saved("Night prints", "a koi pond", None, NOW);
        let refuse = |mut s: Skill, says: &str| {
            let err = check(&mut s).unwrap_err();
            assert!(err.contains(says), "{err:?} should say {says:?}");
        };
        refuse(
            Skill {
                name: "  ".into(),
                ..base()
            },
            "name",
        );
        refuse(
            Skill {
                name: "n".repeat(MAX_NAME_CHARS + 1),
                ..base()
            },
            "too long",
        );
        refuse(
            Skill {
                idea: None,
                ..base()
            },
            "nothing to save",
        );
        refuse(
            Skill {
                base_style: Some("nope".into()),
                ..base()
            },
            "no style called",
        );
        refuse(
            Skill {
                treatment: Some("a woodblock print".into()),
                ..base()
            },
            "8 to 60 words",
        );
        refuse(
            Skill {
                treatment: Some(
                    "a woodblock print: carved keylines, flat indigo, no people anywhere at all"
                        .into(),
                ),
                ..base()
            },
            "\u{201C}no\u{201D}",
        );
        refuse(
            Skill {
                idea_template: Some("{idea} and {idea}".into()),
                ..base()
            },
            "exactly once",
        );
        refuse(
            Skill {
                palette: vec![Swatch {
                    hex: "blue".into(),
                    name: String::new(),
                }],
                ..base()
            },
            "isn't a colour",
        );
        refuse(
            Skill {
                palette: vec![
                    Swatch {
                        hex: "#000000".into(),
                        name: "x".into()
                    };
                    7
                ],
                ..base()
            },
            "at most 6",
        );
        refuse(
            Skill {
                references: vec![SkillReference {
                    role: "mood".into(),
                    file: "a.png".into(),
                    ..Default::default()
                }],
                ..base()
            },
            "subject, a style or a palette",
        );
        refuse(
            Skill {
                command: "Night Prints".into(),
                ..base()
            },
            "a to z",
        );
        refuse(
            Skill {
                idea: Some("w ".repeat(3000)),
                ..base()
            },
            "too long to keep",
        );
    }

    #[test]
    fn the_rules_tidy_what_they_can_and_note_a_named_style() {
        let mut s = Skill::saved(
            "Koi",
            "a (koi:1.3) pond --sref 123 at night:: [[moody]]",
            None,
            NOW,
        );
        s.command.clear();
        s.palette = vec![Swatch {
            hex: " #c9a227 ".into(),
            name: String::new(),
        }];
        s.keep_out = vec!["  Mount   Fuji ".into(), " ".into()];
        assert_eq!(check(&mut s), Ok(vec![]));
        assert_eq!(s.idea.as_deref(), Some("a koi pond at night moody"));
        assert_eq!(s.command, "koi");
        assert_eq!(
            s.palette[0],
            Swatch {
                hex: "#C9A227".into(),
                name: "gold".into()
            }
        );
        assert_eq!(s.keep_out, ["Mount Fuji"]);
        let mut named = Skill::saved(
            "Ghibli",
            "a forest spirit in the style of Studio Ghibli",
            None,
            NOW,
        );
        assert_eq!(
            check(&mut named),
            Ok(vec![Warning::NamesSomeone("Studio Ghibli".into())])
        );
        assert_eq!(
            names_someone("a portrait by Van Gogh."),
            Some("Van Gogh".into())
        );
        assert_eq!(names_someone("lit by candlelight"), None);
        assert_eq!(
            names_someone("a painting in the style of the old masters"),
            None
        );
    }

    #[test]
    fn tool_syntax_and_colours_are_read_the_way_the_rules_need() {
        assert_eq!(
            strip_tool_syntax("(a cat:1.2) --ar 16:9 on a mat"),
            "a cat on a mat"
        );
        assert_eq!(
            strip_tool_syntax("a (small) cat"),
            "a (small) cat",
            "brackets that aren't weights stay"
        );
        assert_eq!(strip_tool_syntax("cat::2 dog"), "cat dog");
        assert_eq!(strip_tool_syntax("time: 10:30"), "time: 10:30");
        assert_eq!(name_of("#1D2B53"), "deep indigo");
        assert_eq!(name_of("#FFFFFF"), "white");
        assert_eq!(name_of("nope"), "");
        assert_eq!(command_for("Été à Paris!"), "t-paris");
        assert_eq!(command_for("!!!"), "prompt");
        assert_eq!(utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc(1_790_416_800), "2026-09-26T10:00:00Z");
        assert_eq!(utc_now().len(), NOW.len());
    }
}
