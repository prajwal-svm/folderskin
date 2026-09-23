---
name: folderskin-localgen
description: Paints FolderSkin folder art on this computer with open-weight models, no API key and no filters. Z-Image-Turbo for text to picture and FLUX.2 [klein] 4B for reference pictures and whole-folder skins, both Apache-2.0, run by stable-diffusion.cpp (CUDA or Vulkan on Windows and Linux) or mflux (MLX on Apple Silicon), all driven by the `folderskin` command line. Sets the machine up with `folderskin ai setup`, paints single ideas or whole batches in a style (pop art, anime, oil, sketch, woodblock and more), themes every folder under a root from its name and applies the results, works from one or several reference photos, repaints FolderSkin's own blank folder and cuts it out along the app's exact silhouette, checks and cleans pictures up (trim, clip, cut out, adjust), and previews every result as the folder the app makes of it. Use when the user says "generate locally", "make folder art offline", "paint a folder of X in Y style", "use this photo as a folder", "batch generate skins", "theme my whole drive", "paint all these folders", "set up local generation", "make a pack of N skins about X", or asks which local model or GPU settings to use.
version: 2.0.0
---

# Painting folder art locally

The `folderskin` command line paints pictures for FolderSkin with models that run on this
computer. What it paints goes through FolderSkin's own code afterwards, so a result always lands
on the folder the way the app would put it there. Making a community pack from the results is the
other skill, `folderskin-skins`; this one ends with a folder of good pictures.

From a checkout, run it as `cargo run -p folderskin-cli --release -- …` from the repository root
(the first build takes a few minutes; release mode, because cleaning up and previewing 1024 px
pictures is slow otherwise). Where it is installed (`scripts/install-cli.sh`, or
`scripts/install-cli.ps1` on Windows), it is just `folderskin …`; the examples below use that
form. The runtime and the models live outside the repository, in
`%LOCALAPPDATA%\folderskin-localgen` (Windows), `~/Library/Caches/folderskin-localgen` (macOS) or
`~/.cache/folderskin-localgen` (Linux); `FOLDERSKIN_LOCALGEN_HOME` moves them.

Every command explains its own failures (what happened, why, what to try, and a ready-to-paste
`claude "…"` line) and exits 1 for something to put right, 2 for a wrong command line, 3 when the
computer is missing something (runtime, models, driver, network) and 70 for a bug. `--json` makes
any command write one JSON event per line instead, for a script to read; `--verbose` shows
everything the runtime prints.

## Step 1: check the machine, then set it up

```sh
folderskin ai doctor
folderskin ai setup
```

`doctor` says what it found and what it will use: the backend (`cuda` for NVIDIA on Windows,
`vulkan` for other GPUs and Linux, `mlx` on Apple Silicon, `cpu` otherwise) and the tier (`q8`
with 24 GB of RAM or more, `q4` below), what is installed, and the next command to run. `setup`
downloads the pinned stable-diffusion.cpp build and the models, about 15.7 GB at `q8` and 9.4 GB
at `q4`, resuming where a download stopped and checking every file against its published SHA-256.
On a Mac it installs mflux with `uv tool install`, which downloads each model the first time it
runs it. Ctrl+C stops it cleanly; running it again carries on.

`--backend` and `--tier` override the choice for one command; `folderskin ai config set tier q4`
(or `backend`, `model`, `provider`) makes it the default.

RAM decides the tier, not VRAM: stable-diffusion.cpp streams weights from RAM when they do not fit
on the card, so a 4 GB laptop GPU runs the 8-bit models.

Measured on an RTX 3050 Ti laptop GPU (4 GB) with 32 GB of RAM, `q8`, 1024 × 960:

| picture | `cuda` | `vulkan` |
|---|---|---|
| artwork, Z-Image-Turbo (8 steps) | 60 s | |
| artwork, `--model klein` (4 steps) | 28 s | 65 s |
| one or two references, klein | 30–35 s | |
| whole folder, klein | 31 s | |

On a laptop with a second, integrated GPU, Vulkan lists both; on the test laptop (AMD integrated
plus NVIDIA) stable-diffusion.cpp chose the NVIDIA card by itself, and `doctor` lists what it sees.
A Mac has no numbers here yet.

## Step 2: paint

```sh
folderskin ai gen "a retro film camera with a chrome lens" --style pop-art -n 4
```

- The idea is the subject and the scene in plain words (`-` reads it from standard input).
  `--style` is a preset (`folderskin ai styles` lists them: pop-art, anime, oil, sketch,
  woodblock, travel-poster, watercolour, clay, risograph, art-nouveau, pixel, synthwave, photo) or
  your own words. `--raw` sends the idea to the model word for word, without FolderSkin's prompt.
- `-n 4` paints four seeds in a row, and `--seed` fixes where they start, so a good one can be
  painted again exactly. `--name` names the file (with `-n 1`).
- Results go to `folderskin-out/` (`--out` changes it): the picture, a `.json` beside it with the
  idea, prompt, model, licence, seed and settings, and `previews/` with each picture drawn as the
  folder the app makes of it plus `_sheet.png` with all of them. Each finished picture's path is
  printed on its own line.
- `--apply "<folder>"` puts the (first) picture on a folder straight away.

The three ways to paint:

| you want | use | model |
|---|---|---|
| a picture wrapped onto FolderSkin's folder (the normal case) | `ai gen "idea" --style …` | Z-Image-Turbo |
| the same, from photos or pictures you have | `ai gen "idea" --ref a.jpg [--ref b.png]` | FLUX.2 [klein] 4B |
| the whole folder painted as one object | `ai gen "idea" --shape folder` (refs allowed) | FLUX.2 [klein] 4B |

`--model klein` paints plain artwork with klein instead: twice as fast, and in a side-by-side of
pop art, anime, oil and sketch it was as good, bolder in pop art and richer in sketch hatching,
where Z-Image was more painterly in oil. For a big batch, klein; for photographic subjects and
lettering, Z-Image. Whole-folder pictures are cut out along FolderSkin's own silhouette, not by
colour: the model repaints the app's blank folder, the command line finds the painted folder, fits
the silhouette to it and uses that as the edge. The report line gives the fit; below 0.95 the
model changed the folder's shape, and the picture is left on its backdrop for you to look at.

`--provider openai` (or xai, google, bfl, recraft, stability, ideogram) paints the same idea with
your own key instead (`folderskin ai key set openai`, or `FOLDERSKIN_OPENAI_KEY`), processed the
way the app does. That is not local and not free; this skill is about the local models.

## Step 3: paint a batch

For a pack, write the briefs down once and let it run:

```json
[
  {"idea": "a lighthouse on a rocky island at dusk", "style": "woodblock", "n": 2, "name": "lighthouse"},
  {"idea": "our dog Biscuit asleep on a sofa", "style": "anime", "refs": ["biscuit.jpg"]},
  {"idea": "a koi pond at night with paper lanterns", "style": "woodblock", "shape": "folder", "seed": 7}
]
```

```sh
folderskin ai batch briefs.json --out renders/night-prints
```

Reference paths are relative to the JSON file. `name` names the file (numbered when `n` is more
than one); `model` picks the artwork model per brief. Every brief is checked before the first
picture is painted, and the run ends with `previews/_sheet.png`.

### Or theme a whole drive

`theme` paints every folder under a root from the folder's own name, all in one style, and with
`--apply` puts each picture on its folder through the app's own code:

```sh
folderskin ai theme "D:/Projects" --style risograph
folderskin ai theme "D:/Projects" --style risograph --apply
```

Run it without `--apply` first and look at `previews/_sheet.png`; a second run skips folders that
already have a picture, so it only applies. It uses klein (about 28 s a folder on a 4 GB laptop
GPU: a hundred folders is under an hour) and leaves out hidden folders, `$` folders and tool
folders such as `node_modules`. `--depth 2` goes a level deeper. `folderskin revert "<folder>"`
takes an icon off again. A name that is an idea rather than a thing ("Taxes 2025") tends to come
back as lettering; paint that one with `ai gen` and a subject of your own.

## Step 4: look at every result

Open `previews/_sheet.png`, then any picture that looks doubtful at full size in `previews/`.
`folderskin image check <picture>` says what the app will make of a picture and what to fix.

| what you see | what to do |
|---|---|
| the subject cut by the paper strip, or high on the folder | another seed; or say where it is: "…, standing low in the frame" |
| a blank band or a frame along the folder's edges | paper margins are cut already (the `.json` says `border_trimmed`); `folderskin image trim <picture>` does it for any picture; a frame drawn inside the art is the model's, so try another seed |
| a whole folder whose fit was below 0.95 | another seed; the model moved or reshaped the folder (`folderskin image clip` cuts one that fits) |
| the style is weak | `--model klein`, or put more of the style into words |
| the colours are a little off | `folderskin image saturate <picture> 20`, `brightness`, `contrast`, or `image adjust --hue …`: the composer's own adjustments |
| text or a signature in the art | another seed; small models write when a style suggests posters |

Say which check failed before painting again.

## Step 5: make it a pack

The pictures in the output folder (not `previews/`, not `raw/`) are ready for the other skill.
`folderskin packs make` makes the pack, and uses the `cwebp` setup installed (on Windows) when
there is none on the PATH, which whole-folder pictures need to fit in a pack:

```sh
folderskin packs make renders/night-prints --id night-prints \
  --name "Night prints" --tags woodblock,night --author <github-name> --preview /tmp/night-prints.png
```

Whole-folder pictures come out as `folder` in the report, everything else as `artwork`. Follow
`folderskin-skins` from its step 5.

## Rules

- Both models are Apache-2.0, which puts no condition on what they paint, so a result can go in a
  pack under CC0-1.0, CC-BY-4.0 or MIT. Do not add models whose licence is non-commercial or
  restricted (Qwen-Image 2.1, FLUX.2 [dev], klein 9B, Kontext dev, Krea 2, Ideogram 4) to the
  command line: their pictures cannot go in a pack. An ungated mirror of such a model is still
  under its licence.
- Nothing filters a prompt or a picture, and a brand or a named product paints as asked; that
  makes the person running it responsible for what it paints. (klein's weights were fine-tuned by
  their makers against sexual imagery of minors and non-consensual imagery; nothing else is held
  back.) Pictures for a pack still follow `docs/PACK-TERMS.md`: no brand, logo or character that
  belongs to someone else, and no living artist or studio named as the style. The presets describe
  a look without naming anyone; keep it that way.
- Keep the `.json` beside each picture until the pack is made: it is the picture's provenance
  (model, licence, prompt, seed and the SHA-256 of each reference).
- Never commit renders, models or the runtime.

## Commands

| command | use |
|---|---|
| `folderskin ai doctor [--backend …] [--tier …]` | what the machine is, and what is installed |
| `folderskin ai setup [--backend …] [--tier …] [--runtime latest]` | download the runtime and the models |
| `folderskin ai gen "idea" [--style S] [--shape artwork\|folder] [--ref P]… [-n N] [--seed S] [--model auto\|zimage\|klein] [--out DIR] [--apply FOLDER]` | paint pictures from one idea |
| `folderskin ai batch briefs.json [--out DIR]` | paint every brief in a file |
| `folderskin ai theme <root> [--style S] [--depth N] [--apply]` | paint every folder under a root from its name |
| `folderskin ai styles` / `folderskin ai models` | the style presets / the models and providers |
| `folderskin ai config [set\|unset\|get] …` | default provider, model, tier and backend |
| `folderskin packs make <pictures…> --id … --name … --tags … --author …` | make a pack, with cwebp |
| `folderskin image check\|info\|trim\|clip\|cutout\|crop <picture>` | look at a picture and clean it up |
| `folderskin image saturate\|brightness\|contrast\|invert\|adjust <picture> …` | the composer's colour adjustments |
| `folderskin render <picture> --out preview.png` | a picture as the folder the app makes of it |
| `folderskin apply <folder> --image <picture>` / `folderskin revert <folder>` | put an icon on a folder, or take it off |
| `folderskin template --width 1024 --height 960 --out t.png --mask m.png` | the blank folder a model repaints, and its silhouette |
