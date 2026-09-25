# Community skins and packs

Anyone can share skins with everyone who uses FolderSkin, for free. A shared set of skins is a
**pack**; one skin on its own is a pack of one. Packs live in their own repository,
[folderskin-community](https://github.com/prajwal-svm/folderskin-community), under `packs/`, and reach the
app straight from GitHub, so there are no accounts and no server of ours in between. FolderSkin ships no skins of its own: packs, your own pictures and AI
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

From the app:

1. Tag the skins you want to share (⋯ → Tags). To share one skin, use ⋯ → **Share with
   community**. To share several, use **Community → Share your skins** and pick a tag.
2. Fill in the pack's name, its tags, your GitHub user name and a licence, say where the pictures
   came from, and tick that they are yours to share.
3. Press **Connect and publish**. The first time, FolderSkin shows a short code to copy and opens
   <https://github.com/login/device>; approve it once and it remembers.
4. That is the end of it. FolderSkin checks the pack against the contract below, makes you a copy
   of folderskin-community if you can't push to it, puts the pack on a branch in a single commit, and
   opens the pull request there as you, with your answers in the description.

The pull request runs the same checks the app just ran. Once a maintainer merges it, a workflow
in folderskin-community rebuilds `index.json` and the preview, and the pack appears in everyone's
Community view.

FolderSkin asks GitHub for `public_repo`, which is enough to fork a public repository, push to
your own fork and open a pull request. It never asks to see a private repository. The sign-in is
sealed on disk in FolderSkin's own folder, the same way API keys are ([keys.rs](../src-tauri/src/keys.rs)),
and **Use another account** forgets it. To take the permission back entirely, remove FolderSkin
under Settings → Applications on GitHub.

**Save a folder** still does what it always did, for anyone who would rather handle GitHub
themselves: it writes a folder that follows every rule below, ready to drag onto a pull request.
You can also make a pack by hand: follow the contract and open a pull request to
[folderskin-community](https://github.com/prajwal-svm/folderskin-community) that adds one folder under
`packs/`.

## The contract

A pack is one folder:

```
packs/night-prints/
  pack.json
  koi.png
  fox-in-the-rain.jpg
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
    { "file": "koi.png", "name": "Koi over the wave", "tags": ["animals"] },
    { "file": "fox-in-the-rain.jpg", "name": "Fox in the rain" }
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
| each picture | PNG, JPEG or WebP, at most 2 MB |
| picture sides | 256 to 1024 px |
| file names | letters, digits, `.`, `-` and `_`, ending in `.png`, `.jpg`, `.jpeg` or `.webp` |
| folder name | lower-case letters and digits in words joined by single dashes, at most 40 characters |
| tags | lower case letters, digits, spaces and dashes, at most 24 characters |
| `pack.json` | at most 64 KB |

Why 24: a pack is a themed set. Twenty-four skins fill about two screens of the library, stay
quick to review on GitHub, and keep a download under 48 MB even at the largest pictures.

### Pictures

Each picture is one of two kinds, told apart the same way as a picture dropped on the window:

- **A finished folder** on a transparent background, or on flat magenta `#FF00FF`, which
  FolderSkin cuts away. It becomes the icon exactly as drawn.
- **Anything else** is wrapped onto FolderSkin's folder. [SKINS.md](SKINS.md) shows where the
  folder crops a picture, so the subject survives.

1024 px is the largest icon any of the three systems draws, so a bigger picture adds nothing.

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
  --id 3d --name "3D" --tags 3d,glossy --author your-github-name --preview /tmp/3d.png
```

Each picture gets the split the app makes when you add one. A finished folder, painted on
magenta the way the chat prompt in [PROMPTS.md](PROMPTS.md) asks, or on real transparency, is cut
out and becomes the icon itself; anything else is artwork for FolderSkin's folder. Every picture
is shrunk to 1024 px and compressed to at most 400 KB (`--max-kb` changes that), since a pack is
downloaded by everyone who adds it: folders as WebP when `cwebp` is installed (`brew install
webp`, or the `webp` package on Linux), which keeps the transparency at a fraction of a PNG's
size, and artwork as JPEG. [SKINS.md](SKINS.md#pictures-for-a-pack) says more about the formats.
The report says which way each picture went. Skins are named after their files, so name the files
first or fix the names in `pack.json` afterwards, and `--preview` draws every skin as its folder in
one PNG to look over. `packs make` never overwrites a pack: to start again, delete its folder.

Image models asked for `#FF00FF` often paint a steady raspberry or hot pink instead (Grok did,
for the Classic Art pack). `--flat-backdrop` cuts away a flat background of any colour: it
measures each picture's own background, removes only what reaches the edge (so a red cloak
inside the folder stays), takes a soft drop shadow with it, and gives the edge the painting's
colours rather than a pink rim. On a plain grey or black background it keeps to that
background's own noise and never grows upward, so a dark coat or an ink line meeting the folder's
edge isn't mistaken for background. Look at the `--preview` sheet afterwards (it's drawn on light
grey, so a hole shows); a picture with no flat background still comes out as artwork.

To see one picture as the app will show it, `render` draws it as its folder:

```sh
cargo run -p folderskin-tools -- render ../folderskin-community/packs/3d/glass.webp --out /tmp/glass.png --size 512
```

## Checking a pack yourself

From this repository, with folderskin-community checked out beside it:

```
cargo run -p folderskin-tools -- packs check --dir ../folderskin-community
```

It checks every folder in `packs/` with the rules the app uses, and prints each problem as a
sentence. Run inside a folderskin-community checkout, `--dir` can be left out: the tools look in the
current folder by default. `--max-kb` holds the pictures to a smaller size than the 2 MB limit.

## How the app reads packs

Community has a **List** and a **Gallery** view, and **View** on any pack opens it: every skin
drawn as the folder it makes, with its name, before anything is added. **Refresh** reads the list
from GitHub again. A pack you added that has changed on GitHub since shows **Update**, which
swaps its skins for the new version; folders keep their icons, and a favourite of a picture both
versions share stays a favourite.

- `index.json` in folderskin-community lists every pack: its id, name, author, licence, tags, number of skins
  and a hash of its exact contents (`pack.json` and every picture). The app keeps the hash with
  the skins it adds, which is how it knows a pack has an update. Each entry also says when the
  pack was first published (`"added"`, in Unix seconds: the time of the commit that added its
  `pack.json`) and, for a pack `official.json` lists, `"official": true`. `folderskin-tools packs index` writes it, together with
  `previews/<id>.png`, a strip of the pack's first four skins drawn as folders. Both are
  generated on folderskin-community's `main`; never edit them by hand.
- `v2/`, which `packs catalog` writes, is the same packs as a catalog the app searches on your
  computer. Its `head.json` names the current catalog and lists the `featured` and `official`
  packs.
- The app downloads a pack's pictures only when you add it, four at a time, and shows how many
  have arrived. It checks every one against the limits above and saves nothing unless all of
  them pass; then it saves them together, so a pack is never half added.
- `FOLDERSKIN_COMMUNITY_URL` points the app at another copy of folderskin-community. For example,
  serve a checkout with `python3 -m http.server` from its root and set it to
  `http://localhost:8000` to try a pack end to end.

`index.json` and `head.json` gain fields over time, and every version of the app reads the ones
it knows and passes over the rest. `pack.json` is the opposite: it takes no field the contract
doesn't name, so nothing may ever be added to it. Anything new about a pack goes in the index.

## Featured and official packs

Two lists sit beside `packs/` at the root of folderskin-community, and only its maintainer edits
them. Each is a JSON list of pack ids, such as `["classic-art", "colours"]`:

| file | what it does |
|---|---|
| `featured.json` | the packs the first launch offers, and Community shows first, in this order |
| `official.json` | the packs marked **Official** in Community, in a pack's viewer and on the website |

Both are optional. Every id has to be a pack in `packs/`, and one listed twice counts once.
`packs index` and `packs catalog` stop and write nothing when either names a pack that isn't
there, so a pack that is renamed or removed can't leave a gap; the pull request check runs
`packs catalog`, which catches it before a merge. The Packs workflow in folderskin-community
rebuilds the index when a file in its `paths` changes, so both lists belong there beside
`packs/**`.

## The install link

`folderskin://install?pack=<id>` opens FolderSkin on pack `<id>` in Community and adds it,
exactly as its **Add** button does, with the same progress and the same message at the end. The
window comes to the front first. If Community has no pack with that id, even after asking
GitHub again, FolderSkin says so and suggests searching for it. A pack already in your library
is opened and said to be there.

FolderSkin takes a link only when it is exactly that: the scheme `folderskin`, `install` as the
host (`folderskin://install?…`) or the whole path (`folderskin:install?…`), no user, password or
port, and exactly one `pack`, which has to be a pack id (lower-case letters and digits in words
joined by single dashes, at most 40 characters). Any other parameter is passed over; anything else
is ignored.

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
published `index.json`. It keeps a count per pack and, for the rest of that UTC day, a salted hash
of the network the request came from, so adding the same pack again that day doesn't count
twice. The daily clean-up deletes those hashes. No address is stored.

folderskin.app reads the counts from `GET https://community.folderskin.app/v1/packs/installs`:
`{"version": 1, "installs": {"classic-art": 42}}`, cached for five minutes.
[services/community/README.md](../services/community/README.md) has the details.
