//! What goes over the wire, as services/community sends and reads it.

use serde::{Deserialize, Serialize};

/// `GET /v1/status`: whether the service is taking packs at all.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Status {
    pub accepting: bool,
    /// Why not, when it isn't.
    #[serde(default)]
    pub message: String,
    pub terms_version: u32,
    #[serde(default)]
    pub max_pictures: usize,
}

/// `GET /v1/me`: this computer as the service knows it.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Me {
    pub verified: bool,
    #[serde(default)]
    pub handle: Option<String>,
    /// probation, active, trusted or banned.
    #[serde(default)]
    pub tier: Option<String>,
    #[serde(default = "yes")]
    pub accepting: bool,
}

fn yes() -> bool {
    true
}

/// A pack without the fields the service decides itself (version, author, licence).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Manifest {
    pub name: String,
    pub tags: Vec<String>,
    pub skins: Vec<ManifestSkin>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ManifestSkin {
    pub file: String,
    pub name: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// One picture of a pack, described before it is sent.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Item {
    pub file: String,
    pub sha256: String,
    pub bytes: usize,
    pub width: u32,
    pub height: u32,
}

/// `POST /v1/submissions`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct NewSubmission {
    pub manifest: Manifest,
    pub license: String,
    /// own, ai, mixed or licensed.
    pub source: String,
    pub notes: String,
    pub terms_version: u32,
    pub items: Vec<Item>,
}

/// What `POST /v1/submissions` answers: the pictures still to send, by sha256, and how many
/// contact sheets.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Created {
    pub submission_id: String,
    pub need: Vec<String>,
    pub sheets: usize,
}

/// Why a pack was turned down or taken down: a rule in docs/PACK-TERMS.md and a sentence.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Reason {
    pub code: String,
    pub term: u32,
    pub message: String,
}

/// One of the author's packs, as `GET /v1/submissions` lists it.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Submission {
    pub id: String,
    pub name: String,
    /// uploading, in_review, approved, rejected, withdrawn, taken_down or expired.
    pub status: String,
    pub pictures: usize,
    pub license: String,
    pub created_at: u64,
    #[serde(default)]
    pub decided_at: Option<u64>,
    /// The pack's folder name once it is approved.
    #[serde(default)]
    pub pack_id: Option<String>,
    /// Whether it has been pulled into community/packs, from where only the maintainer can take
    /// it out again.
    #[serde(default)]
    pub pulled: bool,
    #[serde(default)]
    pub reasons: Vec<Reason>,
    /// The maintainer's own words, when they left some.
    #[serde(default)]
    pub note: String,
}

#[derive(Deserialize)]
pub(crate) struct Submissions {
    pub submissions: Vec<Submission>,
}

/// An approved pack waiting to be pulled into the repository (`GET /v1/admin/exports`).
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Export {
    /// The submission.
    pub id: String,
    /// The folder name the service gave it.
    pub pack_id: String,
    pub name: String,
    pub license: String,
    /// The handle its pack.json credits, as it was when the pack was approved.
    pub handle: String,
    pub files: Vec<ExportFile>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct ExportFile {
    pub file: String,
    pub sha256: String,
    pub bytes: usize,
}

#[derive(Deserialize)]
pub(crate) struct Exports {
    pub packs: Vec<Export>,
}

/// The service's errors: a code, and a sentence meant to be shown as it is.
#[derive(Deserialize, Debug)]
pub(crate) struct ErrorBody {
    pub error: ErrorDetail,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ErrorDetail {
    pub code: String,
    pub message: String,
}
