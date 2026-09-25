---
name: folderskin-skins
description: Makes and checks FolderSkin community skin packs. Turns a folder of pictures (renders from Grok Imagine, ChatGPT, Gemini or any image model, photos, paintings) into a pack in the folderskin-community repository with `folderskin-tools packs make`, looks at every skin as the folder the app makes of it, cuts drifted pink or raspberry backdrops away with --flat-backdrop, names and orders the skins, validates the pack with `packs check`, and refreshes the index with `packs index`. Also previews any single picture as a folder icon with `render` and writes the safe-area template with `guide`. Use when the user says "make a pack", "make a pack from these", "turn these renders into a pack", "add a community pack", "share these as a pack", "check the packs", "update the pack index", "preview this picture as a folder", "how will this look on a folder" or "turn this photo into a folder icon", or asks where a picture's subject lands on the folder.
version: 3.0.0
---

# Making FolderSkin community packs

FolderSkin ships no skins of its own. Every skin in a library is a picture the user added, an AI
result, or a skin from a community pack: a folder under `packs/` in the packs repository,
github.com/prajwal-svm/folderskin-community,
which the app downloads from GitHub, and which the first-launch welcome offers to new users. This
skill makes and checks those packs.

Follow the steps in order. Run every command from the repository root, the directory that holds
`Cargo.toml`, with folderskin-community checked out beside it as `../folderskin-community`
(`git clone https://github.com/prajwal-svm/folderskin-community ../folderskin-community` if it isn't). Background, if you need it: [docs/PACKS.md](../../../docs/PACKS.md)
has the contract and its limits, and [docs/SKINS.md](../../../docs/SKINS.md) how a picture becomes a
folder icon.

## Step 1 — collect the pictures and the facts

1. Get the absolute path to the folder of pictures, or to the files, from the user. If they have
   not given one, ask; do not guess and do not invent art.
2. Accepted inputs are PNG, JPEG and WebP. Convert HEIC or HEIF first (macOS only):

   ```sh
   sips -s format png "<picture>.heic" --out /tmp/<name>.png
   ```

3. Ask who made the pictures and under what licence, unless the user already said. A pack is
   `CC0-1.0` (the default), `CC-BY-4.0` or `MIT`, and may only hold pictures the user made or is
   allowed to share. If the answer is "I found them online" or is unclear, stop: the pictures
   cannot go in a pack, but the user can drop them on the app to use them on their own machine.
4. The author is the user's GitHub user name.
5. A pack holds 1 to 50 skins. Split a bigger set into themed packs.

## Step 2 — name and order the pictures

Skins are named after their files: `glass_folder-2.png` becomes "Glass folder 2". Look at each
picture with the Read tool and rename meaningless files such as `grok-image-3.jpg` before making
the pack, or fix the names in `pack.json` afterwards (1 to 60 characters).

Don't trust that a batch's file names match its pictures. A zip saved by a batch job can put
pictures under the wrong names, or mix in pictures from another batch: a "Scientists" zip once
held city postcards filed as scientists. Make the pack into a scratch `--dir` with `--preview`
first and look over the contact sheet, whose cells are in file-name order. When the pictures
show real people, never name one from their face. Use text in the picture (a caption, a speech
bubble), or ask the user. If the names can't be trusted, stop and tell the user rather than
publishing.

Order matters: the library shows a pack's skins in its own order, first skin first, and the
pack's preview strip is its first four. `packs make` takes files in the order given and the
pictures inside a folder in name order. To get another order, pass the files one by one, or move
the entries in `pack.json` afterwards; the files never need renaming for it.

## Step 3 — choose the name and tags

- `--name`: what the app shows, 1 to 40 characters, e.g. `Night prints`. Names can repeat: other
  packs may have the same one.
- `--tags`: 1 to 5, comma-separated, lower case. The first names the pack in everyone's filters.

Propose the two to the user in one line and carry on unless they object. The id isn't chosen:
`packs make` gives a new pack one, its name and six random characters such as
`night-prints-h4x2qe`. It is the pack's folder name and part of every download URL, and it never
changes.

## Step 4 — make the pack

```sh
cargo run -p folderskin-tools -- packs make "<folder or files>" --dir ../folderskin-community --name "<Name>" \
  --tags <first>,<more> --author <github-name> --preview /tmp/<name>.png
```

- It writes `../folderskin-community/packs/<id>/` (`--dir` names the folderskin-community checkout; it
  defaults to the current folder) with `pack.json` and one
  picture per skin, each shrunk to 1024 px and saved as a lossless WebP of at most 1.5 MB (the
  pack limit; `--max-kb` holds them to less). A picture too detailed for that is made 896 px, then
  768 px, and the report says so. A pack's pictures come to 64 MB at most, so split a bigger set
  into two packs. The report says the id; use it for every step below.
- Nothing needs installing: the encoder is built in. It takes several seconds a picture, on every
  core at once.
- `--license` defaults to `CC0-1.0`; pass the licence from step 1 if it is another.
- Run again without `--id`, it makes a second pack. To make the same pack again, pass its id:
  `--id <id>` replaces `packs/<id>/` once the new folder passes, and the pack keeps its id.
- It runs `packs check` on what it wrote, and leaves nothing behind if anything fails.

## Step 5 — check the split

Every line of the report says `folder` or `artwork`:

- `folder`: a finished folder, cut out of its magenta or transparent background and used as the
  icon exactly as drawn.
- `artwork`: an ordinary picture, wrapped onto FolderSkin's own folder template.

This is the split the app makes when someone adds the picture. Renders meant as whole folders
that came out as `artwork` usually sit on a drifted pink or raspberry instead of `#FF00FF`
(Grok does this). Make it again with `--id <id> --flat-backdrop`, which keeps its id: it measures
each picture's own flat background, removes only what reaches the edge (so a red cloak inside the
folder stays), takes a soft drop shadow with it, and gives the edge the painting's colours rather
than a pink rim. A picture with no flat background still comes out as `artwork`.

## Step 6 — look at every skin

Open the `--preview` sheet with the Read tool. It draws every skin as the folder the app makes of
it. Check:

| skin | what a failure looks like |
|---|---|
| finished folder | a magenta or pink fringe along the edge, a dark blob left from a drop shadow, a tab cut off, a folder that does not fill its square |
| artwork | a face, horizon or logo cut by the paper strip (the top 12% of the picture), the subject clipped at the top or bottom of the front panel, detail lost in the rounded corners |
| any | a name that reads badly |

For a close look at one picture, draw it large; a one-pixel pink rim or a leftover shadow often
only shows at full size:

```sh
cargo run -p folderskin-tools -- render ../folderskin-community/packs/<id>/<file> --out /tmp/one.png --size 1024
```

`render` also says whether the picture is a finished folder or artwork. To see where the folder
crops artwork, write the safe-area template and open it:

```sh
cargo run -p folderskin-tools -- guide --out /tmp/guide.png
```

Rows 0 to 62 of a 958-row picture land in the tab, 62 to 127 beside the paper strip, and the
front panel keeps rows 60 to 898. The app always crops artwork around its centre (a pack has no
focus point), so an off-centre subject is fixed by cropping the source picture and making the
pack again. `render --focus x,y` shows what another crop would keep before you crop the file.

Say which check failed before changing anything.

## Step 7 — fix names, then check

Edit `../folderskin-community/packs/<id>/pack.json` for names, per-skin tags (up to 3 each) or order. It takes
no other fields. Then:

```sh
cargo run -p folderskin-tools -- packs check --dir ../folderskin-community
```

It checks every pack in `packs/` the way the app does before it saves one, prints each
problem as a sentence and exits non-zero on any. Fix every problem before moving on.

## Step 8 — try it in the app (optional)

In the app, **Community → Add from a folder** with `../folderskin-community/packs/<id>` adds the pack exactly as
one from GitHub would. To try the Community list itself, run step 9, serve the repository with
`python3 -m http.server` from its root, and start the app with
`FOLDERSKIN_COMMUNITY_URL=http://localhost:8000`, serving the folderskin-community checkout from its root. Add `FOLDERSKIN_ONBOARDING=1` to see
the pack offered in the first-launch welcome as well.

## Step 9 — the index

`index.json` and `previews/<id>.png` in folderskin-community are what the app reads to list the
packs. The Packs workflow in folderskin-community rebuilds them on `main` after a pack is merged, so a pull request
does not need them, and nobody edits them by hand. To rebuild them locally:

```sh
cargo run -p folderskin-tools -- packs index --dir ../folderskin-community
```

It checks every pack first and writes nothing when one has a problem. It is deterministic, so a
second run changes nothing. Each entry is dated by the commit that added the pack (so a pack
that isn't committed yet has no date), and marked official when `official.json` beside `packs/`
lists it. `official.json` and `featured.json` are the maintainer's own lists; don't add a pack to
either unless the user asks.

## Step 10 — report

Tell the user, in a few lines:

- the pack folder, its id, name, tags, licence and author;
- how many skins, how many finished folders and how many artwork, and the total size;
- whether `--flat-backdrop` was used;
- the preview sheet's path;
- that `packs check` passed.

Do not commit anything unless the user asks. The files to commit are `packs/<id>/` in
folderskin-community, where a pull request proposes the pack.

## Rules

- Only pictures the user made or is allowed to share, under `CC0-1.0`, `CC-BY-4.0` or `MIT`, with
  their GitHub name as the author. Never art extracted from another product, a stock photo, a
  wallpaper, or model output whose terms have not been checked.
- 1 to 50 skins a pack; pictures lossless (PNG or lossless WebP, which `packs make` writes),
  256 to 1024 px on each side and at most 1.5 MB; 64 MB a pack.
- Never hand-edit `index.json`, `previews/` or `v2/` in folderskin-community.
- FolderSkin ships no skins: nothing goes under `assets/`, and there is no skin manifest and no
  pack built into the app.
- HEIC and HEIF must be converted with `sips` first, which only works on macOS.

## Commands

| command | use |
|---|---|
| `cargo run -p folderskin-tools -- packs make <pictures…> --dir ../folderskin-community --name … --tags … --author … [--id ID] [--preview PNG] [--flat-backdrop]` | make a pack in folderskin-community's `packs/` from pictures or folders of them; `--id` makes one that is there again |
| `cargo run -p folderskin-tools -- packs check --dir ../folderskin-community` | check every community pack the way the app and CI do |
| `cargo run -p folderskin-tools -- packs index --dir ../folderskin-community` | rebuild `index.json` and the preview strips |
| `cargo run -p folderskin-tools -- render "<picture>" --out /tmp/icon.png --size 512` | draw one picture as the folder the app makes of it, and say which kind it is |
| `cargo run -p folderskin-tools -- render --solid RRGGBB --out /tmp/flat.png` | the template in a flat colour, to inspect the template itself |
| `cargo run -p folderskin-tools -- guide --out /tmp/guide.png` | the 1024 × 958 safe-area template |
| `cargo run -p folderskin-tools -- apply "<folder>" --image "<picture>"` | apply a picture to a real folder from the terminal, as the app would |
| `cargo run -p folderskin-tools -- revert "<folder>"` | put the default icon back |
