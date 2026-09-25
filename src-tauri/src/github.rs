//! Publishing a pack to the community repository without leaving the app.
//!
//! Sharing a pack used to end with "now go to GitHub, fork this, drag your folder in and open a
//! pull request". Most people stop there, so the packs never arrive. This does the same work over
//! GitHub's API: sign in once, and publishing is a button.
//!
//! **Signing in** uses the device flow. The app asks GitHub for a short code, the person types it
//! into <https://github.com/login/device>, and the app polls until they have approved it. There is
//! no client secret to ship — the device flow is built for programs that can't keep one — and the
//! token that comes back is sealed on disk beside the AI keys ([`crate::keys`]), never in the
//! system keychain. The device code itself never reaches the webview.
//!
//! **Publishing** asks GitHub whether this person can push to the community repository. A
//! maintainer gets a branch on the repository itself; everyone else gets a fork, brought up to
//! date first so the branch starts from what is on `main` today. Either way the files go up as one
//! commit through the git data API — blobs, a tree, a commit, a ref — rather than fifty separate
//! writes, and the pull request is opened against the community repository.

use crate::keys::Keys;
use crate::state::AppState;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::ipc::Channel;
use tauri::State;

/// Where packs live, and where the pull request is opened: the packs repository, which is small
/// enough that forking it costs a contributor nothing.
const OWNER: &str = "prajwal-svm";
const REPO: &str = "folderskin-community";
const API: &str = "https://api.github.com";

/// What the token is stored under, alongside the AI providers.
const PROVIDER: &str = "github";

/// Enough to fork a public repository, push to your own fork and open a pull request. Nothing
/// here needs to see a private repository, so it doesn't ask to.
const SCOPE: &str = "public_repo";

/// The OAuth app's client id, from github.com/settings/developers with "Enable Device Flow" on.
///
/// Public by design: the device flow has no client secret to keep, so this is safe to commit and
/// safe to ship in the binary. A build without one says so rather than pretending to sign in.
const CLIENT_ID: &str = "Ov23li4bp8DzaxZpGDOB";

fn client_id() -> &'static str {
    match option_env!("FOLDERSKIN_GITHUB_CLIENT_ID") {
        Some(id) if !id.is_empty() => id,
        _ => CLIENT_ID,
    }
}

const NOT_SET_UP: &str =
    "this build of FolderSkin has no GitHub app set up, so it can't sign you in";
const OFFLINE: &str = "couldn't reach GitHub — check your connection";

/// The device code, kept here rather than handed to the webview: it is what redeems the token.
#[derive(Default)]
pub struct Pending(Mutex<Option<String>>);

fn client() -> Result<&'static reqwest::Client, String> {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client);
    }
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .user_agent(concat!("FolderSkin/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    Ok(CLIENT.get_or_init(|| client))
}

// ---------- signing in ----------

/// What the webview shows while someone signs in: the code to type and where to type it.
#[derive(Serialize, Clone)]
pub struct DeviceCode {
    pub user_code: String,
    pub verification_uri: String,
    /// How long the code lasts, so the app can say when it has gone stale.
    pub expires_in: u64,
}

#[derive(Deserialize)]
struct DeviceStart {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

/// Who is signed in.
#[derive(Serialize, Clone)]
pub struct Account {
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: String,
}

/// Asks GitHub for a code to show. The answer is what the person types into the page; the half
/// that redeems it stays here.
#[tauri::command]
pub async fn github_connect(pending: State<'_, Pending>) -> Result<DeviceCode, String> {
    if client_id().is_empty() {
        return Err(NOT_SET_UP.into());
    }
    let start: DeviceStart = client()?
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form(&[("client_id", client_id()), ("scope", SCOPE)]))
        .send()
        .await
        .map_err(|_| OFFLINE.to_string())?
        .json()
        .await
        .map_err(|_| "GitHub sent something FolderSkin couldn't read".to_string())?;
    let code = DeviceCode {
        user_code: start.user_code,
        verification_uri: start.verification_uri,
        expires_in: start.expires_in,
    };
    *lock(&pending.0) = Some(format!("{}:{}", start.interval, start.device_code));
    Ok(code)
}

/// Waits for the code to be approved, then keeps the token and says who signed in. Resolves with
/// an error the moment GitHub says the person refused or the code ran out, so the app can stop
/// showing a code that no longer works.
#[tauri::command]
pub async fn github_wait(
    pending: State<'_, Pending>,
    keys: State<'_, Keys>,
) -> Result<Account, String> {
    let Some(kept) = lock(&pending.0).clone() else {
        return Err("start signing in first".into());
    };
    let (interval, device_code) = kept
        .split_once(':')
        .ok_or_else(|| "start signing in first".to_string())?;
    let mut wait = interval.parse::<u64>().unwrap_or(5).max(1);
    let device_code = device_code.to_string();

    loop {
        tokio::time::sleep(Duration::from_secs(wait)).await;
        // The person may have closed the panel; stop rather than poll on in the background.
        if lock(&pending.0).is_none() {
            return Err("signing in was stopped".into());
        }
        let answer: Value = client()?
            .post("https://github.com/login/oauth/access_token")
            .header("Accept", "application/json")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(form(&[
                ("client_id", client_id()),
                ("device_code", &device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ]))
            .send()
            .await
            .map_err(|_| OFFLINE.to_string())?
            .json()
            .await
            .map_err(|_| "GitHub sent something FolderSkin couldn't read".to_string())?;

        if let Some(token) = answer["access_token"].as_str() {
            *lock(&pending.0) = None;
            keys.set(PROVIDER, token)?;
            return account(token).await;
        }
        match answer["error"].as_str().unwrap_or("") {
            // Not yet: keep waiting at the pace GitHub asked for.
            "authorization_pending" => {}
            "slow_down" => wait += 5,
            "expired_token" => {
                *lock(&pending.0) = None;
                return Err("that code ran out — ask for a new one".into());
            }
            "access_denied" => {
                *lock(&pending.0) = None;
                return Err("you turned that request down".into());
            }
            other => {
                *lock(&pending.0) = None;
                return Err(format!("GitHub wouldn't sign you in ({other})"));
            }
        }
    }
}

/// Stops waiting for a code that is on screen.
#[tauri::command]
pub fn github_cancel(pending: State<'_, Pending>) {
    *lock(&pending.0) = None;
}

/// Who is signed in, or `None`. A token GitHub no longer accepts is forgotten rather than kept
/// around to fail later.
#[tauri::command]
pub async fn github_account(keys: State<'_, Keys>) -> Result<Option<Account>, String> {
    let Some(token) = keys.get(PROVIDER) else {
        return Ok(None);
    };
    match account(&token).await {
        Ok(account) => Ok(Some(account)),
        Err(_) => {
            let _ = keys.clear(PROVIDER);
            Ok(None)
        }
    }
}

/// Forgets the token. GitHub still lists the app under the person's authorised apps until they
/// revoke it there, which the app says.
#[tauri::command(async)]
pub fn github_sign_out(keys: State<'_, Keys>) -> Result<(), String> {
    keys.clear(PROVIDER)
}

async fn account(token: &str) -> Result<Account, String> {
    let user = api(token, reqwest::Method::GET, &format!("{API}/user"), None).await?;
    Ok(Account {
        login: user["login"]
            .as_str()
            .ok_or_else(|| "GitHub didn't say who you are".to_string())?
            .to_string(),
        name: user["name"].as_str().map(str::to_string),
        avatar_url: user["avatar_url"].as_str().unwrap_or_default().to_string(),
    })
}

// ---------- publishing ----------

/// How far publishing has got, for the panel to show.
#[derive(Serialize, Clone)]
#[serde(tag = "stage", rename_all = "camelCase")]
pub enum Progress {
    /// Working out where the pack can go.
    Checking,
    /// Making the person's own copy of the repository, and waiting for GitHub to finish it.
    Forking,
    /// Starting a branch for the pack.
    Branching,
    /// Sending the pictures, one at a time.
    Uploading { done: usize, total: usize },
    /// Opening the pull request.
    Opening,
}

/// The pull request that was opened.
#[derive(Serialize, Clone)]
pub struct Published {
    pub url: String,
    pub number: u64,
    /// True when it went through a fork, which is what most people will see.
    pub forked: bool,
}

/// A pack to publish, and what the person answered about it in the app.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackRequest {
    pub name: String,
    pub license: String,
    pub tags: Vec<String>,
    pub skin_ids: Vec<String>,
    /// Where the pictures came from, in their words. It goes in the pull request body, where a
    /// maintainer would otherwise have to ask for it.
    pub notes: String,
    /// Which version of the terms they were shown and agreed to, recorded in the pull request so
    /// it is never a question of what happened to be on main that day.
    pub terms_version: u32,
}

/// Puts a pack up as a pull request against the community repository.
#[tauri::command]
pub async fn publish_pack(
    state: State<'_, AppState>,
    keys: State<'_, Keys>,
    pack: PackRequest,
    on_progress: Channel<Progress>,
) -> Result<Published, String> {
    let PackRequest {
        name,
        license,
        tags,
        skin_ids,
        notes,
        terms_version,
    } = pack;

    let token = keys
        .get(PROVIDER)
        .ok_or_else(|| "connect to GitHub first".to_string())?;
    let app = state.inner().clone();

    // Who GitHub says this is. The pack is credited to the account that publishes it, so there is
    // nothing to type, nothing to get wrong, and nobody else's name to put on it.
    let _ = on_progress.send(Progress::Checking);
    let me = account(&token).await?;

    // The pack is built and checked before anything is sent, so a pack that would fail the
    // pull-request checks never becomes a branch someone has to clean up.
    let (id, files) = {
        let (name, author, license) = (name.clone(), me.login.clone(), license.clone());
        tauri::async_runtime::spawn_blocking(move || {
            crate::community::build_pack(&app, &name, &author, &license, &tags, &skin_ids)
        })
        .await
        .map_err(|e| e.to_string())??
    };

    let repo = api(
        &token,
        reqwest::Method::GET,
        &format!("{API}/repos/{OWNER}/{REPO}"),
        None,
    )
    .await?;
    let base_branch = repo["default_branch"]
        .as_str()
        .unwrap_or("main")
        .to_string();
    let can_push = repo["permissions"]["push"].as_bool().unwrap_or(false);

    // A maintainer works on the repository itself; everyone else on their own copy of it.
    let head_owner = if can_push {
        OWNER.to_string()
    } else {
        let _ = on_progress.send(Progress::Forking);
        fork(&token, &me.login).await?;
        me.login.clone()
    };

    let _ = on_progress.send(Progress::Branching);
    // A fork made a while ago is behind, and a branch off it would ask to merge old commits too.
    if !can_push {
        let _ = api(
            &token,
            reqwest::Method::POST,
            &format!("{API}/repos/{head_owner}/{REPO}/merge-upstream"),
            Some(json!({ "branch": base_branch })),
        )
        .await;
    }
    let base_sha = api(
        &token,
        reqwest::Method::GET,
        &format!("{API}/repos/{head_owner}/{REPO}/git/ref/heads/{base_branch}"),
        None,
    )
    .await?["object"]["sha"]
        .as_str()
        .ok_or_else(|| "GitHub didn't say where the branch starts".to_string())?
        .to_string();

    let branch = format!("pack/{id}-{}", &base_sha[..7]);
    let total = files.len();
    let mut tree = Vec::with_capacity(total);
    for (n, (file, bytes)) in files.iter().enumerate() {
        let _ = on_progress.send(Progress::Uploading { done: n, total });
        let blob = api(
            &token,
            reqwest::Method::POST,
            &format!("{API}/repos/{head_owner}/{REPO}/git/blobs"),
            Some(json!({ "content": B64.encode(bytes), "encoding": "base64" })),
        )
        .await?;
        tree.push(json!({
            "path": format!("packs/{id}/{file}"),
            "mode": "100644",
            "type": "blob",
            "sha": blob["sha"],
        }));
    }
    let _ = on_progress.send(Progress::Uploading { done: total, total });

    let tree = api(
        &token,
        reqwest::Method::POST,
        &format!("{API}/repos/{head_owner}/{REPO}/git/trees"),
        Some(json!({ "base_tree": base_sha, "tree": tree })),
    )
    .await?;
    let commit = api(
        &token,
        reqwest::Method::POST,
        &format!("{API}/repos/{head_owner}/{REPO}/git/commits"),
        Some(json!({
            "message": format!("Add the {name} pack"),
            "tree": tree["sha"],
            "parents": [base_sha],
        })),
    )
    .await?;
    api(
        &token,
        reqwest::Method::POST,
        &format!("{API}/repos/{head_owner}/{REPO}/git/refs"),
        Some(json!({ "ref": format!("refs/heads/{branch}"), "sha": commit["sha"] })),
    )
    .await?;

    let _ = on_progress.send(Progress::Opening);
    // A fork's branch is named for its owner; a branch on the repository itself is not.
    let head = if can_push {
        branch.clone()
    } else {
        format!("{}:{}", me.login, branch)
    };
    let pull = api(
        &token,
        reqwest::Method::POST,
        &format!("{API}/repos/{OWNER}/{REPO}/pulls"),
        Some(json!({
            "title": format!("Add the {} pack", name.trim()),
            "head": head,
            "base": base_branch,
            "body": body(&name, &me.login, &license, total - 1, &notes, terms_version),
            "maintainer_can_modify": true,
        })),
    )
    .await?;
    Ok(Published {
        url: pull["html_url"].as_str().unwrap_or_default().to_string(),
        number: pull["number"].as_u64().unwrap_or(0),
        forked: !can_push,
    })
}

/// Makes the person's copy of the repository and waits for GitHub to finish it. A fork they
/// already have comes back at once.
async fn fork(token: &str, login: &str) -> Result<(), String> {
    api(
        token,
        reqwest::Method::POST,
        &format!("{API}/repos/{OWNER}/{REPO}/forks"),
        Some(json!({})),
    )
    .await?;
    // GitHub answers before the copy exists. It is usually a second or two, rarely longer.
    for _ in 0..30 {
        if api(
            token,
            reqwest::Method::GET,
            &format!("{API}/repos/{login}/{REPO}"),
            None,
        )
        .await
        .is_ok()
        {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    Err("GitHub is still making your copy of the repository — try again in a moment".into())
}

/// The pull request body: what a maintainer needs to see without asking for it.
fn body(name: &str, author: &str, license: &str, skins: usize, notes: &str, terms: u32) -> String {
    let notes = notes.trim();
    format!(
        "Adds the **{name}** pack: {skins} skins, by @{author}, under {license}.\n\n\
         Opened from FolderSkin, which checked the pack against the contract in \
         [the pack guide](https://folderskin.app/docs/packs/) before sending \
         it: picture sizes and formats, file and folder names, tag shapes, the manifest and the \
         size limits all pass.\n\n\
         **Where the pictures came from**\n\n{}\n\n\
         @{author} agreed to [the pack terms](https://folderskin.app/docs/pack-terms/), \
         version {terms}, in the app before this was opened.\n",
        if notes.is_empty() {
            "_Not said._".to_string()
        } else {
            notes.to_string()
        }
    )
}

/// One GitHub API call. Errors come back as a sentence, with GitHub's own message when it gave
/// one, since those are usually the useful part ("name already exists on this account").
async fn api(
    token: &str,
    method: reqwest::Method,
    url: &str,
    body: Option<Value>,
) -> Result<Value, String> {
    let mut request = client()?
        .request(method, url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .bearer_auth(token);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request.send().await.map_err(|_| OFFLINE.to_string())?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    let value: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if status.is_success() {
        return Ok(value);
    }
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err("GitHub no longer accepts that sign-in — connect again".into());
    }
    let said = value["message"].as_str().unwrap_or("");
    let detail = value["errors"][0]["message"].as_str().unwrap_or("");
    Err(match (said.is_empty(), detail.is_empty()) {
        (false, false) => format!("GitHub said: {said} ({detail})"),
        (false, true) => format!("GitHub said: {said}"),
        _ => format!("GitHub answered {status}"),
    })
}

/// A form body. reqwest is built here without its own form support, and these are the only two
/// requests that need one.
fn form(fields: &[(&str, &str)]) -> String {
    fields
        .iter()
        .map(|(k, v)| format!("{}={}", escape(k), escape(v)))
        .collect::<Vec<_>>()
        .join("&")
}

/// Percent-encodes everything that isn't unreserved, which is more than these values need but
/// leaves nothing to think about if one ever changes shape.
fn escape(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_body_says_what_a_maintainer_would_otherwise_ask() {
        let text = body(
            "Greek Art",
            "someone",
            "CC0-1.0",
            12,
            "  All mine, drawn in Procreate. ",
            1,
        );
        assert!(text.contains("**Greek Art**"), "{text}");
        assert!(text.contains("12 skins"), "{text}");
        assert!(text.contains("@someone"), "{text}");
        assert!(text.contains("CC0-1.0"), "{text}");
        assert!(text.contains("All mine, drawn in Procreate."), "{text}");
        assert!(!text.contains("Not said"), "{text}");
        assert!(
            text.contains("version 1"),
            "what they agreed to is on the record: {text}"
        );
        // The pull request goes to the packs repository; the contract and the terms are on
        // FolderSkin's website.
        assert!(
            text.contains("https://folderskin.app/docs/packs/"),
            "{text}"
        );
        assert!(
            text.contains("https://folderskin.app/docs/pack-terms/"),
            "{text}"
        );
    }

    #[test]
    fn an_unanswered_question_says_so_rather_than_leaving_a_gap() {
        let text = body("Dunes", "someone", "CC0-1.0", 3, "   ", 1);
        assert!(text.contains("_Not said._"), "{text}");
    }

    #[test]
    fn a_form_body_escapes_what_it_carries() {
        assert_eq!(
            form(&[("grant_type", "urn:ietf:params:oauth:grant-type:device_code")]),
            "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code"
        );
        assert_eq!(form(&[("a", "b"), ("c", "d")]), "a=b&c=d");
    }

    #[test]
    fn a_build_with_no_app_set_up_says_so_instead_of_pretending() {
        // The shipped default is empty until an OAuth app's client id is pasted in.
        assert!(CLIENT_ID.is_empty() || CLIENT_ID.starts_with("Ov"));
    }
}
