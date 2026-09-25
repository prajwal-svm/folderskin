# Community skins and packs

Anyone can share skins with everyone who uses FolderSkin, for free. A shared set of skins is a
**pack**, and one skin on its own is a pack of one. Packs live in their own repository,
[folderskin-community](https://github.com/prajwal-svm/folderskin-community), under `packs/`, and
adding one needs no account. FolderSkin ships no skins of its own: packs, your own pictures and AI
results are where every skin comes from.

## Adding a pack

The first time FolderSkin opens, it offers packs to start your library with. After that, open
**Community** in the app. The filters at the top are the packs' tags. **Add** puts a pack's skins
in your library, carrying the pack's tags, and each skin's ⋯ menu says which pack it came from and
who shared it. **Remove** takes the whole pack out again. Folders that already use one of its
skins keep their icon, because the icon lives in the folder itself.

**Add from a folder** does the same for a pack folder on your computer, which is also how you try
a pack out before sharing it.

The gallery on [folderskin.app](https://folderskin.app/community/) has an **Install** button on
every pack. It opens FolderSkin on that pack in Community and adds it, just as its **Add** button
would ([the install link](#the-install-link) below). Packs marked **Official** are ones the
maintainer vouches for.

A pack is added whole or not at all: every picture is downloaded and checked first, then all of
them are saved in one go, so a dropped connection or a full disk never leaves half a pack in your
library. Its skins appear in the pack's own order.

## Sharing yours

You share from the app, and FolderSkin sends the pack to its community service at
`community.folderskin.app`, where the maintainer reviews it. You need no GitHub account.
FolderSkin 0.1.6 and earlier could also open a pull request on GitHub for you, but 0.1.7 drops
that option.

1. Tag the skins you want to share (⋯ → Tags). To share one skin, use ⋯ → **Share with
   community**. To share several, use **Community → Share your skins** and pick a tag.
2. Fill in the pack's name, its tags and a licence, say where the pictures came from, and tick
   that they are yours to share.
3. The first time, FolderSkin verifies this computer in your browser, under the name your packs
   are credited to. It asks once per computer.
4. Send it. FolderSkin checks the pack against the contract below before anything leaves your
   computer. **Your submissions** shows each pack you sent and, if one is turned down, why.

Every picture is shared losslessly, so a pack looks exactly as you made it, a finished folder's
transparent edge included. FolderSkin makes each one a lossless WebP before it's sent, which takes
a few seconds a picture, and counts them as they're ready. A picture too detailed to fit in 1.5 MB
at 1024 px is made 896 px, then 768 px, still lossless, and FolderSkin says which. A pack's
pictures come to 64 MB at most. A bigger one is turned down with a suggestion to split it into
two packs.

A person looks at every pack before anyone else can see it. Once it's approved, it's published
on its own, within about 15 minutes ([How approval publishes](#how-approval-publishes) below).
Many packs can share a name: the one you pick is what everyone sees, and the pack gets an id of
its own ([Pack ids](#pack-ids)).

Packs also come in by hand, through a pull request to
[folderskin-community](https://github.com/prajwal-svm/folderskin-community) that adds one folder
under `packs/`. Make the folder with `packs make` ([Making a pack from
pictures](#making-a-pack-from-pictures)), which gives it a generated id, and the pull request
runs the same checks the app runs. **Save a folder** in the app writes a pack folder that follows
every rule below.

## Pack ids

Every pack has an id, which is its folder's name and part of every link to it. The id is made
once, when the pack is made, from its name and six random characters: a pack called Classic Art
gets an id such as `classic-art-k7q2mx`. Names can repeat freely, so a hundred packs can be
called Classic Art, and only the id has to be unique. `packs make` makes ids for packs made by
hand, and the community service for packs shared from the app. An id never changes after, even
when the pack's name does.

The random part is six characters from `a` to `z` and `2` to `7`, drawn from the system's secure
random source. A new id is never the name of a folder in `packs/`, or an old id in `moved.json`.

### moved.json

Packs made before ids were generated had ids made from their names alone, such as
`classic-art`. They were given generated ids with `packs rename`, and `moved.json`, beside
`packs/`, records each old id and the id that pack has now:

```json
{ "version": 1, "moved": { "classic-art": "classic-art-k7q2mx" } }
```

- Every old id is a pack id that no folder in `packs/` has, and it is never given to a new pack.
- Every new id is a pack in `packs/`. It never leads to another old id: renaming a pack again
  points everything that led to it at its newest id.
- `packs index` and `packs catalog` copy the map into `index.json` and `head.json` as `"moved"`,
  so the app, the website and the community service can follow a pack from its old id. From 0.1.7
  the app moves packs you added under an old id to the new one, and an install link with an old
  id still finds its pack.
- A renamed pack keeps the date it was first published: `packs index` dates it by the earliest
  commit that added any of its ids.
- Nothing goes in `pack.json`. It takes no field the contract doesn't name, and apps 0.1.4 to
  0.1.6 would turn the pack down.

### packs rename

```sh
cargo run -p folderskin-tools -- packs rename --dir ../folderskin-community --all
cargo run -p folderskin-tools -- packs rename --dir ../folderskin-community classic-art
cargo run -p folderskin-tools -- packs rename --dir ../folderskin-community classic-art --to classic-art-k7q2mx
```

It gives packs generated ids in a git checkout of folderskin-community. Each folder moves with
`git mv`, so its history goes with it. `featured.json` and `official.json` are rewritten to the new
ids, in the same order, and `moved.json` records every move, made if it isn't there. Everything is
staged, ready to commit.

`--all` renames every pack whose id isn't a generated one, and leaves the rest alone. It goes by
the shape alone: an old id whose last word happens to be six letters, such as
`data-structures-in-bricks`, looks generated, so rename that pack by its id. Named on its own, a
pack moves to a new generated id whatever its id is now, or to `--to`, which has to be a generated
id that no pack has and no pack had before. Running it again changes nothing: a pack that moved is
found in `moved.json` and said to have moved already.

## How approval publishes

Approving a pack publishes it. Nobody copies files by hand.

1. When the maintainer approves a pack, the community service gives it its id. When the service
   has a GitHub token (`GITHUB_DISPATCH_TOKEN`), it also starts the Packs workflow in
   folderskin-community at once, with a `pack-approved` repository dispatch.
2. Without one, the workflow finds the pack itself. Every 15 minutes it asks
   `https://community.folderskin.app/v1/exports/pending` how many approved packs are waiting,
   which takes seconds, and does the rest only when some are. It also runs every day and every
   week whatever is waiting, and whenever the maintainer starts it by hand.
3. `community pull --no-done` writes each approved pack into `packs/`, and checks every file's
   size and SHA-256 against what the service recorded when it was uploaded. The id the service
   gave has to be a generated one, and a folder that is there already is never written over or
   given a number. The pack's finished folders are given one shape ([One shape for a pack's
   folders](#one-shape-for-a-packs-folders)) before it is checked. A folder too far off that shape
   is kept as it is, and the run's log says so in a warning.
4. `packs check` checks every pack, as it does for a pull request.
5. Each pack is committed by github-actions[bot] as `Add the <name> pack`, and pushed to `main`.
6. Only then does `community done` tell the service the pack is published. A check or a push that
   fails leaves the pack waiting at the service, and the next run tries it again.
7. The same run rebuilds `index.json`, the previews and `v2/`, copies `v2/` to the mirror
   ([below](#the-mirror)), and commits them. A push made with the workflow's own token starts no
   other workflow, which is why it all happens in one run.

So a pack is published within about 15 minutes of approval, or within a few minutes when the
service starts the workflow itself. A run that fails publishes nothing, and GitHub emails the
maintainer that the workflow failed. GitHub can start a scheduled run late when it is busy, and it
turns the schedule off in a repository with no activity for 60 days. The dispatch depends on
neither.

The workflow needs the maintainer's signing key, the whole file `community keygen` wrote, as the
repository secret `FOLDERSKIN_ADMIN_KEY`. Without it, approved packs wait at the service, and the
run says so. The repository variable `REQUIRE_GENERATED_IDS`, set to `true`, makes every check
turn down a pack without a generated id.

Pulling by hand still works. `community pull` writes the packs and tells the service at once,
since whoever runs it commits what it wrote. `--no-done --pulled pulled.json` holds that back
until `community done --from pulled.json`, which is how the workflow runs it. Telling the service
twice does no harm.

### The mirror

The app can read the whole tree from `https://packs.folderskin.app`, a Cloudflare R2 bucket that
holds the same `v2/` as the repository. The community service writes to the bucket, so the
workflow needs no Cloudflare token of its own: `community mirror` sends each file through the
service, signed with the maintainer's key.

```sh
cargo run -p folderskin-tools -- community mirror --tree ../folderskin-community \
  --public https://packs.folderskin.app --api https://community.folderskin.app --key ~/folderskin-admin.key
```

It asks the mirror for every file with a HEAD request, and leaves alone one it already serves at
the same length: every name but `head.json` is the hash of what is in it. The rest are uploaded
with `PUT /v1/admin/tree/<path>`, each with its SHA-256 in `X-Content-SHA256` for the bucket to
check. `head.json` goes last, and only once every other file is there, so the mirror never names a
catalog it doesn't hold. A request that may pass on its own (no answer, a 5xx or a 429) is tried
five times in all, the wait doubling from a second, and a `Retry-After` is honoured up to 30
seconds. The workflow runs it before it commits, so GitHub's `head.json` only moves once the
mirror has everything it names. The repository variable `COMMUNITY_MIRROR_URL` turns the mirror
on, and `head.json` lists it only while it's on.

## The contract

A pack is one folder:

```
packs/night-prints-h4x2qe/
  pack.json
  koi.webp
  fox-in-the-rain.webp
```

`pack.json`:

```json
{
  "version": 1,
  "name": "Night prints",
  "author": "your-github-name",
  "license": "CC0-1.0",
  "tags": ["woodblock", "night"],
  "skins": [
    { "file": "koi.webp", "name": "Koi over the wave", "tags": ["animals"] },
    { "file": "fox-in-the-rain.webp", "name": "Fox in the rain" }
  ]
}
```

| field | rule |
|---|---|
| `version` | `1` |
| `name` | 1 to 40 characters |
| `author` | your GitHub user name |
| `license` | `CC0-1.0`, `CC-BY-4.0` or `MIT` |
| `tags` | 1 to 5 tags. Every skin in the pack gets them, and the first names the pack in everyone's filters |
| `skins` | 1 to 50 entries |
| `skins[].file` | a picture in the folder |
| `skins[].name` | 1 to 60 characters |
| `skins[].tags` | optional, up to 3 more for that skin |

No other fields are allowed, so a typo such as `"tag"` fails the check instead of being ignored.

### Limits

| | limit |
|---|---|
| skins in a pack | 1 to 50 |
| each picture | lossless: PNG, or lossless WebP. At most 1.5 MB |
| a pack's pictures together | at most 64 MB |
| picture sides | 256 to 1024 px |
| file names | letters, digits, `.`, `-` and `_`, ending in `.png` or `.webp` |
| id, the folder's name | a name and six random characters, such as `night-prints-h4x2qe` ([Pack ids](#pack-ids)): lower-case letters and digits in words joined by single dashes, at most 40 characters |
| tags | lower case letters, digits, spaces and dashes, at most 24 characters |
| `pack.json` | at most 64 KB |

Why 50 and 64 MB: a pack is a themed set, and everyone who adds it downloads all of it. Fifty
skins stay quick to review, and 64 MB holds all fifty at 1.3 MB a picture, or forty-two at the
largest.

Packs published before FolderSkin 0.1.7 were held to 2 MB a picture in PNG, JPEG or any WebP,
and the app still reads them. Made again with `packs make`, they follow the rules above.
`packs check` holds a pack to them with `--require-lossless` ([Checking a pack
yourself](#checking-a-pack-yourself)), and the community service takes nothing else.

### Pictures

Each picture is one of two kinds, told apart the same way as a picture dropped on the window:

- **A finished folder** on a transparent background, or on flat magenta `#FF00FF`, which
  FolderSkin cuts away. It becomes the icon exactly as drawn.
- **Anything else** is wrapped onto FolderSkin's folder. [SKINS.md](SKINS.md) shows where the
  folder crops a picture, so the subject survives.

1024 px is the largest icon any of the three systems draws, so a bigger picture adds nothing.

Every picture is lossless, so a pack looks exactly as it was made: no blocks in a gradient, no
ringing around lettering, and a finished folder's edge as clean as it was drawn. A lossless WebP
is about a third smaller than the same PNG, which is why the app and `packs make` write WebP. A
detailed 1024 px picture comes to between 0.6 and 1.5 MB, most of them about 800 KB. One that
doesn't fit in 1.5 MB is made 896 px, then 768 px, still lossless, rather than blurred to fit.

### One shape for a pack's folders

FolderSkin fits the whole of a finished folder's picture into the icon. Folders made one at a
time come out cropped tight to themselves, and no two renders have quite the same proportions:
one pack's ran from 1.03 to 1.30 times as wide as they were tall. Side by side in Finder, the
squat ones looked smaller than the tall ones. So the finished folders in a pack share one shape:

- **The pack's shape** is the median of its folders' own. A folder is measured by the part of
  its picture more than half opaque, width ÷ height, so a soft edge or a faint shadow doesn't
  count.
- **Each folder is redrawn at exactly that shape.** It is cropped to its folder and resized to
  962 px wide, which is how wide FolderSkin's own folder is in its 1024 px template
  (`folderskin-tools template`), and as tall as the shape makes it. Then it's put where the
  template's folder is, on the same left edge and standing on the same baseline, in a
  transparent 1024 × 1024 picture, and saved as a lossless WebP. A PNG becomes a `.webp` of the
  same name, and `pack.json` follows.
- **A folder is reshaped by 8% at most.** Nobody sees that much. A folder that would need more
  is an *outlier*: stretched that far, lettering and faces look squashed, so it is never
  reshaped. What happens to it depends on the command, and a person decides.
- **Artwork is left alone.** FolderSkin wraps it onto its own folder, so it has no shape of its
  own to fix. So is a pack with one finished folder.

`packs make` does this for every pack it makes, `community pull` for every pack it pulls, and
`packs normalize` for the packs already in `packs/`. Doing it twice changes nothing: a folder
already at its pack's shape, in its place, is never redrawn, and a file that already holds what
it would get isn't written.

| command | an outlier |
|---|---|
| `packs make` | left out of the pack, and listed. `--keep-outliers` keeps it as it is |
| `packs normalize` | listed, and left as it is. `--drop-outliers` takes it out of `pack.json` and deletes its picture |
| `community pull` | kept as it is, with a warning in the output that the Packs workflow's log shows. A skin someone shared is never dropped without a person deciding |

```sh
cargo run -p folderskin-tools -- packs normalize --dir ../folderskin-community
cargo run -p folderskin-tools -- packs normalize --dir ../folderskin-community classic-art-5rxas2 --tolerance 0.3
cargo run -p folderskin-tools -- packs normalize --dir ../folderskin-community dreamscapes-ppfia6 --drop-outliers
```

`packs normalize` goes through every pack, or the ones named. For each it says the shape and
what it redrew, and names every outlier with how far off it is. A pack that doesn't pass
`packs check` is left alone until it does. `--tolerance` changes the 8%, for a pack whose
outliers should take its shape anyway, as the Mona Lisa does in Classic Art. `--drop-outliers`
is for the maintainer: nothing else ever removes a skin. `packs check --require-one-shape` turns
down a pack whose folders are more than 1% apart ([Checking a pack
yourself](#checking-a-pack-yourself)). Folders redrawn at one shape never are.

### Licences

Shared skins use Creative Commons or MIT:

- `CC0-1.0`: anyone can use them for anything. This is the default, since most skins are made
  with AI and CC0 claims the least over them.
- `CC-BY-4.0`: anyone can use them, and credits you.
- `MIT`: anyone can use them, and your name stays with them.

Share only pictures you made or are allowed to share.

## Making a pack from pictures

`packs make` turns a folder of pictures, such as renders saved from an image model, into a pack
under `packs/` in a checkout of folderskin-community that already passes the checks. The commands
below run from this repository, with folderskin-community checked out beside it:

```sh
cargo run -p folderskin-tools -- packs make ~/Downloads/3d-renders --dir ../folderskin-community \
  --name "3D" --tags 3d,glossy --author your-github-name --preview /tmp/3d.png
```

The pack gets an id of its own, its name and six random characters such as `3d-k7q2mx`, and
that is its folder's name. The report says what it is. The id is never one a folder in `packs/`
has, or an old id in `moved.json`.

Each picture gets the split the app makes when you add one. A finished folder, painted on
magenta the way the chat prompt in [PROMPTS.md](PROMPTS.md) asks, or on real transparency, is cut
out and becomes the icon itself. Anything else is artwork for FolderSkin's folder. Every picture
is shrunk to 1024 px and saved as a lossless WebP, with the encoder the app shares packs with
built in, so there is nothing to install. One still over 1.5 MB is made 896 px, then 768 px, and
the report says so. `--max-kb` holds the pictures to less than 1.5 MB. Pictures that come to more
than 64 MB together are turned down, with a suggestion to split them into two packs. libwebp's
most thorough setting takes several seconds a picture, so they're made on every core at once.
[SKINS.md](SKINS.md#pictures-for-a-pack) says more about the formats. The report says which way
each picture went. Skins are named after their files, so name the files first or fix the names
in `pack.json` afterwards, and `--preview` draws every skin as its folder in one PNG to look
over.

Two finished folders or more are given one shape ([One shape for a pack's
folders](#one-shape-for-a-packs-folders)), and the report says which were redrawn. A folder more
than 8% off the others' shape is left out, and the report says how far off it is.
`--keep-outliers` keeps it as it is instead. A pack made with no outlier kept passes
`packs check --require-one-shape`.

`--id` makes a pack that is there already again, from new pictures: `--id 3d-k7q2mx` replaces
everything in `packs/3d-k7q2mx/`, and the pack keeps its id, so everyone who added it gets the new
version as an update. The new folder is made and checked on the side first, and takes the old
one's place only when it passes. Without `--id`, `packs make` always makes a new pack.

Image models asked for `#FF00FF` often paint a steady raspberry or hot pink instead (Grok did,
for the Classic Art pack). `--flat-backdrop` cuts away a flat background of any colour: it
measures each picture's own background, removes only what reaches the edge (so a red cloak
inside the folder stays), takes a soft drop shadow with it, and gives the edge the painting's
colours rather than a pink rim. On a plain grey or black background it keeps to that
background's own noise and never grows upward, so a dark coat or an ink line meeting the folder's
edge isn't mistaken for background. Look at the `--preview` sheet afterwards (it's drawn on light
grey, so a hole shows). A picture with no flat background still comes out as artwork.

To see one picture as the app will show it, `render` draws it as its folder:

```sh
cargo run -p folderskin-tools -- render ../folderskin-community/packs/3d-k7q2mx/glass.webp --out /tmp/glass.png --size 512
```

## Checking a pack yourself

From this repository, with folderskin-community checked out beside it:

```
cargo run -p folderskin-tools -- packs check --dir ../folderskin-community
```

It checks every folder in `packs/` with the rules the app uses, and prints each problem as a
sentence. Run inside a folderskin-community checkout, `--dir` can be left out: the tools look in the
current folder by default. It holds each picture to the 2 MB the packs published before 0.1.7 were
made under, and each pack's pictures to 64 MB together. `--require-lossless` holds every picture
to the rules new packs follow: PNG or lossless WebP, at most 1.5 MB, which is what the community
service takes and `packs make` writes. It's off unless asked, while the packs from before are made
again. `--max-kb` holds the pictures to less.

It also checks the ids between them: two folders whose names differ only in capitals are one
folder on macOS and Windows, and a pack can't take an old id from `moved.json`, which has to
follow its own rules ([moved.json](#movedjson)). Names are never compared, since they can repeat.
`--require-generated-ids` turns down any pack whose id isn't a generated one. It's off unless
asked, and folderskin-community's workflow asks when its variable `REQUIRE_GENERATED_IDS` is
`true`.

`--require-one-shape` turns down a pack whose finished folders aren't one shape: two of them more
than 1% apart, width ÷ height. Folders redrawn at one shape are always within it, so it catches a
pack that was never given one, and an outlier someone kept. The problem names the two furthest
apart and says what to run ([One shape for a pack's folders](#one-shape-for-a-packs-folders)).
It's off unless asked, and meant for folderskin-community's workflow.

## How the app reads packs

Community has a **List** and a **Gallery** view, and **View** on any pack opens it: every skin
drawn as the folder it makes, with its name, before anything is added. **Refresh** reads the list
again. A pack you added that has changed since shows **Update**, which swaps its skins for the
new version. Folders keep their icons, and a favourite of a picture both versions share stays a
favourite.

- `index.json` in folderskin-community lists every pack: its id, name, author, licence, tags, number of skins
  and a hash of its exact contents (`pack.json` and every picture). The app keeps the hash with
  the skins it adds, which is how it knows a pack has an update. Each entry also says when the
  pack was first published (`"added"`, in Unix seconds: the time of the commit that added its
  `pack.json`, under its first id) and, for a pack `official.json` lists, `"official": true`.
  `"moved"` beside the packs is [moved.json](#movedjson). `folderskin-tools packs index` writes it, together with
  `previews/<id>.png`, a strip of the pack's first four skins drawn as folders. Both are
  generated on folderskin-community's `main`, so never edit them by hand.
- `v2/`, which `packs catalog` writes, is the same packs as a catalog the app searches on your
  computer. Its `head.json` names the current catalog, lists the `featured` and `official`
  packs, carries `moved`, and lists the mirrors that serve the same tree, such as
  `https://packs.folderskin.app` ([The mirror](#the-mirror)). The app fetches each file from the
  mirrors first and from GitHub when they fail, and checks every one against its hash either way.
  From 0.1.7 it reads `head.json` itself from `https://packs.folderskin.app` first.
- The app downloads a pack's pictures only when you add it, four at a time, and shows how many
  have arrived. It checks every one against the limits above and saves nothing unless all of
  them pass. Then it saves them together, so a pack is never half added.
- `FOLDERSKIN_COMMUNITY_URL` points the app at another copy of folderskin-community. For example,
  serve a checkout with `python3 -m http.server` from its root and set it to
  `http://localhost:8000` to try a pack end to end.

`index.json` and `head.json` gain fields over time, and every version of the app reads the ones
it knows and passes over the rest. `pack.json` is the opposite: it takes no field the contract
doesn't name, so nothing may ever be added to it. Anything new about a pack goes in the index.

## Featured and official packs

Two lists sit beside `packs/` at the root of folderskin-community, and only its maintainer edits
them. Each is a JSON list of pack ids, such as `["classic-art-k7q2mx", "colours-a2b3c4"]`:

| file | what it does |
|---|---|
| `featured.json` | the packs the first launch offers, and Community shows first, in this order |
| `official.json` | the packs marked **Official** in Community, in a pack's viewer and on the website |

Both are optional. Every id has to be a pack in `packs/`, and one listed twice counts once.
`packs index` and `packs catalog` stop and write nothing when either names a pack that isn't
there, so a pack that is renamed or removed can't leave a gap. The pull request check runs
`packs catalog`, which catches it before a merge. `packs rename` rewrites both lists itself. The
Packs workflow in folderskin-community rebuilds the index when a file in its `paths` changes, so
both lists belong there beside `packs/**`, and so does `moved.json`.

## The install link

`folderskin://install?pack=<id>` opens FolderSkin on pack `<id>` in Community and adds it,
exactly as its **Add** button does, with the same progress and the same message at the end. The
window comes to the front first. If Community has no pack with that id, even after asking
GitHub again, FolderSkin says so and suggests searching for it. A pack already in your library
is opened and said to be there. From 0.1.7, a link with an old id opens the pack it moved to
([moved.json](#movedjson)).

FolderSkin takes a link only when it is exactly that: the scheme `folderskin`, `install` as the
host (`folderskin://install?…`) or the whole path (`folderskin:install?…`), no user, password or
port, and exactly one `pack`, which has to be a pack id (lower-case letters and digits in words
joined by single dashes, at most 40 characters). Any other parameter is passed over, and anything
else is ignored.

The installers register the scheme: the macOS app's `Info.plist`, the Windows installers, and
the desktop entry of the Linux `.deb` and `.rpm`. An AppImage registers it as it starts, since
nothing installs it. On Windows and Linux a link starts a second FolderSkin, which hands the link
to the one already running and quits, so only one ever runs.

To try it:

| | |
|---|---|
| macOS | Build the app (`pnpm tauri build --bundles app`) and open `target/release/bundle/macos/FolderSkin.app` once, which registers the scheme with macOS (a copy in `/Applications` is the surest), then `open 'folderskin://install?pack=classic-art'`. macOS sends links only to a bundled app, so `pnpm tauri dev` never receives one |
| Windows | Install a build, or run `pnpm tauri dev` (a development build registers the scheme for itself), then `start "" "folderskin://install?pack=classic-art"` in a command prompt, or the same link in the Run box (Windows+R) |
| Linux | Install the `.deb` or `.rpm`, start the AppImage once, or run `pnpm tauri dev`, then `xdg-open 'folderskin://install?pack=classic-art'` |
| browser preview | `pnpm dev` and open `http://localhost:14200/?install=classic-art` |

Quit an installed FolderSkin before `pnpm tauri dev` on Windows or Linux: with one already
running, the new one hands over to it and quits. A development build that registered the scheme
keeps it until an installer or another build registers it again.

## Install counts

When a pack from Community has been added, FolderSkin tells its community service the pack's
id: `POST https://community.folderskin.app/v1/packs/<id>/installs`, with no body. That is all it
sends: no account, no device id, nothing about your library or your folders (like every request
FolderSkin makes, it names the app's version in its User-Agent). It happens after the pack is
saved, gives up after five seconds, and nothing waits for it or reports a failure. A development
build, and one reading packs from another copy (`FOLDERSKIN_COMMUNITY_URL`), send nothing unless
`FOLDERSKIN_COMMUNITY_API` names a service to send to.

The service counts an add once a day for each network and pack, and only for packs in the
published `index.json`. An add under an old id counts for the pack it moved to, so apps from
before 0.1.7 still count. It keeps a count per pack and, for the rest of that UTC day, a salted
hash of the network the request came from, so adding the same pack again that day doesn't count
twice. The daily clean-up deletes those hashes. No address is stored.

folderskin.app reads the counts from `GET https://community.folderskin.app/v1/packs/installs`:
`{"version": 1, "installs": {"classic-art-k7q2mx": 42}}`, cached for five minutes.
[services/community/README.md](../services/community/README.md) has the details.
