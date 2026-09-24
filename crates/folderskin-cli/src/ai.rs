//! `folderskin ai …`: set the computer up, paint, and choose the defaults.

use crate::cli::{
    AiCommand, BatchArgs, ConfigCommand, ConfigKey, GenArgs, KeyCommand, MachineArgs, RuntimeArg,
    SetupArgs, ThemeArgs,
};
use crate::config::{self, Config, LOCAL};
use crate::error::CliError;
use crate::out::Out;
use crate::paint::{self, Order, Painted, Painter};
use crate::{preview, runtime, terminal};
use folderskin_local::{
    detect, random_seed, slug, CancelToken, ModelId, Runtime, Shape, MODELS, STYLES,
};
use serde::Deserialize;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub fn run(command: AiCommand, out: &Arc<Out>) -> Result<(), CliError> {
    match command {
        AiCommand::Doctor(args) => doctor(&args, out),
        AiCommand::Setup(args) => setup(&args, out),
        AiCommand::Gen(args) => gen(args, out),
        AiCommand::Batch(args) => batch(&args, out),
        AiCommand::Theme(args) => theme(&args, out),
        AiCommand::Styles => styles(out),
        AiCommand::Models => models(out),
        AiCommand::Config { command } => config(command, out),
        AiCommand::Key(command) => key(command, out),
    }
}

// ---------- the computer ----------

fn doctor(args: &MachineArgs, out: &Arc<Out>) -> Result<(), CliError> {
    let machine = detect();
    let config = Config::load()?;
    let (settings, backend_from, tier_from) = paint::settings(&machine, args, &config)?;
    let status = folderskin_local::status(&machine, &settings);
    let keys = config::keys();

    let mut lines = vec![
        format!("machine:  {}", machine.describe()),
        format!(
            "backend:  {} ({})   tier: {} ({})",
            settings.backend,
            backend_from.label(),
            settings.tier,
            tier_from.label()
        ),
        format!("home:     {}", status.home.display()),
    ];
    let runtime = &status.runtime;
    let mut next = Vec::new();
    match (&runtime.path, runtime.installed) {
        (Some(path), true) => lines.push(format!(
            "runtime:  {} {}",
            path.display(),
            runtime
                .release
                .as_deref()
                .map(|r| format!("({r})"))
                .unwrap_or_default()
        )),
        (None, true) => lines.push(format!("runtime:  {} is installed", runtime.name)),
        _ if !runtime.available => {
            // Nothing setup could install would run here: say so now, not after a download.
            let why = folderskin_local::setup::no_build(machine.os, machine.arch, settings.backend);
            lines.push(format!("runtime:  none to install: {}", why.what));
            lines.push(format!("          {}", why.why));
            next.extend(why.fix);
        }
        _ => {
            lines.push(format!("runtime:  {} is not installed", runtime.name));
            next.push(format!(
                "folderskin ai setup --backend {} --tier {}",
                settings.backend, settings.tier
            ));
        }
    }
    if let Some(problem) = &runtime.problem {
        lines.push(format!("problem:  {} won't start: {problem}", runtime.name));
    }
    if !runtime.devices.is_empty() {
        lines.push(format!(
            "devices:  {}",
            runtime.devices.join("\n          ").replace('\t', "  ")
        ));
    }
    for model in &status.models {
        let have = model.files.iter().filter(|f| f.present).count();
        lines.push(format!(
            "model:    {}: {have}/{} files ({:.1} GB with shared parts)",
            model.label,
            model.files.len(),
            model.bytes as f64 / 1e9
        ));
        if !model.ready() && next.is_empty() {
            next.push(format!(
                "folderskin ai setup --backend {} --tier {}",
                settings.backend, settings.tier
            ));
        }
    }
    lines.push(format!(
        "cwebp:    {}",
        status.cwebp.as_ref().map_or(
            "not found (setup installs it on Windows; packs need it)".into(),
            |p| p.display().to_string()
        )
    ));
    lines.push(format!(
        "settings: {}",
        config::config_file().map_or("nowhere to keep them".into(), |p| p.display().to_string())
    ));
    let with_keys: Vec<String> = folderskin_ai::providers()
        .iter()
        .filter_map(|p| {
            config::key_for(p.id, &keys)
                .map(|_| format!("{} ({})", p.id, paint::key_state(p, &keys)))
        })
        .collect();
    lines.push(format!(
        "keys:     {}",
        if with_keys.is_empty() {
            "none (only needed for --provider)".to_string()
        } else {
            with_keys.join(", ")
        }
    ));
    if settings.backend == folderskin_local::Backend::Cpu {
        lines.push("note:     no GPU backend; expect minutes per picture".into());
    }
    match next.split_first() {
        Some((step, more)) => {
            lines.push(format!("next:     {step}"));
            lines.extend(more.iter().map(|m| format!("          {m}")));
        }
        None => lines.push("ready:    folderskin ai gen \"a lighthouse at dusk\"".into()),
    }
    let meta = serde_json::to_value(&status).unwrap_or_default();
    out.result(None, "doctor", meta, &lines.join("\n"), false);
    Ok(())
}

fn setup(args: &SetupArgs, out: &Arc<Out>) -> Result<(), CliError> {
    let machine = detect();
    let config = Config::load()?;
    let (settings, _, _) = paint::settings(&machine, &args.machine, &config)?;
    out.note(&format!(
        "{} -> {}, {}",
        machine.describe(),
        settings.backend,
        settings.tier
    ));
    let runtime_choice = match args.runtime {
        RuntimeArg::Pinned => Runtime::Pinned,
        RuntimeArg::Latest => Runtime::Latest,
    };
    let cancel = terminal::stop_on_ctrl_c();
    runtime()?.block_on(folderskin_local::setup(
        &machine,
        &settings,
        runtime_choice,
        &out.reporter(),
        &cancel,
    ))?;
    out.finish_line();
    out.result(
        Some(&folderskin_local::home()),
        "setup",
        json!({"backend": settings.backend, "tier": settings.tier}),
        &format!(
            "Ready: {} with {} weights. Paint something: folderskin ai gen \"a lighthouse at dusk\"",
            settings.backend, settings.tier
        ),
        false,
    );
    Ok(())
}

// ---------- painting ----------

/// Paints every order into `out_dir`, previewing each unless told not to, and says where each
/// went. Stops at the first failure: what was painted before it is kept.
fn paint_all(
    painter: &Painter,
    orders: &[Order],
    out_dir: &Path,
    previews: bool,
    out: &Arc<Out>,
    cancel: &CancelToken,
) -> Result<Vec<Painted>, CliError> {
    let rt = runtime()?;
    let mut made = Vec::new();
    for order in orders {
        let mut painted = rt.block_on(painter.paint(order, out_dir, out, cancel))?;
        out.finish_line();
        if previews {
            match preview::preview(&painted.path, &out_dir.join("previews")) {
                Ok((path, what)) => {
                    out.note(&format!("preview: {} ({what})", path.display()));
                    painted.meta["preview"] = json!(path);
                }
                Err(e) => out.warn(&format!("no preview: {} {}", e.what, e.why)),
            }
        }
        out.result(
            Some(&painted.path),
            painted.shape.id(),
            painted.meta.clone(),
            &painted.path.display().to_string(),
            false,
        );
        made.push(painted);
    }
    Ok(made)
}

/// The contact sheet of every picture's preview, when there is more than one.
fn sheet(made: &[PathBuf], out_dir: &Path, out: &Arc<Out>) {
    let previews: Vec<PathBuf> = made
        .iter()
        .filter_map(|p| p.file_name())
        .map(|n| out_dir.join("previews").join(n).with_extension("png"))
        .filter(|p| p.is_file())
        .collect();
    if previews.len() < 2 {
        return;
    }
    let dest = out_dir.join("previews").join("_sheet.png");
    match preview::contact_sheet(&previews, &dest) {
        Ok(()) => out.note(&format!("contact sheet: {}", dest.display())),
        Err(e) => out.warn(&format!("no contact sheet: {} {}", e.what, e.why)),
    }
}

fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

fn read_idea(idea: &str) -> Result<String, CliError> {
    let idea = if idea == "-" {
        let mut text = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)
            .map_err(|e| CliError::io("read the idea", Path::new("standard input"), &e))?;
        text
    } else {
        idea.to_string()
    };
    let idea = idea.trim().to_string();
    if idea.is_empty() {
        return Err(CliError::fixable(
            "no_idea",
            "There is nothing to paint.",
            "The idea is empty.",
        )
        .fix("Describe the subject and the scene: folderskin ai gen \"a lighthouse on a rock at dusk\""));
    }
    Ok(idea)
}

fn gen(args: GenArgs, out: &Arc<Out>) -> Result<(), CliError> {
    let idea = read_idea(&args.idea)?;
    let config = Config::load()?;
    let painter = Painter::choose(
        args.provider.as_deref(),
        args.model.as_deref(),
        &args.machine,
        &config,
    )?;
    if args.seed.is_some() && !painter.seeded() {
        out.warn("a provider picks its own seed, so --seed is left out");
    }
    let first = args.seed.unwrap_or_else(random_seed);
    let orders: Vec<Order> = paint::seeds(first, u64::from(args.n))?
        .map(|seed| Order {
            idea: idea.clone(),
            style: args.style.clone(),
            shape: paint::shape(args.shape),
            refs: args.refs.clone(),
            seed,
            name: (args.n == 1).then(|| args.name.clone()).flatten(),
            raw: args.raw,
        })
        .collect();
    painter.check_ready(&orders[0])?;
    if let Some(folder) = &args.apply {
        if !folder.is_dir() {
            return Err(preview::apply_error(
                folder,
                folderskin_core::apply::ApplyError::NotADirectory(folder.clone()),
            ));
        }
    }
    let out_dir = absolute(&args.out);
    out.note(&format!(
        "painting {} with {}",
        count(args.n as usize, "picture"),
        painter.describe()
    ));
    let cancel = terminal::stop_on_ctrl_c();
    let made = paint_all(&painter, &orders, &out_dir, !args.no_preview, out, &cancel)?;
    let paths: Vec<PathBuf> = made.iter().map(|p| p.path.clone()).collect();
    if !args.no_preview {
        sheet(&paths, &out_dir, out);
    }
    if let (Some(folder), Some(first)) = (&args.apply, paths.first()) {
        let what = preview::apply(folder, first, (0.5, 0.5))?;
        out.result(
            Some(folder),
            "applied",
            json!({"folder": folder, "picture": first}),
            &format!(
                "applied {} to {}: {what}",
                first.display(),
                folder.display()
            ),
            false,
        );
    }
    Ok(())
}

/// One entry of a batch file.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Brief {
    pub idea: String,
    #[serde(default)]
    pub style: Option<String>,
    #[serde(default)]
    pub shape: Option<String>,
    #[serde(default)]
    pub refs: Vec<String>,
    #[serde(default)]
    pub n: Option<u32>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub model: Option<String>,
}

/// Reads and checks a batch file; reference paths are made relative to it.
pub fn read_briefs(path: &Path) -> Result<Vec<Brief>, CliError> {
    let bytes = std::fs::read(path).map_err(|e| CliError::io("read the briefs", path, &e))?;
    let mut briefs: Vec<Brief> = serde_json::from_slice(&bytes).map_err(|e| {
        CliError::fixable(
            "bad_briefs",
            "The briefs file isn't right.",
            format!("{}: {e}.", path.display()),
        )
        .fix(
            "It is a JSON list like [{\"idea\": \"a lighthouse\", \"style\": \"woodblock\", \"n\": 2}]; \
             each brief can have idea, style, shape, refs, n, name, seed and model.",
        )
    })?;
    let base = path.parent().unwrap_or(Path::new("."));
    for (i, brief) in briefs.iter_mut().enumerate() {
        let which = format!("Brief {} ({:?})", i + 1, brief.idea);
        if brief.idea.trim().is_empty() {
            return Err(CliError::fixable(
                "bad_briefs",
                format!("Brief {} has no idea.", i + 1),
                "Every brief needs an idea to paint.",
            ));
        }
        if let Some(shape) = &brief.shape {
            if Shape::parse(shape).is_none() {
                return Err(CliError::fixable(
                    "bad_briefs",
                    format!("{which} asks for the shape {shape:?}."),
                    "A shape is artwork or folder.",
                ));
            }
        }
        if brief.n == Some(0) {
            return Err(CliError::fixable(
                "bad_briefs",
                format!("{which} asks for no pictures."),
                "n is how many to paint, at least 1.",
            ));
        }
        if let Some(seed) = brief.seed {
            if let Err(e) = paint::seeds(seed, u64::from(brief.n.unwrap_or(1))) {
                return Err(CliError::fixable(
                    "bad_briefs",
                    format!("{which} starts from too high a seed."),
                    e.why,
                )
                .fix("Give it a smaller seed, or leave seed out for a random one."));
            }
        }
        for r in &mut brief.refs {
            let full = base.join(&*r);
            if !full.is_file() {
                return Err(CliError::fixable(
                    "reference_missing",
                    format!("{which} names a reference picture that isn't there."),
                    format!("{} doesn't exist.", full.display()),
                )
                .fix("Reference paths are relative to the briefs file."));
            }
            *r = full.to_string_lossy().into_owned();
        }
    }
    Ok(briefs)
}

fn batch(args: &BatchArgs, out: &Arc<Out>) -> Result<(), CliError> {
    let briefs = read_briefs(&args.briefs)?;
    let config = Config::load()?;
    let painter = Painter::choose(args.provider.as_deref(), None, &args.machine, &config)?;
    let mut work: Vec<(Painter, Order)> = Vec::new();
    for brief in &briefs {
        let painter = match &brief.model {
            Some(m) => painter.with_model(m)?,
            None => painter.clone(),
        };
        let n = brief.n.unwrap_or(1);
        let first = brief.seed.unwrap_or_else(random_seed);
        for (i, seed) in paint::seeds(first, u64::from(n))?.enumerate() {
            let name = match (&brief.name, n) {
                (Some(name), 1) => Some(name.clone()),
                (Some(name), _) => Some(format!("{name}-{}", i + 1)),
                (None, _) => None,
            };
            let order = Order {
                idea: brief.idea.clone(),
                style: brief.style.clone().unwrap_or_else(|| args.style.clone()),
                shape: brief
                    .shape
                    .as_deref()
                    .and_then(Shape::parse)
                    .unwrap_or_default(),
                refs: brief.refs.iter().map(PathBuf::from).collect(),
                seed,
                name,
                raw: false,
            };
            painter.check_ready(&order)?;
            work.push((painter.clone(), order));
        }
    }
    let out_dir = absolute(&args.out);
    out.note(&format!(
        "{} from {} with {}",
        count(work.len(), "picture"),
        count(briefs.len(), "brief"),
        painter.describe()
    ));
    let cancel = terminal::stop_on_ctrl_c();
    let mut made = Vec::new();
    for (painter, order) in &work {
        made.extend(paint_all(
            painter,
            std::slice::from_ref(order),
            &out_dir,
            true,
            out,
            &cancel,
        )?);
    }
    let paths: Vec<PathBuf> = made.iter().map(|p| p.path.clone()).collect();
    sheet(&paths, &out_dir, out);
    out.note(&format!(
        "{} in {}",
        count(made.len(), "picture"),
        out_dir.display()
    ));
    Ok(())
}

/// "1 picture", "3 pictures".
fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

/// Folders to theme, `depth` levels down, leaving out hidden, system and tool folders.
pub fn folders_under(root: &Path, depth: u32) -> Result<Vec<PathBuf>, CliError> {
    const SKIP: [&str; 5] = ["node_modules", "target", "__pycache__", "venv", ".git"];
    let mut found = Vec::new();
    let mut level = vec![root.to_path_buf()];
    for _ in 0..depth {
        let mut next = Vec::new();
        for parent in &level {
            let entries = std::fs::read_dir(parent)
                .map_err(|e| CliError::io("list the folders", parent, &e))?;
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let is_dir = entry.file_type().is_ok_and(|t| t.is_dir());
                if is_dir
                    && !name.starts_with('.')
                    && !name.starts_with('$')
                    && !SKIP.contains(&name.as_str())
                {
                    next.push(entry.path());
                }
            }
        }
        next.sort();
        found.extend(next.iter().cloned());
        level = next;
    }
    Ok(found)
}

/// File names for `folders` under `root`: the folder's path in words (in any script), then a
/// fingerprint of the path itself. A run that finds a picture already painted keeps it, and
/// `--apply` puts it on the folder of that name, so a name must belong to its folder alone: told
/// apart by order instead, "Фото" and "照片" (or "A b" and "a-b") would swap pictures as soon as a
/// folder was added beside them.
pub fn theme_names(root: &Path, folders: &[PathBuf]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    folders
        .iter()
        .map(|folder| {
            let relative = folder.strip_prefix(root).unwrap_or(folder);
            // The same on every system: `/` between the parts.
            let path = relative
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            let hash = fingerprint(&path);
            let name = format!("{}-{:08x}", words(&path, 48), hash >> 32);
            if seen.insert(name.clone()) {
                name
            } else {
                // Two paths sharing both parts: all 64 bits of the fingerprint tell them apart.
                format!("{}-{hash:016x}", words(&path, 48))
            }
        })
        .collect()
}

/// Lower-case letters and digits of any script, joined by dashes, at most `limit` of them:
/// "Photos/Summer '25" → "photos-summer-25".
fn words(text: &str, limit: usize) -> String {
    let mut out = String::new();
    for c in text.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let cut: String = out.trim_matches('-').chars().take(limit).collect();
    match cut.trim_end_matches('-') {
        "" => "folder".to_string(),
        cut => cut.to_string(),
    }
}

/// 64-bit FNV-1a: small, and the same in every build, unlike the standard library's hasher.
fn fingerprint(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn theme(args: &ThemeArgs, out: &Arc<Out>) -> Result<(), CliError> {
    let root = absolute(&args.root);
    if !root.is_dir() {
        return Err(CliError::fixable(
            "not_a_folder",
            "There are no folders to paint there.",
            format!("{} isn't a folder.", root.display()),
        )
        .fix("Give the folder whose folders should be painted, e.g. folderskin ai theme \"D:/Projects\""));
    }
    let config = Config::load()?;
    let painter = Painter::choose(args.provider.as_deref(), None, &args.machine, &config)?;
    let painter = match (&args.model, &painter) {
        (Some(m), _) => painter.with_model(m)?,
        // A drive has many folders: klein is twice as fast as Z-Image and as good for this.
        (None, Painter::Local { .. }) => painter.with_model(ModelId::Klein.id())?,
        (None, Painter::Byok { .. }) => painter,
    };
    let out_dir = absolute(&args.out.clone().unwrap_or_else(|| {
        PathBuf::from("folderskin-out").join(format!(
            "theme-{}",
            words(&root.file_name().unwrap_or_default().to_string_lossy(), 40)
        ))
    }));
    let seed = args.seed.unwrap_or_else(random_seed);
    let folders = folders_under(&root, args.depth)?;
    let names = theme_names(&root, &folders);
    let seeds = paint::seeds(seed, folders.len() as u64)?;
    if folders.is_empty() {
        return Err(CliError::fixable(
            "no_folders",
            "There are no folders to paint there.",
            format!(
                "{} has no folders {} down, leaving out hidden, system and tool folders.",
                root.display(),
                count(args.depth as usize, "level")
            ),
        )
        .fix("Point it at the folder that holds the folders, or go deeper with --depth 2."));
    }
    out.note(&format!(
        "{} under {}, style {:?}, seed {seed}, with {}",
        count(folders.len(), "folder"),
        root.display(),
        args.style,
        painter.describe()
    ));
    let cancel = terminal::stop_on_ctrl_c();
    let mut first_check = true;
    for ((folder, name), seed) in folders.iter().zip(&names).zip(seeds) {
        let picture = out_dir.join(format!("{name}.png"));
        if !picture.is_file() {
            let order = Order {
                idea: folderskin_local::prompts::theme_idea(
                    &folder.file_name().unwrap_or_default().to_string_lossy(),
                ),
                style: args.style.clone(),
                shape: paint::shape(args.shape),
                refs: Vec::new(),
                seed,
                name: Some(name.clone()),
                raw: false,
            };
            if first_check {
                painter.check_ready(&order)?;
                first_check = false;
            }
            paint_all(
                &painter,
                std::slice::from_ref(&order),
                &out_dir,
                true,
                out,
                &cancel,
            )?;
        } else {
            out.note(&format!("{name}: already painted, kept"));
        }
        if args.apply {
            cancel_check(&cancel)?;
            match preview::apply(folder, &picture, (0.5, 0.5)) {
                Ok(what) => out.result(
                    Some(folder),
                    "applied",
                    json!({"folder": folder, "picture": picture}),
                    &format!(
                        "applied {} to {}: {what}",
                        picture.display(),
                        folder.display()
                    ),
                    false,
                ),
                // One folder that won't take an icon shouldn't stop a whole drive.
                Err(e) => out.warn(&format!("{}: {} {}", folder.display(), e.what, e.why)),
            }
        }
    }
    let previews: Vec<PathBuf> = names
        .iter()
        .map(|n| out_dir.join(format!("{n}.png")))
        .collect();
    sheet(&previews, &out_dir, out);
    if !args.apply {
        out.note("nothing applied; look at the sheet, then run again with --apply (undo one with folderskin revert <folder>)");
    }
    Ok(())
}

fn cancel_check(cancel: &CancelToken) -> Result<(), CliError> {
    cancel.check().map_err(CliError::from)
}

// ---------- what is on offer ----------

fn styles(out: &Arc<Out>) -> Result<(), CliError> {
    let human = STYLES
        .iter()
        .map(|s| format!("{:14} {}", s.key, s.text))
        .collect::<Vec<_>>()
        .join("\n");
    out.result(None, "styles", json!(STYLES), &human, false);
    Ok(())
}

fn models(out: &Arc<Out>) -> Result<(), CliError> {
    let machine = detect();
    let config = Config::load()?;
    let (settings, _, _) = paint::settings(&machine, &MachineArgs::default(), &config)?;
    let keys = config::keys();
    let mut lines = vec![format!(
        "On this computer, no key needed ({}, {}):",
        settings.backend, settings.tier
    )];
    let mut local = Vec::new();
    for model in &MODELS {
        let files = model.files(settings.tier).all();
        let ready = files.iter().all(|f| f.local().is_file());
        let what = if model.takes_pictures {
            "text or pictures to picture"
        } else {
            "text to picture"
        };
        lines.push(format!(
            "  {:8} {:18} {}  {what}, {} steps  {}",
            model.id.id(),
            model.label,
            model.licence,
            model.steps,
            if ready {
                "downloaded"
            } else {
                "not downloaded"
            }
        ));
        local.push(
            json!({"id": model.id, "label": model.label, "licence": model.licence,
            "takes_pictures": model.takes_pictures, "steps": model.steps, "downloaded": ready}),
        );
    }
    lines.push(String::new());
    lines.push("With your own key (--provider):".into());
    let mut providers = Vec::new();
    for p in folderskin_ai::providers() {
        let state = paint::key_state(p, &keys);
        lines.push(format!("  {:10} {:22} {state}", p.id, p.label));
        for m in p.models {
            let mut can = Vec::new();
            if m.accepts_reference {
                can.push("takes a picture");
            }
            if m.native_alpha {
                can.push("transparency");
            }
            lines.push(format!(
                "      {:28} {:26} {:18} {}",
                m.id,
                m.label,
                m.price_hint,
                can.join(", ")
            ));
        }
        providers.push(
            json!({"id": p.id, "label": p.label, "key": state, "keys_url": p.keys_url,
            "models": p.models.iter().map(|m| json!({"id": m.id, "label": m.label,
                "accepts_reference": m.accepts_reference, "native_alpha": m.native_alpha,
                "price_hint": m.price_hint})).collect::<Vec<_>>()}),
        );
    }
    out.result(
        None,
        "models",
        json!({"local": local, "providers": providers}),
        &lines.join("\n"),
        false,
    );
    Ok(())
}

// ---------- settings and keys ----------

fn config(command: Option<ConfigCommand>, out: &Arc<Out>) -> Result<(), CliError> {
    let mut config = Config::load()?;
    let show = |config: &Config, keys: &[ConfigKey]| {
        let meta: serde_json::Map<String, serde_json::Value> = keys
            .iter()
            .map(|k| (k.id().to_string(), json!(config.get(*k))))
            .collect();
        let human = keys
            .iter()
            .map(|k| {
                let value = config.get(*k).map_or_else(
                    || match k {
                        ConfigKey::Provider => format!("{LOCAL} (default)"),
                        _ => "auto (default)".into(),
                    },
                    str::to_string,
                );
                format!("{:9} {value}", k.id())
            })
            .collect::<Vec<_>>()
            .join("\n");
        out.result(
            None,
            "config",
            serde_json::Value::Object(meta),
            &human,
            false,
        );
    };
    let all = [
        ConfigKey::Provider,
        ConfigKey::Model,
        ConfigKey::Tier,
        ConfigKey::Backend,
    ];
    match command {
        None | Some(ConfigCommand::Get { key: None }) => show(&config, &all),
        Some(ConfigCommand::Get { key: Some(key) }) => show(&config, &[key]),
        Some(ConfigCommand::Set { key, value }) => {
            config.set(key, &value)?;
            let path = config.save()?;
            out.note(&format!("saved in {}", path.display()));
            show(&config, &all);
        }
        Some(ConfigCommand::Unset { key }) => {
            config.unset(key);
            let path = config.save()?;
            out.note(&format!("saved in {}", path.display()));
            show(&config, &all);
        }
        Some(ConfigCommand::Path) => {
            let path = config::config_file();
            out.result(
                path.as_deref(),
                "config_path",
                json!(path),
                &path
                    .as_ref()
                    .map_or("nowhere: this account has no config folder".into(), |p| {
                        p.display().to_string()
                    }),
                false,
            );
        }
    }
    Ok(())
}

fn provider_or_error(id: &str) -> Result<&'static folderskin_ai::ProviderInfo, CliError> {
    folderskin_ai::provider(&id.to_lowercase()).ok_or_else(|| {
        CliError::fixable(
            "unknown_provider",
            format!("FolderSkin doesn't know a provider called {id:?}."),
            format!("Keys are for {}.", config::provider_ids().join(", ")),
        )
        .fix("See them all: folderskin ai models")
    })
}

fn key(command: KeyCommand, out: &Arc<Out>) -> Result<(), CliError> {
    match command {
        KeyCommand::Set { provider } => {
            let p = provider_or_error(&provider)?;
            let key = terminal::read_secret(&format!(
                "Paste your {} key ({}), then press Enter: ",
                p.label, p.key_hint
            ))
            .map_err(|e| CliError::io("read the key", Path::new("standard input"), &e))?;
            if key.is_empty() {
                return Err(CliError::fixable(
                    "empty_key",
                    "No key was given.",
                    "The key read from standard input was empty.",
                )
                .fix(format!(
                    "Run it again and paste the key, or pipe it in: echo <key> | folderskin ai key set {}",
                    p.id
                )));
            }
            let dir = config::config_dir().ok_or_else(|| {
                CliError::environment(
                    "no_config_folder",
                    "There is nowhere to keep the key.",
                    "This account has no configuration folder.",
                )
                .fix(format!("Use {} instead.", config::key_variable(p.id)))
            })?;
            let keys = config::keys();
            keys.set(p.id, &key).map_err(|why| {
                CliError::environment("key_not_saved", "The key couldn't be saved.", why)
            })?;
            out.result(
                Some(&dir),
                "key_saved",
                json!({"provider": p.id}),
                &format!(
                    "Saved your {} key, sealed in {} (the app uses it too). Check it with: folderskin ai key test {}",
                    p.label,
                    dir.display(),
                    p.id
                ),
                false,
            );
        }
        KeyCommand::Clear { provider } => {
            let p = provider_or_error(&provider)?;
            config::keys().clear(p.id).map_err(|why| {
                CliError::environment("key_not_saved", "The key couldn't be removed.", why)
            })?;
            let mut human = format!("Removed the saved {} key.", p.label);
            if std::env::var_os(config::key_variable(p.id)).is_some() {
                human.push_str(&format!(
                    " {} is still set in this session.",
                    config::key_variable(p.id)
                ));
            }
            out.result(
                None,
                "key_cleared",
                json!({"provider": p.id}),
                &human,
                false,
            );
        }
        KeyCommand::Test { provider } => {
            let p = provider_or_error(&provider)?;
            let keys = config::keys();
            let Some((key, source)) = config::key_for(p.id, &keys) else {
                return Err(CliError::fixable(
                    "key_missing",
                    format!("There is no {} key to test.", p.label),
                    format!(
                        "None is saved, and {} isn't set.",
                        config::key_variable(p.id)
                    ),
                )
                .fix(format!("Save one: folderskin ai key set {}", p.id)));
            };
            runtime()?
                .block_on(folderskin_ai::test_key(p.id, &key))
                .map_err(|e| paint::ai_error(p, e))?;
            let checked = !matches!(p.id, "bfl" | "ideogram");
            let from = match source {
                config::KeySource::Environment => config::key_variable(p.id),
                config::KeySource::Saved => "the saved keys".into(),
            };
            out.result(
                None,
                "key_ok",
                json!({"provider": p.id, "checked": checked}),
                &if checked {
                    format!("{} accepted the key from {from}.", p.label)
                } else {
                    format!(
                        "{} has no free way to check a key; it is checked the first time you paint with it.",
                        p.label
                    )
                },
                false,
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fs-cli-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn briefs_are_read_with_their_references_beside_them() {
        let dir = temp_dir("briefs");
        std::fs::write(dir.join("biscuit.jpg"), b"x").unwrap();
        std::fs::write(
            dir.join("briefs.json"),
            r#"[
              {"idea": "a lighthouse on a rocky island at dusk", "style": "woodblock", "n": 2, "name": "lighthouse"},
              {"idea": "our dog Biscuit asleep on a sofa", "style": "anime", "refs": ["biscuit.jpg"]},
              {"idea": "a koi pond at night with paper lanterns", "style": "woodblock", "shape": "folder", "seed": 7}
            ]"#,
        )
        .unwrap();
        let briefs = read_briefs(&dir.join("briefs.json")).unwrap();
        assert_eq!(briefs.len(), 3);
        assert_eq!(briefs[0].n, Some(2));
        assert_eq!(PathBuf::from(&briefs[1].refs[0]), dir.join("biscuit.jpg"));
        assert_eq!(briefs[2].shape.as_deref(), Some("folder"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_wrong_brief_says_which_and_why() {
        let dir = temp_dir("bad-briefs");
        let check = |text: &str| {
            std::fs::write(dir.join("b.json"), text).unwrap();
            read_briefs(&dir.join("b.json")).unwrap_err()
        };
        let e = check(r#"[{"idea": "x", "ref": ["a.jpg"]}]"#);
        assert_eq!(e.code, "bad_briefs");
        assert!(e.why.contains("unknown field `ref`"), "{e:?}");
        let e = check(r#"[{"idea": "x"}, {"idea": "y", "shape": "skin"}]"#);
        assert!(e.what.starts_with("Brief 2"), "{e:?}");
        let e = check(r#"[{"idea": "x", "refs": ["gone.jpg"]}]"#);
        assert_eq!(e.code, "reference_missing");
        assert!(e.why.contains("gone.jpg"));
        let e = check("not json");
        assert!(e.fix[0].contains("JSON list"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn theming_skips_hidden_system_and_tool_folders() {
        let root = temp_dir("theme");
        for d in [
            "Photos",
            "Taxes 2025",
            ".git",
            ".cache",
            "$RECYCLE.BIN",
            "node_modules",
            "Photos/Holidays",
            "Photos/target",
        ] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        std::fs::write(root.join("notes.txt"), b"x").unwrap();
        let one = folders_under(&root, 1).unwrap();
        assert_eq!(one, [root.join("Photos"), root.join("Taxes 2025")]);
        let two = folders_under(&root, 2).unwrap();
        assert_eq!(two.len(), 3);
        let words: Vec<String> = theme_names(&root, &two)
            .iter()
            .map(|n| n[..n.len() - 9].to_string())
            .collect();
        assert_eq!(words, ["photos", "taxes-2025", "photos-holidays"]);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn theme_names_never_collide() {
        let root = PathBuf::from("/r");
        let names = theme_names(
            &root,
            &[root.join("A b"), root.join("a-b"), root.join("A_B")],
        );
        assert!(names.iter().all(|n| n.starts_with("a-b-")), "{names:?}");
        let unique: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(unique.len(), 3, "{names:?}");
    }

    #[test]
    fn a_folder_keeps_its_name_whatever_is_beside_it() {
        // Non-English names used to become "skin", "skin-x", … in order, so adding a folder
        // handed one folder's picture to another.
        let root = PathBuf::from("/r");
        let first = theme_names(&root, &[root.join("Фото"), root.join("照片")]);
        let later = theme_names(
            &root,
            &[root.join("Документы"), root.join("Фото"), root.join("照片")],
        );
        assert_eq!(later[1..], first[..], "{first:?} {later:?}");
        assert!(first[0].starts_with("фото-"), "{first:?}");
        assert!(first[1].starts_with("照片-"), "{first:?}");
        assert!(later[0].starts_with("документы-"), "{later:?}");
        // The same on every system, and for the same folder next time.
        assert_eq!(
            theme_names(&root, &[root.join("Photos").join("Holidays")]),
            theme_names(&root, &[PathBuf::from("/r/Photos/Holidays")])
        );
        assert_eq!(
            fingerprint("a"),
            0xaf63_dc4c_8601_ec8c,
            "FNV-1a's published value"
        );
        assert_eq!(words("!!!", 40), "folder");
        assert_eq!(words("Summer '25 — Crète", 40), "summer-25-crète");
    }
}
