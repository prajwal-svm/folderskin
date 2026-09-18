# Security policy

## Reporting a vulnerability

Report security problems privately through GitHub's advisory form:
<https://github.com/YOUR-USER/folderskin/security/advisories/new>. Please do not open a
public issue for a vulnerability.

Include the platform and app version, what FolderSkin did, and the smallest set of steps
that reproduces it. If you have a patch, say so and we will bring you into the private
advisory rather than asking for a public pull request.

Expect an acknowledgement within a week. Once a fix is ready we publish the advisory and
release a patched version; you will be credited unless you ask otherwise.

## Supported versions

Only the latest release gets fixes. There are no long-term support branches.

## Scope

FolderSkin runs entirely on your machine. It makes no network requests and collects no
telemetry, so the interesting surface is local: the paths it accepts, the files it writes
inside a folder you chose (see [docs/PLATFORMS.md](docs/PLATFORMS.md)), and the images it
decodes. Reports about path handling, writes outside the selected folder, or crashes while
decoding a picture are all in scope.
