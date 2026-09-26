# The AI assistant

FolderSkin can generate a skin from a description, in one of two ways:

- **The Local Model** paints on your own computer, for free. Set it up once (FLUX.2 [klein] 4B, a
  4.6 GB download on a Mac and 5.2 GB elsewhere), and it works offline with no key and nothing
  sent anywhere. It runs on Apple silicon Macs with macOS 14 or later, and on Windows and Linux
  PCs.
- **Bring your own key**: you paste an API key from a provider you already have an account with,
  the key is saved in a private file on your computer, and FolderSkin talks to that provider
  directly from your machine.

**Settings → AI Provider** is where you choose. A tick shows the Local Model is set up, or that a
provider has a key.

![Settings, AI Provider: the Local Model and seven providers, each ticked or marked No key, and the model on this machine below them](images/ai-providers.webp)

There is no FolderSkin server, no proxy, no bundled key and no free tier to subsidise. With a key,
nothing is sent anywhere until you press **Generate**, and what is sent is the prompt FolderSkin
writes from your words, the size the shape needs, and the pictures that go with it: for a whole
folder, FolderSkin's own blank template of that folder, and after it any reference pictures you
added.

## Where the key lives

Encrypted, in FolderSkin's own folder, readable only by your user account:

| | |
|---|---|
| macOS | `~/Library/Application Support/app.folderskin.desktop/keys.json` |
| Windows | `%APPDATA%\app.folderskin.desktop\keys.json` |
| Linux | `~/.config/app.folderskin.desktop/keys.json` |

The keys are sealed with AES-256-GCM before they are written. The encryption key is derived
(HKDF-SHA256) from a random secret in `keys.secret`, beside `keys.json`, and from this computer's
hardware id, so `keys.json` on its own gives nothing away and the two files copied to another
computer don't open there: enter the keys again on the new one. Both files are created with
owner-only permissions (0600) and written atomically. What no file can do is keep out a program
already running as you, which could read both. Only the system keychain could, with the
password prompts below.

Why not the system keychain: macOS ties a saved keychain item to the exact signature of the app
that saved it. Open-source builds are usually unsigned or ad-hoc signed, so every rebuild or
update would read as a different app and ask for your login password again. A password prompt
from an app you just downloaded looks like exactly the thing it isn't, so FolderSkin doesn't
use the keychain at all.

The key is read at the moment of a request, never included in an error message, and never
returned to the app's window. **Remove key** in the provider dialog deletes it from the file,
and deleting `keys.json` removes them all.

## What a picture is for

Every picture is made for a shape. The chip beside the model's name, under the prompt box, shows
which one with a small picture of it. Click it, or type @ in the box, to pick another:

- **Mac folder**: FolderSkin's folder, as Finder shows it.
- **Windows folder**: the folder Windows draws.
- **Free icon**: one thing on its own, such as a mascot, an object or a character, with no folder
  around it. It goes on any folder as it is.

After @, type part of a name (`@win`) and press Return or Tab, or click one. The chip changes and
the @ word leaves the box. A new chat starts on the folder the folder panel shows skins on.

The shape belongs to the chat. Each picture is made for the shape that was picked when it was
sent, an older chat opens on the shape of its last picture, and the skin is saved as made for
that shape. The shape decides the prompt, the template, the size and how the result is cut out,
with every provider and with the Local Model.

## Just the art or the whole folder

For a folder, this is the choice that matters most, and it is not about quality.

**Just the art** asks the model for a flat picture in the folder's own proportions (1024 × 960
for the Mac's folder, 1024 × 800 for Windows'), and FolderSkin wraps it onto its own folder,
exactly like a photo you add. The geometry is ours, so every skin lines up with every other, at
every icon size. Any provider can do this, including the ones with no transparency support.
This is the default and the right answer most of the time.

**Whole folder** asks the model to paint the folder itself, and that image becomes the icon
directly, bypassing the compositor. You give up pixel-exact geometry and gain artwork that can
sit in real relief and break over the folder's top edge.

When the model can work from a picture (OpenAI, Grok, Gemini and FLUX.2, and the Local Model),
FolderSkin sends its own blank template of the folder as the first picture: the folder painted
flat light grey, centred on a flat key colour, in the folder's own proportions and at most 1024
pixels on its longer side (`Base::blank`). The prompt tells the model to repaint that exact
folder, keeping its outline, its tab, the parts that make it that folder, its size and its
position, and to leave the backdrop as it is. The key colour is never named, because a model
told about magenta paints with it. FolderSkin then cuts the painting out along the folder's own
outline, so the result keeps FolderSkin's silhouette and keeps the painting's colours right up
to its edge. Models rarely keep the backdrop flat: it drifts to a dusty pink that brightens
towards a corner, or to a dark grey behind a night scene. So the backdrop is measured as the
colour it has at every pixel, from the edge of the frame inward (`backdrop.rs`), and the painted
folder is found against it. The outline counts as kept when paint spills past it, or backdrop
shows inside it (which counts twice), on less than half a percent of the folder. Only backdrop as
plain as the backdrop itself, reaching in from outside, counts as showing, so a painting that
shares the backdrop's colours, like a sunset's clouds, isn't mistaken for it. Where the model's
edge sits a few pixels inside FolderSkin's, the backdrop between the two is painted over with the
paint beside it. A painting that moved or reshaped the folder is cut out by its key colour
instead.
Reference pictures you add go after the template, each with the job you gave it, as many as the
model takes.

A **Free icon** is always painted whole: one subject, complete, in the middle of a square, on a
transparent backdrop or on a key colour, and cut out.

## How transparency is handled

FolderSkin picks the right route for the model you chose:

- **Native alpha.** The request asks for a transparent background and the returned PNG already
  has one. FolderSkin only trims the transparent margin. GPT Image 2.5 Flare and Sunburst work
  this way.
- **No alpha.** The prompt asks for the folder alone on a flat key colour. FolderSkin then
  removes that colour, removes the key that bled into the soft edge (the step that stops a
  cutout looking like it has a coloured halo), and trims. A missing backdrop is detectable: if
  the border is not the key colour, the model ignored the instruction, and FolderSkin keeps the
  picture as artwork for its own folder instead of applying a broken icon. Recraft is also told
  the colour as a parameter (`controls.background_color`), so its backdrop comes out flat
  whatever style you ask for.

The key is magenta, `#FF00FF`, because it almost never appears in folder art. It is green,
`#00FF00`, for Gemini, which leaves a dark reddish rim around a subject on magenta, and for
anything that is meant to be pink or violet, which a magenta cut would eat: the Neon, 70s
airbrush, Pop art and Synthwave styles, and any idea that names pink or violet in one of
FolderSkin's languages (pink, lilac, rose, rosa, morado, ピンク, 보라, 粉红 and the like). Green
does belong in folder art, in every leaf and field, so a green backdrop is only taken away where
it reaches the edge of the picture, starting from the shade of green actually painted, and the
greens painted on the folder stay (`matte::cutout_connected`).

A whole folder painted on FolderSkin's template takes neither route. It is cut along the
template's own outline, as described above, and only falls back to the key when the folder moved.

The keying code is in `crates/folderskin-core/src/matte.rs` and is unit-tested, including the
case of a genuinely pink subject on a magenta backdrop.

## Prompts

Your words are never rewritten. `crates/folderskin-ai/src/recipe.rs` gathers what a picture needs
into one recipe: your idea, the shape and what it keeps, the style, any lettering, your
pictures and the job each one does, and what goes around the subject. `prompts.rs` then writes
that recipe out the way each family of models reads best, always in the same order: what to
paint, how it looks, how it is framed, the pictures, the lettering, and what to leave out.

- **OpenAI and Gemini** get labelled lines (Style, Composition, Lettering, Constraints), with the
  pictures called image 1, image 2 and so on.
- **Grok** gets the same, with the pictures called `<IMAGE_0>`, `<IMAGE_1>` as Grok names them.
- **FLUX** (Black Forest Labs and the Local Model) gets plain prose with the subject first and no
  instructions about what not to draw, because FLUX has no negative prompt and paints what it is
  told to avoid. The Local Model's prompt stays inside the 400 tokens its text encoder reads,
  shortening the style to its medium when it has to.
- **Ideogram and Recraft** get a short design brief, with the lettering early.
- **Stability** gets a short list. It and Ideogram get what to leave out as their negative
  prompt (below).

The parts that do the work are structural rather than stylistic:

- **Just-the-art prompts** ask for one continuous picture that fills the frame, with the subject
  large and in the middle, and keep the band the folder's tab hides for sky or texture, because
  the template crops or curves it away. Windows' folder keeps its upper-left corner clear too.
- **Whole-folder prompts** name the folder's parts, back to front, and what stays as it is: the
  Mac's single tab and its pale paper strip, or the curved step on Windows' folder. Without that,
  models reliably produce stacked folders and double tabs.
- **Free-icon prompts** ask for one complete object, centred and uncropped in a square, with no
  floor, scenery or frame around it.
- **Reference pictures** are named by number and job. A subject is kept recognisably the same, a
  style picture gives its medium, palette, light and texture and none of its content, and a
  colours picture gives only its colours.
- **What to leave out** is written for the shape and the style: borders, frames, watermarks and
  signatures always, text unless you asked for some, the style's own clichés (such as Mount Fuji
  for a woodblock print) unless your idea asks for them, and clip-art looks for a realistic style.
- **None of them state a size.** Models don't paint to a pixel count they read, so the size goes
  in the request's own parameters (below).

Tests assert the load-bearing phrases for every family and every shape, and the recipe is
versioned (`RECIPE_VERSION`) so a skin can say which one made it.

## Styles

Type / in the prompt box for thirty styles, in five groups: Photo and 3D, Materials and craft,
Painting and drawing, Print, and Digital and graphic. Type part of a name to narrow the list.
A style goes in its own slot beside the box, never into your words, so the idea stays word for
word and the style is added after it. Click the slot's x to take it off.

Each style is one row of `crates/folderskin-ai/src/styles.json`, which the app, the command line
(`folderskin ai styles` lists them) and the prompt writer all read. A row has the style's name,
a one-line description, the words the prompt uses for it (the medium, then its technique, light,
colour and texture, and never a name of an artist), how it letters words, what it tends to add
unasked, three checks a result should pass, and each provider's own preset for it where one
exists (Stability's style preset, and Ideogram's style preset or style type). Old style names from
earlier versions still work: `travel` is now Travel poster (`screenprint`), `ukiyoe` is
Woodblock print and `diorama` is Tilt-shift miniature.

The same menu has **Ideas** to start from (a subject each, with no style) and **Your prompts**.
The style buttons under a new chat work the same way: each click fills the box with another idea
and puts the button's style in the slot.

## Lettering

Put the words you want on the skin in quotes: *a fox reading a map, with the word "ESCAPE"*.
FolderSkin letters exactly what is quoted, spelled out letter by letter for the models that read
instructions, once, in the style's own lettering, placed for the shape: across the middle of the
front panel for a folder, and on the object or beneath it for a free icon. Without quotes, the
prompt asks for no text at all. Keep it to one or two short words, because long text still comes
out garbled.

## Your prompts

**Save as a prompt**, in the / menu, keeps what is in the box, with its style, under a name you
give it. It is listed under **Your prompts** from then on: pick it and the box and the style slot
fill in again. Typing a name that is taken replaces that prompt, after saying so. The x beside
one of yours removes it, with **Undo** for a few seconds after.

Saved prompts are kept in `skills.json`, beside the `skins` folder
([ARCHITECTURE.md](ARCHITECTURE.md#saved-skins) says where that is), as skills in the
`folderskin.skill/1` format that `crates/folderskin-ai/src/skill.rs` describes. A
skill keeps what a picture shows (`idea`) apart from how it looks (`base_style`, or its own
`treatment`, `palette` and `light`), so one saved look can go with any idea. Every skill is
checked before it is written: it needs a name of up to 60 characters and something to save, its
style must be one FolderSkin has, its own treatment is 8 to 60 words and tells the model what to
do rather than what not to, and the whole skill fits in 4 KB. A prompt that names someone as
its style ("in the style of" a name, or "by" one) is saved after a word, because describing the
technique works better.

## Reference pictures

A picture added to a prompt is used for its **Subject** unless you say otherwise. Click its chip
to make it the **Style** (a look to match, taking none of what it shows) or the **Colours** (its
palette and nothing else). The pictures go to the model in that order, after the template, and
the prompt names each one by number and job.

## What each provider is sent

`crates/folderskin-ai/src/request.rs` builds every request exactly as its provider's API
reference documents it, checked against the provider's own SDK where it has one, and its tests
check each one field by field. A test in `crates/folderskin-ai/tests/providers.rs` also sends
every request to a stand-in server on your computer that answers the way the provider's
documentation says, so the method, the address, the key's header, the content type and every
field are checked on the wire.

Settings go in each provider's own parameters, never in the words. The size is the shape's
(1024 × 960 for the Mac's artwork, 1024 × 800 for Windows', a whole folder in its own shape and a
free icon square), sent exactly where a provider takes any size and as the nearest size or
aspect ratio it offers where it doesn't:

| Provider | Request | Size | Also sent |
|---|---|---|---|
| OpenAI | JSON to `images/generations`, or a form with each picture as `image[]` to `images/edits` | exact, on a 16-pixel grid (1024 × 960) | `quality`: high for Flare, max for Sunburst, medium for GPT Image 2, so the price is known beforehand |
| xAI Grok | JSON only, pictures included as data URLs (`image`, or `images` for several) | nearest `aspect_ratio`, or the template's own frame | |
| Google Gemini | JSON to `generateContent` | `generationConfig.imageConfig`: 1K, nearest `aspectRatio`, or the template's frame | `responseModalities: ["IMAGE"]` |
| Black Forest Labs | JSON, pictures as `input_image`, `input_image_2` and so on | exact, within one megapixel (FLUX.2 bills by the started megapixel) | prompt rewriting off (`disable_pup`, or `prompt_upsampling: false` on flex) |
| Recraft | JSON | the nearest size in V4.1's list | PNG output, and the key colour as `controls.background_color` |
| Stability AI | a form | nearest `aspect_ratio` | a negative prompt, and a style preset when the style has one |
| Ideogram | a form | the nearest of 3.0's resolutions (1024 × 960) | Magic Prompt off, a negative prompt, and a style type or preset |

The negative prompt keeps out text (unless lettering is asked for), watermarks and signatures,
plus whatever the chosen style tends to add. OpenAI, Gemini and Grok have no negative prompt,
so their prompt says the same in words.

## Models that were taken out

A choice saved while a model was on offer moves to the model that took its place, and a skin
keeps the name of the model that made it.

| Was | Now | Why |
|---|---|---|
| OpenAI GPT Image 1 | GPT Image 2 | OpenAI shuts it down on 23 October 2026 |
| Gemini 2.5 Flash Image | Gemini 3.1 Flash Image | Google shuts it down on 2 October 2026 |
| FLUX 1.1 Pro | FLUX.2 pro | the previous generation, which took no pictures |
| Recraft V3 | Recraft V4.1 | the previous generation, whose prompts stop at 1,000 characters, fewer than FolderSkin's own instructions take |

Ideogram 4.0 isn't offered yet: it rewrites every prompt written as words, and FolderSkin keeps
the idea exactly as you wrote it.

## Cost

Every request is billed to your own account by your provider. The Generate view shows the
model's rough price before you press the button, taken from the provider's pricing page at the
settings FolderSkin sends. A whole folder costs a little more on the providers that bill for the
pictures they are sent as well (xAI, Black Forest Labs). Details, under each picture, shows what
the provider said the request cost (xAI, Black Forest Labs, Recraft) or used (OpenAI, Gemini).

FolderSkin makes exactly one request per press and never retries on its own. That holds when a
provider finishes without a picture, too, as Gemini sometimes does (`NO_IMAGE`): the chat says so
and offers **Try again**, and only your press sends the request again.

## Building and cross-compiling

The provider layer uses `rustls` for TLS, whose crypto backend (`aws-lc-sys`) compiles C. That
builds cleanly on each platform's own CI runner, which is how FolderSkin's releases are made.
Cross-compiling it from one desktop to another (for example `cargo check --target
x86_64-pc-windows-msvc` on a Mac) needs a C cross-toolchain for the target and will otherwise
fail in `aws-lc-sys`'s build script. The rest of the workspace cross-checks without one.

## Failure messages you may see

| Message | What happened |
|---|---|
| "add your … API key first" | No key saved for that provider |
| "that key was rejected by …" | The provider turned the key down |
| "… is rate limiting you right now" | 429, so wait and try again |
| "… finished without painting a picture" | The provider answered without a picture and without saying why (Gemini's `NO_IMAGE`), so try again or reword the idea |
| "… declined that prompt: its filter blocked the picture" | The provider's safety filter stopped the prompt or the picture, such as Stability's blurred picture or Ideogram's safety check, so reword it |
| "… said: … (error 400)" | The provider's own words, shown as they came. Worth reporting if it names a field FolderSkin sent |
| "the model drew a scene instead of a folder on a plain backdrop" | Whole-folder mode with no keyable background, so try again or switch to Just the art. The app keeps such a picture as artwork for its own folder, and a free icon as the square picture it is, and says so. |
| "the provider returned something that is not an image" | A malformed or non-image response |

## Folders made in a chat assistant

You can also paint a whole folder in ChatGPT, Grok or any other chat assistant and bring it in
with **Add your photo**. Ask for the folder on a solid #FF00FF background, or on a transparent one.
FolderSkin recognises either and uses the picture as the icon as it is, cut out and trimmed,
instead of wrapping it in its own folder a second time. Any other picture is treated as
artwork for the template. [ARCHITECTURE.md](ARCHITECTURE.md#artwork-or-a-finished-folder) has
the exact rules, including why a photo of something on magenta paper stays a picture.

## Keeping a generated skin

Every generated skin is saved the moment it arrives, like an imported picture, together with
the provider, the model, your prompt and the shape it was made for. It is in the gallery under
Yours after a restart, and deleting it there removes it from disk.
[ARCHITECTURE.md](ARCHITECTURE.md#saved-skins) says where the files live. If the write fails (a
full disk, say), the skin stays for the rest of the session rather than being lost.

It also keeps what it was made from, under `recipe` in the skins' index, so a result can be
traced to its prompt and made again: the prompt exactly as it was sent, the negative prompt for
the providers that take one, the style, the words it letters, each picture's job and a hash of
it, the template and its version (`mac-folder/1`), the key colour, and the recipe's version.

The chat keeps its words and pictures while you look at other views, including a picture that is
still being made, and a new one starts when FolderSkin opens again. Earlier chats are in the
chat list, each on the shape it was for.

To share generated skins with everyone, put them in a community pack: tag them and use **Share
with community** in the app, or turn a folder of saved renders into a pack with
`folderskin-tools packs make`. [PACKS.md](PACKS.md) has both, and [SKINS.md](SKINS.md) says how a
picture lands on the folder.
