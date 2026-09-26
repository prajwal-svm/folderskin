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
nothing is sent anywhere until you press **Generate**, and what is sent is your prompt, the size
the shape needs, and the reference picture if you picked one (for a whole folder without one,
FolderSkin's own blank folder template).

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

## The two shapes

This is the choice that matters most, and it is not about quality.

**Just the art** asks the model for a flat 1024 × 958 picture and FolderSkin wraps it onto its own
folder template, exactly like a photo you add. The geometry is ours, so every skin lines up with
every other, at every icon size. Any provider can do this, including the ones with no
transparency support. This is the default and the right answer most of the time.

**Whole folder** asks the model to draw the folder itself on a transparent or keyed background,
and that image becomes the icon directly, bypassing the compositor. You give up pixel-exact
geometry and gain artwork that can sit in real relief and break over the folder's top edge.

When the model can work from a picture (OpenAI, Grok, Gemini and FLUX.2) and you have not
attached one, FolderSkin sends its own blank folder template as that picture: our folder, painted
flat light grey, centred on the solid key colour, 1024 × 958 pixels (`compositor::blank_template`).
The prompt tells the model to repaint that exact folder, keeping its outline, tab, paper strip,
size and position, and to leave the backdrop flat. The result keeps FolderSkin's silhouette
instead of whatever folder the model would have invented. Because the template sits on the key
colour, such a run always takes the keyed route below, even on a model that could return
transparency. A reference picture you attach yourself is used as the artwork instead, as before.

## How transparency is handled

FolderSkin picks the right route for the model you chose:

- **Native alpha.** The request asks for a transparent background and the returned PNG already
  has one. FolderSkin only trims the transparent margin. GPT Image 2.5 Flare and Sunburst work
  this way.
- **No alpha.** The prompt asks for the folder alone on a flat key colour: magenta, `#FF00FF`,
  for every provider but Google. FolderSkin then removes that colour, removes the magenta that
  bled into the soft edge (the step that stops a cutout looking like it has a pink halo), and
  trims. Magenta is used because it almost never appears in folder art, and because a missing
  backdrop is detectable: if the border is not magenta, the model ignored the instruction and
  FolderSkin says so instead of applying a broken icon. Recraft is also told the colour as a
  parameter (`controls.background_color`), so its backdrop comes out flat whatever style you ask
  for.
- **Green for Gemini.** Gemini leaves a dark reddish rim around a subject on magenta, so its
  prompt and its template use green, `#00FF00`, instead. Green does belong in folder art (every
  leaf and field), so a green backdrop is only taken away where it reaches the edge of the
  picture, starting from the shade of green Gemini actually painted, and the greens painted on
  the folder stay (`matte::cutout_connected`).

The keying code is in `crates/folderskin-core/src/matte.rs` and is unit-tested, including the
case of a genuinely pink subject on a magenta backdrop.

## Prompts

`crates/folderskin-ai/src/prompts.rs` composes the prompt from your words plus a contract. The
parts that do the work are structural rather than stylistic:

- **Just-the-art prompts** forbid drawing a folder, an icon, a device or a mockup, and reserve the
  top eighth and a 6% border as dead space, because the template crops or curves those away.
- **Whole-folder prompts** pin the construction: exactly three parts, one tab, one visible paper
  edge, one front panel, and an explicit instruction not to add layers. Without that sentence
  models reliably produce stacked folders and double tabs.
- **Template prompts** (`compose_on_template`) go with the blank template: the attached image
  is the exact folder to repaint, its shape and framing stay as they are, the idea is painted
  across the back and front panels, and the key colour stays flat.
- **All of them** end with a hard output contract naming the isolation of the subject, and
  either the key colour or the transparent background. They never state a size: models don't
  paint to a pixel count they read, so the size goes in the request's own parameters (below).

You can edit these templates. They are ordinary Rust string constants with tests that assert
the load-bearing phrases are present.

## What each provider is sent

`crates/folderskin-ai/src/request.rs` builds every request exactly as its provider's API
reference documents it, checked against the provider's own SDK where it has one, and its tests
check each one field by field. A test in `crates/folderskin-ai/tests/providers.rs` also sends
every request to a stand-in server on your computer that answers the way the provider's
documentation says, so the method, the address, the key's header, the content type and every
field are checked on the wire.

Settings go in each provider's own parameters, never in the words. The size is the shape's
(1024 × 958 for the Mac look's artwork), sent exactly where a provider takes any size and as the
nearest size or aspect ratio it offers where it doesn't:

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
| "the model drew a scene instead of a folder on a plain backdrop" | Whole-folder mode with no keyable background, so try again or switch to Just the art |
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
the provider, the model and your prompt. It is in the gallery under Yours after a restart, and
deleting it there removes it from disk. [ARCHITECTURE.md](ARCHITECTURE.md#saved-skins) says
where the files live. If the write fails (a full disk, say), the skin stays for the rest of the
session rather than being lost.

To share generated skins with everyone, put them in a community pack: tag them and use **Share
with community** in the app, or turn a folder of saved renders into a pack with
`folderskin-tools packs make`. [PACKS.md](PACKS.md) has both, and [SKINS.md](SKINS.md) says how a
picture lands on the folder.
