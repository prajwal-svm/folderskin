# The AI assistant

FolderSkin can generate a skin from a description. The feature is **bring your own key**: you
paste an API key from a provider you already have an account with, the key is stored by your
operating system's keychain, and FolderSkin talks to that provider directly from your machine.

There is no FolderSkin server, no proxy, no bundled key and no free tier to subsidise. Nothing
is sent anywhere until you press **Generate**, and what is sent is your prompt, your chosen
size, and the reference picture if you picked one.

## Where the key lives

| | |
|---|---|
| macOS | Keychain, service `app.folderskin.desktop`, account = the provider id |
| Windows | Credential Manager, same service and account |
| Linux | Secret Service (GNOME Keyring, KWallet) |

The key is read at the moment of a request and is never written to a FolderSkin file, never
included in an error message, and never returned to the app's window. **Remove** in the
Generate view deletes it from the keychain.

If your desktop has no Secret Service running, saving a key fails with a message saying so;
install `gnome-keyring` or `kwalletmanager`, or use the built-in skins.

## The two shapes

This is the choice that matters most, and it is not about quality.

**Artwork** asks the model for a flat 1024 × 958 picture and FolderSkin wraps it onto its own
folder template, exactly like the ten built-in skins. The geometry is ours, so every skin lines
up with every other, at every icon size. Any provider can do this, including the ones with no
transparency support. This is the default and the right answer most of the time.

**Whole folder** asks the model to draw the folder itself on a transparent or keyed background,
and that image becomes the icon directly, bypassing the compositor. You give up pixel-exact
geometry and gain artwork that can sit in real relief and break over the folder's top edge.

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
- **Both** end with a hard output contract naming the pixel size, the isolation of the subject,
  and either the key colour or the transparent background.

You can edit these templates; they are ordinary Rust string constants with tests that assert
the load-bearing phrases are present.

## Cost

Every request is billed to your own account by your provider. The Generate view shows the
model's rough price before you press the button. FolderSkin makes exactly one request per
press; it never retries on its own.

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
| "… is rate limiting you right now" | 429; wait and try again |
| "the model drew a scene instead of a folder on a plain backdrop" | Whole-folder mode with no keyable background; try again or switch to Artwork |
| "the provider returned something that is not an image" | A malformed or non-image response |

## Keeping a generated skin

A generated skin lives in the session: it appears in the gallery, you can apply it to as many
folders as you like, and it is gone when you quit. To keep one in the repository's built-in set,
save the image and import it with the maintainer CLI, which crops it and writes the manifest
entry:

```sh
cargo run -p folderskin-tools -- skin add ~/Downloads/aurora.png \
  --id aurora-night --name "Aurora Night" --collection glow
```

See [SKINS.md](SKINS.md) for the format rules and the safe areas.
