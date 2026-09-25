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
nothing is sent anywhere until you press **Generate**, and what is sent is your prompt, your chosen
size, and the reference picture if you picked one (for a whole folder without one, FolderSkin's
own blank folder template).

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

**Artwork** asks the model for a flat 1024 × 958 picture and FolderSkin wraps it onto its own
folder template, exactly like a photo you add. The geometry is ours, so every skin lines up with
every other, at every icon size. Any provider can do this, including the ones with no
transparency support. This is the default and the right answer most of the time.

**Whole folder** asks the model to draw the folder itself on a transparent or keyed background,
and that image becomes the icon directly, bypassing the compositor. You give up pixel-exact
geometry and gain artwork that can sit in real relief and break over the folder's top edge.

When the model can work from a picture (OpenAI, Grok, Gemini) and you have not attached one,
FolderSkin sends its own blank folder template as that picture: our folder, painted flat light
grey, centred on solid magenta at the requested size (`compositor::blank_template`). The prompt
tells the model to repaint that exact folder, keeping its outline, tab, paper strip, size and
position, and to leave the magenta flat. The result keeps FolderSkin's silhouette instead of
whatever folder the model would have invented. Because the template sits on magenta, such a
run always takes the keyed route below, even on a model that could return transparency. A
reference picture you attach yourself is used as the artwork instead, as before.

## How transparency is handled

Models split into two groups, and FolderSkin picks the right route for the model you chose:

- **Native alpha.** The request asks for a transparent background and the returned PNG already
  has one. FolderSkin only trims the transparent margin.
- **No alpha.** The prompt asks for the folder alone on flat magenta, `#FF00FF`. FolderSkin then
  removes that colour, removes the magenta that bled into the soft edge (the step that stops a
  cutout looking like it has a pink halo), and trims. Magenta is used because it almost never
  appears in folder art, and because a missing backdrop is detectable: if the border is not
  magenta, the model ignored the instruction and FolderSkin says so instead of applying a
  broken icon.

The keying code is in `crates/folderskin-core/src/matte.rs` and is unit-tested, including the
case of a genuinely pink subject on a magenta backdrop.

## Prompts

`crates/folderskin-ai/src/prompts.rs` composes the prompt from your words plus a contract. The
parts that do the work are structural rather than stylistic:

- **Artwork prompts** forbid drawing a folder, an icon, a device or a mockup, and reserve the
  top eighth and a 6% border as dead space, because the template crops or curves those away.
- **Whole-folder prompts** pin the construction: exactly three parts, one tab, one visible paper
  edge, one front panel, and an explicit instruction not to add layers. Without that sentence
  models reliably produce stacked folders and double tabs.
- **Template prompts** (`compose_on_template`) go with the blank template: the attached image
  is the exact folder to repaint, its shape and framing stay as they are, the idea is painted
  across the back and front panels, and the magenta stays flat.
- **All of them** end with a hard output contract naming the pixel size, the isolation of the
  subject, and either the key colour or the transparent background.

You can edit these templates. They are ordinary Rust string constants with tests that assert
the load-bearing phrases are present.

## Cost

Every request is billed to your own account by your provider. The Generate view shows the
model's rough price before you press the button. FolderSkin makes exactly one request per
press and never retries on its own.

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
| "that key was rejected by …" | The provider returned 401 or 403 |
| "… is rate limiting you right now" | 429, so wait and try again |
| "the model drew a scene instead of a folder on a plain backdrop" | Whole-folder mode with no keyable background, so try again or switch to Artwork |
| "the provider returned something that is not an image" | A malformed or non-image response |

## Folders made in a chat assistant

You can also paint a whole folder in ChatGPT, Grok or any other chat assistant and bring it in
with **Your photo**. Ask for the folder on a solid #FF00FF background, or on a transparent one.
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
