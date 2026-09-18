# Community skin packs

Skin packs anyone can add from FolderSkin's Community page. The app reads this folder straight
from GitHub.

- `packs/<id>/` is one pack: a `pack.json` and the pictures it lists. The folder name is the
  pack's id. New packs arrive here by pull request.
- `index.json` lists every pack, and `previews/<id>.png` shows the first four skins of each as
  folders. A workflow regenerates both whenever a pack changes, so never edit them by hand.

The full contract (every field of `pack.json`, the picture limits, the licences) and how to
share a pack are in [docs/PACKS.md](../docs/PACKS.md).

Check your pack before you open the pull request, from the repository root:

```sh
cargo run -p folderskin-tools -- packs check
```
