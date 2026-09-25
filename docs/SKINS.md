# How a picture becomes a folder icon

Every skin is one picture: one you added, one an AI model painted, or one from a community pack.
FolderSkin turns it into a folder icon in one of two ways, and a picture that looks fine on its
own can still land badly on a folder. This page is how FolderSkin decides, what the folder does to
a picture, and how to check one before you share it in a pack ([PACKS.md](PACKS.md)).

If you want an agent to do the work, [.claude/skills/folderskin-skins/SKILL.md](../.claude/skills/folderskin-skins/SKILL.md)
walks Claude Code through making a pack.

## Artwork or a finished folder

FolderSkin looks at what surrounds the picture:

- **A finished folder** sits on real transparency, or on flat magenta `#FF00FF`. The magenta is
  cut away, the picture is trimmed to the folder, and it becomes the icon exactly as drawn: scaled
  to fit the icon and centred, never cropped. This is what an image model paints from the prompts
  in [PROMPTS.md](PROMPTS.md).
- **Anything else is artwork**: a photo, a painting, a pattern. FolderSkin wraps it onto its own
  folder template, so it gets the same tab, paper and outline as every other skin.

The magenta has to be flat `#FF00FF` or very close to it, so a product shot on magenta paper or a
vivid pink sunset stays a picture. [ARCHITECTURE.md](ARCHITECTURE.md#artwork-or-a-finished-folder)
has the exact rules. The rest of this page is about artwork.

## Safe areas

The compositor cover-fits the same picture into two rectangles of the 1024 × 1024 icon canvas
([crates/folderskin-core/src/geometry.rs](../crates/folderskin-core/src/geometry.rs) holds the
constants):

- **Back panel**: the part with the tab, spanning y 36.5 to 973.5. It is very close to the aspect
  of a 1024 × 958 picture, so the *whole height* is shown here and about 18 px is trimmed from
  each side (1.8%).
- **Front panel**: the paper's cover, spanning y 160.5 to 973.5 with a 55 px corner radius. It is
  wider than the picture, so the picture is scaled to the panel's width and the middle ~87% of
  its height survives: roughly 6% is cropped off the top and the same off the bottom.

Read as bands of a 1024 × 958 picture (a picture of the same shape at another size scales the
same way). The bands overlap because the two panels show overlapping parts of the same picture:
the back panel shows the whole height, the front panel the middle.

| rows (of 958) | share of the height | where it ends up |
|---|---|---|
| 0 – 62 | top 6.5% | inside the tab |
| 62 – 127 | next 6.7% | the strip of back panel above the front panel, beside the paper sheet |
| 60 – 898 | middle ~87% | the front panel, the part people actually look at |
| 898 – 958 | bottom 6.3% | cropped away |

Two rules follow:

1. **Nothing important in the top 12%.** Those rows are the tab and the thin strip above the
   front panel. A face, a horizon or a logo up there is cut into two pieces by the paper sheet.
2. **Keep the subject in the middle.** The front panel drops about 60 px from the top and the
   bottom, and its corners are rounded by 55 px at 1024 px, so detail in the extreme corners
   disappears at small icon sizes anyway.

Flat and abstract art survives all of this without any thought. A photograph with one clear
subject needs it in the middle band. FolderSkin keeps artwork centred, so if the subject sits
high or low, crop the picture yourself before adding it or putting it in a pack.

`folderskin-tools guide` draws these bands:

```sh
cargo run -p folderskin-tools -- guide --out /tmp/guide.png
```

It writes a 1024 × 958 template marked with the tab, the paper strip, the front panel and the rows
the front panel crops. Open it in an image editor as a layer over your picture.

## Checking a picture

`render` draws any picture as the folder the app makes of it, through the same code, and says
which of the two it is:

```sh
cargo run -p folderskin-tools -- render ~/Pictures/koi.webp --out /tmp/koi.png --size 512
# wrote /tmp/koi.png (512×512): artwork on FolderSkin's folder
```

Look at the result before you share the picture. `--focus x,y` moves the crop of artwork, which
is a quick way to see how much of a picture a different crop would keep; the app itself always
crops around the centre, so bake the crop you want into the picture. `render --solid RRGGBB`
draws the template in a flat colour, to look at the template itself.

## Pictures for a pack

A pack's pictures are shared losslessly, so a pack looks exactly as you made it.
[PACKS.md](PACKS.md) has the limits: at most 1024 px on a side (the largest icon any of the three
systems draws), at least 256, at most 1.5 MB each and 64 MB for the whole pack. The app and
`packs make` shrink and encode every picture for you, as a lossless WebP:

| picture | format | why |
|---|---|---|
| a finished folder | lossless WebP, with its transparency | every pixel as drawn, the edge included, at about two thirds of a PNG's size |
| artwork: photographs, paintings, gradients, grain | lossless WebP | no blocks or ringing; a detailed 1024 px picture comes to about 800 KB |
| either, too detailed for 1.5 MB at 1024 px | lossless WebP at 896, then 768 px | smaller rather than blurred, and the app says which |

Encoding by hand, a PNG is fine too, and so is `cwebp -lossless -z 9`. JPEG and lossy WebP
aren't taken for a new pack: the community service turns them down, and so does `packs check
--require-lossless`. Artwork has no transparency to keep, since the template supplies the
folder's shape. Artwork at 1024 × 958 loses the least to the crop, but any size works.

## Licensing

Share only pictures you made or are allowed to share, under one of the licences a pack can use
([PACKS.md](PACKS.md#licences)). Anything else, such as stock photos, wallpapers, screenshots of
someone's work or model output whose terms you have not read, stays on your own computer: drop it
on the app to use it there, where it never leaves your machine.
