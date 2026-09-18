# Skins

A skin is one image. FolderSkin cuts it into the folder template twice — once for the back
panel with the tab, once for the front panel — so a picture that looks fine on its own can
still land badly on a folder. This page is the format, the safe areas, and the commands
that let you check a skin instead of guessing.

If you want an agent to do the work, [.claude/skills/folderskin-skins/SKILL.md](../.claude/skills/folderskin-skins/SKILL.md)
walks Claude Code through the same loop.

## The format

| property | value |
|---|---|
| size | 1024 × 958 px, exactly |
| aspect | 1.069 (the folder's own aspect) |
| formats | PNG, JPEG or WebP |
| file size | 400 KB or less per skin; the whole set should stay under 2.5 MB |
| colour | sRGB, no alpha needed (the template supplies the shape) |
| location | `assets/skins/<id>.<ext>`, listed in `assets/skins/manifest.json` |

Skins are embedded into the binary at build time by
[src-tauri/build.rs](../src-tauri/build.rs), so **adding or changing a skin means
rebuilding the app**. Editing the file on disk does nothing to an app that is already
built.

## Safe areas

The compositor cover-fits the same image into two rectangles of the 1024 × 1024 icon
canvas ([crates/folderskin-core/src/geometry.rs](../crates/folderskin-core/src/geometry.rs)
holds the constants):

- **Back panel** — the part with the tab, spanning y 36.5 to 973.5. It is very close to the
  skin's own aspect, so the *whole image height* is shown here and about 18 px is trimmed
  from each side (1.8%).
- **Front panel** — the paper's cover, spanning y 160.5 to 973.5 with a 55 px corner
  radius. It is wider than the skin, so the image is scaled to the panel's width and the
  middle ~85% of its height survives: roughly 6% is cropped off the top and the same off the
  bottom, centred on the focus point.

Read as bands of the source image. The bands overlap because the two panels show
overlapping parts of the same picture: the back panel shows the whole height, the front panel
shows the middle.

| source rows (of 958) | share of the height | where it ends up |
|---|---|---|
| 0 – 62 | top 6.5% | inside the tab |
| 62 – 127 | next 6.7% | the strip of back panel above the front panel, beside the paper sheet |
| 60 – 898 | middle ~87% | the front panel, the part people actually look at |
| 898 – 958 | bottom 6.3% | cropped away (at focus `0.5`) |

Two rules follow:

1. **Nothing important in the top 12%.** Those rows are the tab and the thin strip above the
   front panel. A face, a horizon or a logo up there is cut into two pieces by the paper
   sheet.
2. **Keep the subject in the middle.** The front panel drops about 60 px from the top and
   the bottom, and its corners are rounded by 55 px at 1024 px, so detail in the extreme
   corners disappears at small icon sizes anyway.

Flat and abstract art survives all of this without any thought. Photographs with one clear
subject need the focus point.

## Focus points

`focus` is `[x, y]` in 0..1, in image space. `[0.5, 0.5]` is the centre.

- **`y` is the one that matters.** It slides the 838-row window the front panel keeps. The
  whole travel is only 120 px of source, so `0.1` of focus y moves the crop by about 12 px:
  `[0.5, 0.35]` keeps a little more sky, `[0.5, 0.7]` keeps more foreground.
- **`x` barely does anything.** The front panel is width-fitted, so there is no horizontal
  slack at all; `x` only shifts the back panel's 18 px side trim.

If a subject cannot be saved by focus alone, crop the picture yourself before importing it.
The tool crops to aspect, it does not compose.

## Adding a skin

All commands run from the repository root. The tool is a workspace binary, so no install
step:

```sh
# 1. import a picture: crops to 1024×958 around the focus point, writes the file,
#    the manifest entry and a preview of the finished icon
cargo run -p folderskin-tools -- skin add ~/Pictures/dunes.jpg \
  --id dunes --name Dunes --collection grain --focus 0.5,0.45

# 2. look at assets/previews/dunes.png, then rerun step 1 with a different --focus
#    until the composition is right (the same --id overwrites, it never duplicates)

# 3. validate the manifest, the dimensions and the byte budget
cargo run -p folderskin-tools -- skin check

# 4. rebuild, because skins are embedded at build time
pnpm tauri build
```

Other subcommands that help:

| command | what it does |
|---|---|
| `skin gen` | regenerates the ten built-in skins, the manifest and the previews, deterministically |
| `skin guide --out guide.png` | writes a 1024 × 958 template marked with the tab, the paper strip, the front panel and the rows the front panel crops — open it in an image editor as a layer over your artwork |
| `skin check` | reports duplicate or malformed ids, missing files, wrong dimensions, oversized files and the total byte count; exits non-zero on a problem |
| `render <image> --out out.png [--size N] [--focus x,y]` | renders the finished folder icon from any image without touching the manifest |
| `render --solid RRGGBB --out out.png` | the same for a flat colour, useful for checking the template itself |
| `apply <folder> --skin <id>` / `apply <folder> --image <path>` / `revert <folder>` | applies or reverts an icon from a terminal, without the GUI |

`--lossless` on `skin add` keeps PNG instead of encoding JPEG.

## Encoding

`skin add` writes JPEG at quality 90 by default. Which format is smaller depends entirely
on the art:

| art | format | why |
|---|---|---|
| photographs, gradients, noise, grain | JPEG q90 | 80–250 KB; the grain would blow up a PNG |
| flat colour, hard-edged shapes, halftone dots, stripes | PNG | palette-like data compresses well and JPEG rings around hard edges |
| art you want stored losslessly | PNG (`--lossless`) | no JPEG ringing; note the tool flattens to RGB, so transparency in the source is not preserved — the template supplies the folder shape |

Try both when a skin is near the 400 KB limit and keep the smaller file: `skin add` encodes
both and keeps the smaller one unless you pass `--lossless`. The built-in set does exactly
this — nine recipes ship as JPEG and `halftone`, whose hard dots compress best losslessly,
ships as PNG.

## The manifest

`assets/skins/manifest.json` is the list the app and the build script read:

```json
{
  "version": 1,
  "skins": [
    {
      "id": "aurora",
      "name": "Aurora",
      "collection": "glow",
      "file": "aurora.jpg",
      "focus": [0.5, 0.5],
      "author": "FolderSkin",
      "license": "CC0-1.0"
    }
  ]
}
```

| field | rules |
|---|---|
| `version` | `1` |
| `id` | lowercase letters, digits and dashes; unique; also the preview's filename |
| `name` | what the gallery shows, title case |
| `collection` | `glow`, `grain` or `pop` — these are the gallery's tabs |
| `file` | filename relative to `assets/skins/` |
| `focus` | `[x, y]` in 0..1; optional, defaults to `[0.5, 0.5]` |
| `author` | credit; free text |
| `license` | SPDX id, e.g. `CC0-1.0` or `MIT` |

`Manifest::validate` in
[crates/folderskin-core/src/manifest.rs](../crates/folderskin-core/src/manifest.rs) is the
authority on all of this, and `skin check` is that function with a command around it.

## The shipped set

Ten skins, three collections. The gallery's tabs are `all`, the three collections and
`faves`.

| collection | skins |
|---|---|
| `glow` | `aurora`, `sunset`, `mesh`, `ember` |
| `grain` | `paper`, `denim`, `slate` |
| `pop` | `halftone`, `stripes`, `bubbles` |

All ten are procedural art generated by `skin gen` from a seeded PRNG, so the repository can
reproduce them byte for byte; they are CC0. Keep the shipped set at exactly ten — replace a
skin rather than adding an eleventh — unless a maintainer decides otherwise. `skin check`
reports a problem whenever the count is not exactly ten.

## Licensing what you add

Only commit art you made yourself or art that is CC0. Anything else — stock photos,
wallpapers, screenshots of someone's work, model output whose licence you have not read —
stays out of the repository. Fill in `author` and `license` honestly; a skin with an empty
`license` will not be accepted.

Users are of course free to drop any picture they like onto the app. That stays on their
machine and never touches the manifest.
