# Design your own: the composer

**Design your own**, in the sidebar, is a canvas for making a skin yourself, from a single colour
to a heavily layered design. It works entirely offline. What you design is saved to **Yours** like
any other skin, and **Save & apply** puts it straight on the folder you picked.

![The composer: a bug from the icon library pressed into a blue folder, with the icon search beside it and Save & apply below](images/composer.webp)

## What people make with it

| You want | Start from | Then |
| --- | --- | --- |
| A folder in one colour, any colour | **Colour** or **Plain** | Click the folder, pick the colour. The picker takes any colour, with transparency |
| A labelled folder: *Recipes*, *Notes 2026* | **Label** | Type the words, then pick a font, weight and colour. Short words read best at small sizes |
| A folder that says what's inside with one picture | **Emoji** | Double-click the emoji to swap it, or search the picker (*dog*, *plane*, *receipt*) |
| A label on the tab itself | **Tab label** | The **Tab** button in *Place* puts any layer on the tab |
| A back and a front in different colours | **Two-tone** | The split is hidden behind the paper sheet |
| A see-through folder | **Glass** or **Tinted glass** | Lower the background's opacity, or delete it for a clear folder: only the paper and the edges stay |
| A patterned folder | **Stripes**, **Gingham**, **Polka** | **Pattern** adds ten kinds, from stripes to confetti and film grain |
| A photo with a caption | **Photo** | It asks for the picture first. Drop or paste more later |
| A sticker, a badge, anything that isn't folder-shaped | **Sticker** | *Free icon* makes the design the whole icon |
| A skin you already have, with your own touch | A skin's ⋯ menu → **Remix in the composer** | The skin becomes a picture layer to build on |
| A change to something you designed before | Its ⋯ menu → **Edit design** | **Save changes** updates it in place, and **Save as new** keeps both |

## Using it

**Starting.** The first visit, and **New**, show the starting points. Every one is an ordinary
design: nothing on it is fixed. Picking one while your design has unsaved changes asks first.

**Adding.** The bar above the folder adds **Text**, an **Emoji**, a **Shape** (thirteen, from a
rounded rectangle to a speech bubble), a **Picture** (a file, one of your skins, or one dropped or
pasted onto the folder), a **Pattern**, and **Colour** (selects the background, or adds one).

**Arranging.** Click a layer to select it: it's tinted, with knobs on its corners and sides and a
round knob above it to turn it.

- Drag it to move it. It snaps to the middle of the folder, the tab, the folder's edges and other
  layers. Hold ⌘ (Ctrl) to place it freely.
- Drag a corner or side to resize it. Pictures keep their shape from a corner and crop from a
  side. Text and emoji always keep their shape.
- Hold ⇧ to turn in 15° steps, or to keep a shape's proportions. Hold ⌥ to resize from the middle.
- Double-click words to edit them, or an emoji to swap it.
- Click the folder itself to select its background.

**Layers.** The list at the top right shows the stack, top first. Drag a row to restack it,
double-click a name to rename it, and use the eye, the lock and the bin to hide a layer, pin it in
place or delete it. **Delete all** at the end of the heading clears the design, and the toast that
says so puts them back, as ⌘Z does. A heading's buttons, like a row's, show when it's pointed at.

**Layers and Settings** are each opened and closed from their heading, and the bar between them is
dragged to give the layers more room or less (double-click it to put it back, and the arrow keys
move it while it's focused). How they were left is remembered on this computer.

**Settings.** Below the layers is everything about the selected layer, under its name, with the
buttons to restack, duplicate and delete it in that heading:

- Text: font, weight, alignment, italics, capitals, size, letter spacing, line height and curve
  (an arch or a smile).
- Colour: solid, a linear gradient or a radial one, up to four colours with transparency, and
  ready-made gradients.
- Shapes: corner rounding, number of points or sides, and star depth or ring hole.
- Pictures: brightness, contrast, colour, hue, blur, black and white, sepia, invert, and
  rounded corners.
- Every layer: opacity and one of sixteen blend modes.
- Placed layers also have a shadow (**Make it a glow** for neon) and a sticker edge, a border
  that follows the outline like a die-cut sticker.

With nothing selected, the settings choose **On the folder** or **Free icon**.

**Seeing it.** The **Folder skeleton** switch under the canvas shows the design on the folder
(tab, paper sheet and edges), or flat with the folder's edges drawn over it, so you can see what
the folder hides. For a free icon it shows a faint folder behind it, for size. The four circles
beside it put the design on the window, a light desktop, a dark desktop or a colourful wallpaper,
which matters for anything see-through. On the right, the icon at its real sizes (64, 32 and 16
points) shows how it reads in Finder or Explorer.

**Saving.** The name at the top right is the skin's name. Left empty, the first words of the
design name it. **Save to Yours** keeps it in the library, and **Save & apply** also puts it on
the chosen folder. Once saved, it stays open for more changes: **Save changes** updates the same
skin in its place, and **Save as new** adds another. A design you haven't saved is kept while you
look elsewhere in the app, and on this computer until it's saved or replaced.

**Include N folders inside**, under the folder it applies to, is the folder panel's **Include
subfolders** switch: turning it on in either place turns it on in both. With it on, **Save &
apply** puts the design on the folder and every folder inside it, asking first when that's more
than ten. The button shows how far the run has got, with **Stop** beside it, and a toast says how
it ended: a stopped run offers **Carry on**, and one where some folders couldn't be changed
offers **See which**, which opens the folder panel's summary. Leave the composer while it goes
and it carries on in the background, with how far it has got at the foot of the sidebar.

When some of the folders inside were chosen with **Choose** in the folder panel, only those go,
and the switch says so: **Include 22 of 28 folders inside**.

| Keys | |
| --- | --- |
| ⌘Z / ⇧⌘Z | Undo, redo |
| Delete | Delete the selected layer |
| ⌘D | Duplicate it |
| ⌘C, then ⌘V | Copy it and paste a copy. ⌘V with a picture on the clipboard adds the picture |
| Arrows, ⇧ arrows | Nudge by 1 or 10 |
| ⌘] / ⌘[ | Bring forward, send backward (with ⌥: to the front, to the back) |
| Esc | Select nothing |
| In the layers list | ↑ ↓ select, ⌥↑ ⌥↓ restack, F2 rename |

## How it works

### The design is drawn in icon space

A design is a square canvas 1024 units across: the same canvas the folder template is measured
in (`crates/folderskin-core/src/geometry.rs`). What you put at a point is at that point on the
folder. There is no cover-fit and no crop, unlike artwork added from a file ([SKINS.md](SKINS.md)).
The document is plain data in `src/composer/doc.ts`, and every change makes a new one, which is
what undo walks back through (`src/composer/history.ts`).

### One rendering path still holds

The webview draws only the user's own art: text, emoji, shapes, pictures and patterns
(`src/composer/render.ts`). The folder around it is the Rust template's. `composer_template`
renders the template once, split into the layers a design sits between
(`compositor::template_layers`), at the master size of 2048 px:

```
top      the front panel's rim light and the shade along its bottom
design   masked by front: the front panel's coverage
middle   the back panel's rim light, the paper sheet and its highlight
design   masked by back: the back panel's coverage, tab included
```

The stage stacks those four on every frame (`src/composer/composite.ts`), which is cheap enough
for a drag. The saved icon is `compositor::render_master_placed`, which draws the same template
with the same helpers, both panels filled with the design at its own place. The test
`the_layers_stacked_around_a_design_are_the_saved_icon` holds the two within 3 levels per channel,
so the canvas shows the icon that gets written. A fifth layer, `outline`, is the folder's visible
edges. The flat view tints it, and it is never part of an icon.

### Saving

Saving draws the design once more at 2048 px, encodes it as PNG and sends it to `composer_save`
as raw bytes after a small JSON header (`[u32 LE header length][header][PNG]`,
`src/composer/body.ts`). On the Rust side, `src-tauri/src/composer.rs` then does the rest:

- **On the folder:** it renders the folder from the design.
- **Free icon:** it takes the design as the icon as it is. A completely transparent one is
  refused.
- **Stored as:** a finished folder (`kind: "folder"`, `source: "composer"`), so applying it,
  thumbnails and sharing work as they do for any finished folder.
- **The document:** kept beside the picture as `<stem>.design.json`, which is how **Edit design**
  opens it again.
- **Id:** the SHA-256 of the design's PNG, its document and its shape. The same design saved twice
  is one skin.
- **Save changes:** replaces the old skin in one write of the index and keeps its place in the
  library. A favourite moves to the new id.

A remix reads the skin's own picture with `composer_skin_image`:

- **Artwork** becomes a picture covering the folder.
- **A finished folder** becomes a free icon with the picture fitted in, exactly as the app
  applies it.

### Commands

| command | input | output |
| --- | --- | --- |
| `composer_template` | – | `{size, back, front, middle, top, outline, parts}`: the layers as PNG data URLs, and where the folder's parts are (canvas units) |
| `composer_save` | raw body with header `{name, tags, shape, design, replaces}` | `{skin, replaced}`: the saved skin, and the id of the design it replaced |
| `composer_preview` | raw body with header `{shape, sizes}` | the icon at each size (16 to 512, at most six), as data URLs |
| `composer_image` | `path` | `{url, width, height, name, alpha}`: a picture file, at most 2048 px, PNG if it has transparency and JPEG if not |
| `composer_skin_image` | `skinId` | the same, for a saved skin's own picture |
| `composer_design` | `skinId` | the design's document, or `null` for a skin not made here |

### The document

```json
{ "version": 1, "shape": "folder", "layers": [ { "kind": "fill", "paint": { "type": "solid", "color": "#3a86ff" }, "opacity": 1, "blend": "normal", "id": "…" } ] }
```

Layers are drawn first to last:

- **Covering the whole canvas:** `fill` (a colour or gradient) and `pattern`.
- **Placed:** `text`, `emoji`, `shape` and `image`. Each has a centre `x, y` in canvas units, a
  `rotation` in degrees, flips, an optional `shadow` and an optional sticker `edge`.
- **Every layer:** `opacity`, `blend`, and optional `name`, `hidden` and `locked`.

Colours are `#rrggbb`, or `#rrggbbaa` when see-through. A picture is kept inside the document as a
data URL. A document read from disk goes through `parseDoc` first:

- Unknown layers are dropped, and more than 64 layers are cut to 64.
- Numbers are kept to sensible ranges and colours made canonical.
- A picture that isn't a PNG, JPEG, WebP or GIF data URL is dropped.
- A document from a newer version is refused rather than misread.

### Limits worth knowing

- **Emoji** come from the system's emoji font. On macOS that font is a bitmap, sharp up to about
  160 px, so a very large emoji is slightly soft at Finder's biggest icon sizes and crisp at every
  size Finder normally shows.
- **Fonts** are styles, each a list of fonts that come with macOS, Windows or Linux, best first
  (`src/composer/fonts.ts`). FolderSkin ships only Manrope. **Another font you have** takes any
  installed family by name. A design is saved as pixels, so its fonts only need to be on the
  computer it's made on.
- **Picture adjustments** are done on the pixels (`src/composer/imagefx.ts`), since the macOS 12
  web view has no canvas filters. Blur is three box blurs, close to a Gaussian.
- **A draft** is kept in the web view's storage, which a design with several large pictures can
  outgrow. Such a draft still lasts until the app quits.

### The browser preview

`pnpm dev` in a plain browser shows the composer too. The folder layers are the PNGs in
`docs/images/composer/`, written by `cargo run -p folderskin-tools -- composer-layers --out
docs/images/composer`. A test in `folderskin-tools` checks they are the pixels the compositor
draws, so they can't go stale.

### Tests

| | |
| --- | --- |
| `folderskin-core` compositor | the stacked layers equal the saved icon, a design lands where it was drawn, a see-through design leaves only the paper and edges, and the outline follows the visible edges |
| `src-tauri` store | a design's document is saved beside it, survives a restart, goes when it is deleted, and is cleared away if a crash orphans it. Saving over a design keeps its place and refuses a skin that isn't a design |
| `src-tauri` composer | the body framing, picture checks, previews, saving, naming, a damaged document and picture encodings. A design sent as raw bytes through Tauri's own IPC (its mock runtime) is previewed and saved |
| frontend (vitest) | the document (its changes and how it is read back), undo, moving, resizing, turning and snapping, text layout and curves, shapes, colours, picture adjustments, templates and the body framing |
