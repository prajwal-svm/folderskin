# Community skins and packs

Anyone can share skins with everyone who uses FolderSkin, for free. A shared set of skins is a
**pack**; one skin on its own is a pack of one. Packs live in this repository under
`community/packs/` and reach the app straight from GitHub, so there are no accounts and no
server of ours in between.

## Adding a pack

Open **Community** in the app. The filters at the top are the packs' tags. **Add** puts a
pack's skins in your library, carrying the pack's tags, and each skin's ⋯ menu says which pack it
came from and who shared it. **Remove** takes the whole pack out again. Folders that already use
one of its skins keep their icon, because the icon lives in the folder itself.

**Add from a folder** does the same for a pack folder on your computer, which is also how you try
a pack out before sharing it.

## Sharing yours

From the app:

1. Tag the skins you want to share (⋯ → Tags). To share one skin, use ⋯ → **Share with
   community**. To share several, use **Community → Share your skins** and pick a tag.
2. Fill in the pack's name, its tags, your GitHub user name and a licence, then press
   **Save pack…**. FolderSkin writes a folder that already follows every rule below.
3. Press **Open GitHub** and sign in. GitHub makes you a copy of the repository to add to.
4. Drag the pack folder onto the page, then choose **Propose changes** and **Create pull
   request**.

The pull request runs the same checks the app runs. Once a maintainer merges it, a workflow
rebuilds `community/index.json` and the preview, and the pack appears in everyone's Community
view.

You can also make a pack by hand: follow the contract below and open a pull request that adds
one folder under `community/packs/`.

## The contract

A pack is one folder:

```
community/packs/night-prints/
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
| `skins` | 1 to 24 entries |
| `skins[].file` | a picture in the folder |
| `skins[].name` | 1 to 60 characters |
| `skins[].tags` | optional, up to 3 more for that skin |

No other fields are allowed, so a typo such as `"tag"` fails the check instead of being ignored.

### Limits

| | limit |
|---|---|
| skins in a pack | 1 to 24 |
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

FolderSkin is MIT licensed, and shared skins use Creative Commons or MIT:

- `CC0-1.0`: anyone can use them for anything. This is the default, since most skins are made
  with AI and CC0 claims the least over them.
- `CC-BY-4.0`: anyone can use them, and credits you.
- `MIT`: the same licence as FolderSkin's code; your name stays with them.

Share only pictures you made or are allowed to share.

## Checking a pack yourself

From the repository root:

```
cargo run -p folderskin-tools -- packs check
```

It checks every folder in `community/packs/` with the rules the app uses, and prints each
problem as a sentence. `--dir` points it at another copy of `community/`.

## How the app reads packs

- `community/index.json` lists every pack: its id, name, author, licence, tags and number of
  skins. `folderskin-tools packs index` writes it, together with
  `community/previews/<id>.png`, a strip of the pack's first four skins drawn as folders. Both
  are generated on `main`; never edit them by hand.
- The app downloads a pack's pictures only when you press **Add**. It checks every one against
  the limits above and saves nothing unless all of them pass.
- `FOLDERSKIN_COMMUNITY_URL` points the app at another copy of `community/`. For example,
  serve a checkout with `python3 -m http.server` and set it to
  `http://localhost:8000/community` to try a pack end to end.
