# Security policy

## Reporting a vulnerability

Report security problems privately through GitHub's advisory form:
<https://github.com/prajwal-svm/folderskin/security/advisories/new>. Please do not open a
public issue for a vulnerability.

Include the platform and app version, what FolderSkin did, and the smallest set of steps
that reproduces it. If you have a patch, say so and we will bring you into the private
advisory rather than asking for a public pull request.

Expect an acknowledgement within a week. Once a fix is ready we publish the advisory and
release a patched version. You will be credited unless you ask otherwise.

## Supported versions

Only the latest release gets fixes. There are no long-term support branches.

## Scope

FolderSkin runs on your machine. It goes online only for what the README lists under "What stays
on your computer" (community packs, the update check, an AI request you make, and the id of a
community pack you add, which is counted), so most of the interesting surface is local: the
paths it accepts, the files it writes inside a folder you chose (see
[docs/PLATFORMS.md](docs/PLATFORMS.md)), the images it decodes, and the `folderskin://install`
links it opens, which can only name a community pack. Reports about path handling, writes outside
the selected folder, crashes while decoding a picture, or a link that makes FolderSkin do more
than add a community pack are all in scope.
