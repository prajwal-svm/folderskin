#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.10"
# dependencies = ["pillow>=10.4", "numpy>=1.26"]
# ///
"""Paint FolderSkin folder art on this computer, with open-weight models and no API key.

Two models, the same on every platform, both Apache-2.0:

* Z-Image-Turbo (6B, 8 steps): text to picture. The workhorse for artwork.
* FLUX.2 [klein] 4B (4 steps): works from pictures. Reference photos, style references, and
  repainting FolderSkin's own blank folder for a whole-folder skin.

Two runtimes run them:

* stable-diffusion.cpp on Windows and Linux (CUDA for NVIDIA, Vulkan for everything else) and on
  a Mac's GPU through Metal. One prebuilt binary; its --auto-fit streams weights from RAM when
  they don't fit in VRAM, which is what lets a 4 GB laptop GPU run the 8-bit models.
* mflux on Apple Silicon, the MLX port of the same two models.

What comes out is a picture, not an icon. `artwork` pictures are wrapped onto FolderSkin's folder
by its own compositor, so the geometry is always exact; `folder` pictures are the folder itself on
magenta, which FolderSkin keys out. `folderskin-tools render` draws each one as the folder the app
makes of it, into previews/, and `folderskin-tools packs make` turns a folder of them into a pack.

    uv run fsgen.py doctor
    uv run fsgen.py setup
    uv run fsgen.py gen "a retro film camera" --style pop-art -n 4
    uv run fsgen.py gen "our dog Biscuit" --ref biscuit.jpg --style anime
    uv run fsgen.py gen "a koi pond at night" --shape folder
    uv run fsgen.py batch briefs.json
"""

from __future__ import annotations

import argparse
import dataclasses
import hashlib
import json
import os
import platform
import re
import secrets
import shutil
import subprocess
import sys
import time
import urllib.request
import zipfile
from pathlib import Path

# ---------- where things live ----------


def home() -> Path:
    """Runtimes and models: gigabytes that belong in a cache, never in the repository."""
    if env := os.environ.get("FOLDERSKIN_LOCALGEN_HOME"):
        return Path(env)
    system = platform.system()
    if system == "Windows":
        return Path(os.environ.get("LOCALAPPDATA", Path.home() / "AppData" / "Local")) / "folderskin-localgen"
    if system == "Darwin":
        return Path.home() / "Library" / "Caches" / "folderskin-localgen"
    return Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache")) / "folderskin-localgen"


def repo_root() -> Path:
    """The FolderSkin checkout this script sits in, for folderskin-tools."""
    for parent in Path(__file__).resolve().parents:
        if (parent / "Cargo.toml").is_file() and (parent / "crates" / "folderskin-tools").is_dir():
            return parent
    sys.exit("fsgen.py must stay inside a FolderSkin checkout: it uses folderskin-tools from there")


def tools_binary() -> Path:
    """folderskin-tools, built in release mode the first time: rendering at 2048 px is slow otherwise."""
    root = repo_root()
    exe = root / "target" / "release" / ("folderskin-tools.exe" if os.name == "nt" else "folderskin-tools")
    if not exe.is_file():
        say("building folderskin-tools (once)")
        subprocess.run(["cargo", "build", "--release", "-q", "-p", "folderskin-tools"], cwd=root, check=True)
    return exe


def say(msg: str) -> None:
    print(f"fsgen: {msg}", file=sys.stderr, flush=True)


# ---------- the machine ----------


@dataclasses.dataclass
class Machine:
    os: str  # windows, macos, linux
    arch: str  # x86_64, arm64
    ram_gb: float
    gpu: str  # nvidia, apple, other, none
    gpu_name: str
    vram_gb: float  # dedicated VRAM; on Apple Silicon, the unified memory

    def describe(self) -> str:
        gpu = f"{self.gpu_name} ({self.vram_gb:.0f} GB)" if self.gpu != "none" else "no GPU found"
        return f"{self.os} {self.arch}, {self.ram_gb:.0f} GB RAM, {gpu}"


def _run(cmd: list[str]) -> str:
    try:
        return subprocess.run(cmd, capture_output=True, text=True, timeout=20).stdout.strip()
    except (OSError, subprocess.SubprocessError):
        return ""


def _ram_gb() -> float:
    system = platform.system()
    if system == "Windows":
        import ctypes

        class MemoryStatus(ctypes.Structure):
            _fields_ = [("dwLength", ctypes.c_ulong), ("dwMemoryLoad", ctypes.c_ulong)] + [
                (n, ctypes.c_ulonglong)
                for n in ("total", "avail", "pagetotal", "pageavail", "virtual", "availvirtual", "extended")
            ]

        status = MemoryStatus()
        status.dwLength = ctypes.sizeof(MemoryStatus)
        ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(status))
        return status.total / 2**30
    if system == "Darwin":
        return int(_run(["sysctl", "-n", "hw.memsize"]) or 0) / 2**30
    for line in Path("/proc/meminfo").read_text().splitlines():
        if line.startswith("MemTotal:"):
            return int(line.split()[1]) / 2**20
    return 0.0


def detect() -> Machine:
    system = {"Windows": "windows", "Darwin": "macos"}.get(platform.system(), "linux")
    machine = platform.machine().lower()
    arch = "arm64" if machine in ("arm64", "aarch64") else "x86_64"
    ram = _ram_gb()
    if system == "macos":
        if arch == "arm64":
            chip = _run(["sysctl", "-n", "machdep.cpu.brand_string"]) or "Apple Silicon"
            return Machine(system, arch, ram, "apple", chip, ram)
        return Machine(system, arch, ram, "none", "", 0.0)
    smi = _run(["nvidia-smi", "--query-gpu=name,memory.total", "--format=csv,noheader,nounits"])
    if smi:
        name, mib = smi.splitlines()[0].rsplit(",", 1)
        return Machine(system, arch, ram, "nvidia", name.strip(), int(mib) / 1024)
    if system == "windows":
        names = _run(["powershell", "-NoProfile", "-Command", "(Get-CimInstance Win32_VideoController).Name"])
    else:
        names = _run(["sh", "-c", "lspci 2>/dev/null | grep -Ei 'vga|3d|display'"])
    real = [n for n in names.splitlines() if n.strip() and "basic" not in n.lower() and "virtual" not in n.lower()]
    if real:
        # Windows reports AdapterRAM in 32 bits and Linux doesn't say; Vulkan's --auto-fit copes.
        return Machine(system, arch, ram, "other", real[0].strip(), 0.0)
    return Machine(system, arch, ram, "none", "", 0.0)


def pick_backend(m: Machine) -> str:
    if m.os == "macos":
        return "mlx" if m.arch == "arm64" else "cpu"
    if m.gpu == "nvidia" and m.os == "windows":
        return "cuda"
    if m.gpu in ("nvidia", "other"):
        return "vulkan"  # no Linux CUDA build is published; Vulkan runs on NVIDIA, AMD and Intel
    return "cpu"


def pick_tier(m: Machine) -> str:
    """8-bit weights where memory allows: they stream from RAM, so RAM decides, not VRAM."""
    return "q8" if m.ram_gb >= 24 else "q4"


# ---------- stable-diffusion.cpp ----------

# Pinned, like the models, so a run next month uses the build this was tested with and never needs
# GitHub's API, whose anonymous limit is 60 calls an hour. `setup --runtime latest` moves on.
SDCPP_TAG = "master-899-28b454b"
SDCPP_REPO = "https://github.com/leejet/stable-diffusion.cpp"
# The release assets that make up each backend, as (name, size, sha256). The CUDA build needs its
# runtime DLLs beside it. The macOS build wants macOS 26; mflux (--backend mlx) is the Mac default.
SDCPP_ASSETS = {
    ("windows", "cuda"): [
        ("sd-master-28b454b-bin-win-cuda12-x64.zip", 333408797,
         "85ba25c948b7a8e11e0d9bc2dddc977cad93f0690c31ed787ae089daf69a0588"),
        ("cudart-sd-bin-win-cu12-x64.zip", 563452046,
         "fe20366827d357c00797eebb58244dddab7fd9a348d70090c3871004c320f38d"),
    ],
    ("windows", "vulkan"): [("sd-master-28b454b-bin-win-vulkan-x64.zip", 31955788,
                             "3a4e5a75f022e4c0cad3e5a28c5921683adcb1808c0dd8e8a3536493497c793b")],
    ("windows", "cpu"): [("sd-master-28b454b-bin-win-cpu-x64.zip", 17205650,
                          "cb55af2f5f112f5ef6a8b2785e25d44b5427714b89acae3f7c6fcf4ba7a72eb0")],
    ("linux", "vulkan"): [("sd-master-28b454b-bin-Linux-Ubuntu-24.04-x86_64-vulkan.zip", 38579477,
                           "28675635a82dd24970acd9600dc5f82a6eab1a54b66e962bb39dd51e3d2b7e47")],
    ("linux", "cpu"): [("sd-master-28b454b-bin-Linux-Ubuntu-24.04-x86_64.zip", 25456185,
                        "378f5dcb7bdba87c2e50ea264eb17cb9e73c2bd6343294c1fc72279d78408b07")],
    ("macos", "metal"): [("sd-master-28b454b-bin-Darwin-macOS-26.6.2-arm64.zip", 34355695,
                          "2ef9041b3464dd4354748e52acb4c8a90904150231f131fccc3588b934e1f92d")],
}
# For `--runtime latest`: the same assets by pattern, since their names carry the commit.
SDCPP_PATTERNS = {
    ("windows", "cuda"): [r"bin-win-cuda12-x64\.zip$", r"^cudart-sd-bin-win-cu12-x64\.zip$"],
    ("windows", "vulkan"): [r"bin-win-vulkan-x64\.zip$"],
    ("windows", "cpu"): [r"bin-win-cpu-x64\.zip$"],
    ("linux", "vulkan"): [r"bin-Linux-.*-x86_64-vulkan\.zip$"],
    ("linux", "cpu"): [r"bin-Linux-.*-x86_64\.zip$"],
    ("macos", "metal"): [r"bin-Darwin-.*-arm64\.zip$"],
}


def sd_cli(backend: str) -> Path:
    return home() / "bin" / backend / ("sd-cli.exe" if os.name == "nt" else "sd-cli")


def latest_assets(key: tuple[str, str]) -> tuple[str, list[tuple[str, int, str | None, str]]]:
    """The newest release's assets for `key`, from GitHub's API (GITHUB_TOKEN lifts its limit)."""
    req = urllib.request.Request("https://api.github.com/repos/leejet/stable-diffusion.cpp/releases/latest")
    if token := os.environ.get("GITHUB_TOKEN"):
        req.add_header("Authorization", f"Bearer {token}")
    release = json.load(urllib.request.urlopen(req, timeout=30))
    assets = []
    for pattern in SDCPP_PATTERNS[key]:
        found = [a for a in release["assets"] if re.search(pattern, a["name"])]
        if not found:
            sys.exit(f"release {release['tag_name']} has no asset matching {pattern}")
        a = found[0]
        digest = (a.get("digest") or "").removeprefix("sha256:") or None
        assets.append((a["name"], a["size"], digest, a["browser_download_url"]))
    return release["tag_name"], assets


def install_sdcpp(m: Machine, backend: str, tag: str) -> None:
    key = (m.os, backend)
    if key not in SDCPP_ASSETS:
        sys.exit(f"stable-diffusion.cpp publishes no {backend} build for {m.os} {m.arch}; build it from "
                 f"source ({SDCPP_REPO}/blob/master/docs/build.md) and put sd-cli in {sd_cli(backend).parent}")
    exe = sd_cli(backend)
    stamp = exe.parent / ".release"
    if exe.is_file() and stamp.is_file() and stamp.read_text().strip() == tag:
        say(f"stable-diffusion.cpp {tag} ({backend}) is installed")
        return
    if tag == "latest":
        tag, assets = latest_assets(key)
    else:
        assets = [(n, size, sha, f"{SDCPP_REPO}/releases/download/{SDCPP_TAG}/{n}") for n, size, sha in SDCPP_ASSETS[key]]
    exe.parent.mkdir(parents=True, exist_ok=True)
    for name, size, sha256, url in assets:
        zpath = home() / "downloads" / name
        fetch(url, zpath, size, sha256)
        with zipfile.ZipFile(zpath) as z:
            z.extractall(exe.parent)
    if os.name != "nt":
        exe.chmod(0o755)
    stamp.write_text(tag)
    say(f"installed stable-diffusion.cpp {tag} ({backend}) in {exe.parent}")


# ---------- models ----------


@dataclasses.dataclass(frozen=True)
class File:
    repo: str
    rev: str
    path: str
    size: int
    sha256: str

    @property
    def url(self) -> str:
        return f"https://huggingface.co/{self.repo}/resolve/{self.rev}/{self.path}"

    @property
    def local(self) -> Path:
        return home() / "models" / Path(self.path).name


# Every file pinned to a revision, a size and a hash: what was tested is what gets downloaded.
QWEN3_4B = {
    "q8": File("unsloth/Qwen3-4B-GGUF", "22c9fc8a8c7700b76a1789366280a6a5a1ad1120", "Qwen3-4B-Q8_0.gguf",
               4280405792, "eed555233267a33c7e8ee31682762cc7751b3f6d224039086e0e846f05fffa5d"),
    "q4": File("unsloth/Qwen3-4B-GGUF", "22c9fc8a8c7700b76a1789366280a6a5a1ad1120", "Qwen3-4B-Q4_K_M.gguf",
               2497281312, "f6f851777709861056efcdad3af01da38b31223a3ba26e61a4f8bf3a2195813a"),
}
ZIMAGE = {
    "q8": File("leejet/Z-Image-Turbo-GGUF", "c61c0e422dc8b541b7548cf33a4ef8302b0f8085", "z_image_turbo-Q8_0.gguf",
               6577440704, "df1c5baa86d1398c979495a6072dbcee79444fdb884a2445582ba0769c44e9a1"),
    "q4": File("leejet/Z-Image-Turbo-GGUF", "c61c0e422dc8b541b7548cf33a4ef8302b0f8085", "z_image_turbo-Q4_K.gguf",
               3864250304, "14b375ab4f226bc5378f68f37e899ef3c2242b8541e61e2bc1aff40976086fbd"),
}
ZIMAGE_VAE = File("Comfy-Org/z_image_turbo", "6fc90a3b1b653e935a0d175e260736de25b84df5",
                  "split_files/vae/ae.safetensors", 335304388,
                  "afc8e28272cd15db3919bacdb6918ce9c1ed22e96cb12c4d5ed0fba823529e38")
KLEIN = {
    "q8": File("leejet/FLUX.2-klein-4B-GGUF", "3b1f5a9dc3abb32238b053aeb3d823c30afdacbd",
               "flux-2-klein-4b-Q8_0.gguf", 4300629440,
               "0bba6951258ec8f92d51114a8fa13e66828297bfff58a738f52729b3ef66fa28"),
    "q4": File("leejet/FLUX.2-klein-4B-GGUF", "3b1f5a9dc3abb32238b053aeb3d823c30afdacbd",
               "flux-2-klein-4b-Q4_0.gguf", 2460378560,
               "d1023499ef3f2f82ff7c50e6778495195c1b6cc34835741778868428111f9ff4"),
}
# The FLUX.2 VAE in the FLUX.2-dev repository is gated; this one isn't, and sd.cpp takes it.
KLEIN_VAE = File("black-forest-labs/FLUX.2-small-decoder", "a3efc24f613ef42d9428af62fdbd6f5fd8856c4a",
                 "full_encoder_small_decoder.safetensors", 249519092,
                 "ea4273f02d1fafbf8e1d1c2cf6018ed8748652eb0bf34f2dd91171f16f15ab62")


@dataclasses.dataclass(frozen=True)
class Model:
    id: str
    label: str
    licence: str
    takes_pictures: bool
    steps: int
    mlx_steps: int

    def files(self, tier: str) -> dict[str, File]:
        if self.id == "zimage":
            return {"diffusion": ZIMAGE[tier], "llm": QWEN3_4B[tier], "vae": ZIMAGE_VAE}
        return {"diffusion": KLEIN[tier], "llm": QWEN3_4B[tier], "vae": KLEIN_VAE}


MODELS = {
    "zimage": Model("zimage", "Z-Image-Turbo", "Apache-2.0", takes_pictures=False, steps=8, mlx_steps=9),
    "klein": Model("klein", "FLUX.2 [klein] 4B", "Apache-2.0", takes_pictures=True, steps=4, mlx_steps=4),
}
# mflux names; it downloads and quantizes the weights itself.
MLX_ZIMAGE_4BIT = "filipstrand/Z-Image-Turbo-mflux-4bit"


def fetch(url: str, dest: Path, size: int, sha256: str | None) -> None:
    """Downloads `url` to `dest`, resuming a partial download, and checks its size and hash once."""
    ok = dest.with_name(dest.name + ".ok")
    if dest.is_file() and dest.stat().st_size == size and ok.is_file():
        return
    dest.parent.mkdir(parents=True, exist_ok=True)
    part = dest.with_name(dest.name + ".part")
    if dest.is_file() and dest.stat().st_size == size:
        part = dest  # downloaded earlier, never checked
    else:
        have = part.stat().st_size if part.is_file() else 0
        if have > size:
            part.unlink()
            have = 0
        req = urllib.request.Request(url, headers={"Range": f"bytes={have}-"} if have else {})
        with urllib.request.urlopen(req, timeout=60) as res, open(part, "ab" if have and res.status == 206 else "wb") as out:
            done = have if res.status == 206 else 0
            last = 0.0
            while chunk := res.read(1 << 20):
                out.write(chunk)
                done += len(chunk)
                if time.monotonic() - last > 2:
                    last = time.monotonic()
                    say(f"{dest.name}: {done / 2**30:.2f} / {size / 2**30:.2f} GB")
        if part.stat().st_size != size:
            sys.exit(f"{dest.name} came down as {part.stat().st_size} bytes, not {size}; run setup again to resume")
    if sha256:
        say(f"checking {dest.name}")
        h = hashlib.sha256()
        with open(part, "rb") as f:
            while chunk := f.read(1 << 24):
                h.update(chunk)
        if h.hexdigest() != sha256:
            part.unlink()
            sys.exit(f"{dest.name} doesn't match its published hash; it was deleted, run setup again")
    if part != dest:
        part.replace(dest)
    ok.write_text(sha256 or "size")


def install_models(tier: str) -> None:
    for model in MODELS.values():
        for f in model.files(tier).values():
            fetch(f.url, f.local, f.size, f.sha256)
    say(f"models ({tier}) are in {home() / 'models'}")


# `packs make` saves a finished folder as WebP through cwebp, or as a PNG several times the size,
# too big for a pack. Homebrew and Linux have a `webp` package; on Windows this is Google's build.
WEBP_WINDOWS = ("https://storage.googleapis.com/downloads.webmproject.org/releases/webp/libwebp-1.6.0-windows-x64.zip",
                4106264, "48886f506b21f62e4661f0f4cbfca19800897c385128e8902542d29a950c93f1")


def webp_bin() -> Path:
    return home() / "bin" / "webp"


def install_webp(m: Machine) -> None:
    if shutil.which("cwebp") or (webp_bin() / "cwebp.exe").is_file():
        return
    if m.os != "windows":
        say("cwebp is missing: install it (`brew install webp`, or the `webp` package) before making a pack")
        return
    url, size, sha256 = WEBP_WINDOWS
    zpath = home() / "downloads" / url.rsplit("/", 1)[1]
    fetch(url, zpath, size, sha256)
    webp_bin().mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(zpath) as z:
        for name in z.namelist():
            if name.endswith(("/bin/cwebp.exe", "/bin/webpmux.exe")):
                (webp_bin() / Path(name).name).write_bytes(z.read(name))
    say(f"installed cwebp in {webp_bin()}")


def tools_env() -> dict[str, str]:
    """The environment for folderskin-tools: our cwebp on the PATH if there is no other."""
    env = dict(os.environ)
    env["PATH"] = str(webp_bin()) + os.pathsep + env.get("PATH", "")
    return env


def install_mlx() -> None:
    if shutil.which("mflux-generate-z-image-turbo"):
        say("mflux is installed")
        return
    if not shutil.which("uv"):
        sys.exit("install uv first (https://docs.astral.sh/uv/), then run setup again")
    subprocess.run(["uv", "tool", "install", "--upgrade", "mflux"], check=True)
    say("installed mflux; it downloads each model the first time it runs it")


# ---------- prompts ----------

# Styles are written as what to paint, never as what to avoid: a distilled model runs without a
# negative prompt, and naming a thing ("no folder") tends to paint it. No living artist or studio
# is named, so what they make can go in a community pack (docs/PACK-TERMS.md, rule 3).
STYLES = {
    "pop-art": "a bold pop art illustration: thick black outlines, flat saturated primary colours, Ben-Day halftone dots",
    "anime": "a hand-painted anime film still: soft watercolour skies, lush greenery, gentle warm light, clean cel-shaded shapes, nostalgic and whimsical",
    "oil": "a classical oil painting on canvas: rich glazes, visible impasto brushstrokes, dramatic chiaroscuro light, a museum masterpiece",
    "sketch": "a black and white graphite pencil sketch on textured paper: confident linework, fine hatching and cross-hatching, pure monochrome",
    "woodblock": "an ukiyo-e woodblock print: bold black outlines, flat indigo and vermilion colour blocks, washi paper grain",
    "travel-poster": "a vintage travel poster: flat colour shapes, a limited palette, grainy lithograph print texture",
    "watercolour": "a loose watercolour painting: soft wet edges, granulating pigment, white paper showing through",
    "clay": "a soft clay stop-motion diorama: rounded handmade shapes, pastel colours, fingerprints in the clay, warm studio light",
    "risograph": "a three-colour risograph print in teal, yellow and orange: coarse grain, slight misregistration",
    "art-nouveau": "an art nouveau poster: flowing organic lines, ornate floral borders, thin gold outlines, muted jewel colours",
    "pixel": "detailed 16-bit pixel art: crisp pixels, a limited retro palette, gentle dithering",
    "synthwave": "a 1980s airbrushed synthwave poster: glossy chrome, neon magenta and cyan glow, a sunset grid horizon",
    # Name only what should be in the picture: "softbox lighting" painted the softboxes.
    "photo": "a close-up product photograph: soft diffused light, crisp focus, shallow depth of field, a plain seamless coloured backdrop, rich colour",
    "none": "",
}

# Artwork lands on the folder's back panel whole, and on its front panel less a band at the top
# and bottom; the top eighth is the tab and the strip beside the paper (docs/SKINS.md). So: fill the
# frame, keep the subject in the middle, keep the top quiet.
ARTWORK = (
    "{style_lead}{idea}. The painted scene bleeds off all four edges of the image: no white border, "
    "no margin, no frame line and no paper edge anywhere around it. The main subject is large and sits "
    "in the centre, fully visible, with open space above it; the top eighth of the picture is only sky "
    "or plain background. Bold shapes and strong contrast that still read from across a room."
)

# The picture handed in is FolderSkin's blank folder on magenta (folderskin-tools template). The
# key colour is never named: an edit model told about magenta paints the folder magenta. It is
# told to leave the background alone instead, and cut_folder uses our silhouette, not the colour.
FOLDER = (
    "Turn the plain grey folder in image 1 into a folder painted all over as {idea}{style_tail}. The "
    "painting covers the folder's entire surface edge to edge, the back panel, the tab and the front "
    "panel, like a printed wrap rather than a picture placed on it, with the main subject in the middle "
    "of the front panel. The thin paper strip between the panels stays pale cream. Keep the folder's "
    "exact outline, tab, size and position, and leave the background around the folder exactly as it is."
)

REFERENCES = (
    "Using {refs} as the reference, paint {idea}{style_tail}. Keep the subject recognisable from the "
    "reference. One continuous full-bleed illustration that fills the entire frame edge to edge, the "
    "subject large and centred with open space above it; the top eighth of the picture is only sky or "
    "plain background."
)


def style_text(style: str) -> str:
    return STYLES.get(style, style).strip()


def ref_names(n: int, first: int = 1) -> str:
    names = [f"image {i}" for i in range(first, first + n)]
    return names[0] if n == 1 else ", ".join(names[:-1]) + " and " + names[-1]


def compose(idea: str, style: str, shape: str, n_refs: int) -> str:
    s = style_text(style)
    idea = idea.strip().rstrip(".")
    if shape == "folder":
        extra = f", taking the subject from {ref_names(n_refs, 2)}" if n_refs else ""
        return FOLDER.format(idea=idea + extra, style_tail=f", as {s}" if s else "")
    if n_refs:
        return REFERENCES.format(refs=ref_names(n_refs), idea=idea, style_tail=f", as {s}" if s else "")
    lead = s[0].upper() + s[1:] + " of " if s else ""
    return ARTWORK.format(style_lead=lead, idea=idea if s else idea[0].upper() + idea[1:])


# ---------- generating ----------

# Artwork is 1024 x 958 (compositor::SKIN_WIDTH/HEIGHT); models want sides divisible by 16, and the
# compositor cover-fits any size, so 1024 x 960 loses nothing. The folder's own frame is the same.
WIDTH, HEIGHT = 1024, 960


@dataclasses.dataclass
class Job:
    idea: str
    style: str = "none"
    shape: str = "artwork"  # artwork | folder
    refs: tuple[Path, ...] = ()
    seed: int = 0
    name: str = ""
    model_id: str = "auto"

    def model(self) -> Model:
        if self.refs or self.shape == "folder":
            return MODELS["klein"]  # the one that works from pictures
        return MODELS["zimage" if self.model_id == "auto" else self.model_id]


def slug(text: str, limit: int = 40) -> str:
    s = re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")
    return s[:limit].rstrip("-") or "skin"


@dataclasses.dataclass
class Runner:
    backend: str
    tier: str
    vram_gb: float


def run_sdcpp(job: Job, runner: Runner, prompt: str, pictures: list[Path], out: Path) -> list[str]:
    exe = sd_cli(runner.backend)
    if not exe.is_file():
        sys.exit(f"stable-diffusion.cpp isn't installed for {runner.backend}; run: uv run fsgen.py setup")
    model = job.model()
    files = model.files(runner.tier)
    missing = [f.local.name for f in files.values() if not f.local.is_file()]
    if missing:
        sys.exit(f"missing {', '.join(missing)}; run: uv run fsgen.py setup")
    cmd = [
        str(exe),
        "--diffusion-model", str(files["diffusion"].local),
        "--llm", str(files["llm"].local),
        "--vae", str(files["vae"].local),
        "-p", prompt,
        "--cfg-scale", "1.0",
        "--steps", str(model.steps),
        "--sampling-method", "euler",
        "-W", str(WIDTH), "-H", str(HEIGHT),
        "-s", str(job.seed),
        "--diffusion-fa",
        "-o", str(out),
    ]
    for p in pictures:
        cmd += ["-r", str(p)]
    if pictures and 0 < runner.vram_gb < 6:
        # Encoding a 1024 px reference wants ~3.8 GB of VRAM, more than a 4 GB card has free. At
        # 512 px it takes a second on the GPU (and it is still a 1024 px picture that comes out);
        # the other way out, the VAE on the CPU, costs a minute a picture.
        cmd += ["--ref-image-args", "vae_input_max_pixels=262144"]
    if runner.backend == "cpu":
        cmd += ["--backend", "cpu"]
    return cmd


def run_mlx(job: Job, tier: str, prompt: str, pictures: list[Path], out: Path) -> list[str]:
    model = job.model()
    common = ["--prompt", prompt, "--width", str(WIDTH), "--height", str(HEIGHT),
              "--seed", str(job.seed), "--steps", str(model.mlx_steps), "--output", str(out)]
    if model.id == "zimage":
        q = ["-q", "8"] if tier == "q8" else ["--model", MLX_ZIMAGE_4BIT]
        return ["mflux-generate-z-image-turbo", *q, *common]
    q = ["-q", "8" if tier == "q8" else "4"]
    if pictures:
        return ["mflux-generate-flux2-edit", "--model", "flux2-klein-4b", *q,
                "--image-paths", *map(str, pictures), *common]
    return ["mflux-generate-flux2", "--model", "flux2-klein-4b", *q, *common]


def file_sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def generate(job: Job, runner: Runner, out_dir: Path, preview: bool) -> Path:
    out_dir.mkdir(parents=True, exist_ok=True)
    name = job.name or f"{slug(job.idea)}-{slug(job.style, 16)}-{job.seed}"
    out = out_dir / f"{name}.png"
    pictures = list(job.refs)
    template, silhouette = out_dir / ".template.png", out_dir / ".silhouette.png"
    if job.shape == "folder":
        if not (template.is_file() and silhouette.is_file()):
            subprocess.run([str(tools_binary()), "template", "--width", str(WIDTH), "--height", str(HEIGHT),
                            "--out", str(template), "--mask", str(silhouette)], check=True, capture_output=True)
        pictures.insert(0, template)
    prompt = compose(job.idea, job.style, job.shape, len(job.refs))
    cmd = run_mlx(job, runner.tier, prompt, pictures, out) if runner.backend == "mlx" else \
        run_sdcpp(job, runner, prompt, pictures, out)
    say(f"{name}: {job.model().label}, seed {job.seed}")
    started = time.monotonic()
    res = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", errors="replace")
    seconds = time.monotonic() - started
    if res.returncode == 0xC0000135:  # STATUS_DLL_NOT_FOUND
        sys.exit("stable-diffusion.cpp needs the Microsoft Visual C++ runtime: install "
                 "https://aka.ms/vs/17/release/vc_redist.x64.exe and try again")
    if res.returncode != 0 or not out.is_file():
        tail = "\n".join((res.stderr or res.stdout).strip().splitlines()[-15:])
        sys.exit(f"generation failed ({res.returncode}):\n{tail}")
    if is_blank(out):
        # A backend can fail quietly and write a flat white or black picture (sd.cpp's Metal
        # backend does on some Macs); that is a failure, not a skin.
        out.unlink()
        sys.exit(f"{runner.backend} produced a blank picture; try another backend, e.g. --backend "
                 f"{'mlx' if runner.backend == 'metal' else 'cpu'}")
    say(f"{name}: done in {seconds:.0f} s")
    trimmed = job.shape == "artwork" and trim_border(out, out_dir / "raw")
    fit = cut_folder(out, silhouette, out_dir / "raw") if job.shape == "folder" else None

    # Everything needed to make it again, and to say where it came from when it is shared.
    meta = {
        "idea": job.idea, "style": job.style, "shape": job.shape, "prompt": prompt,
        "model": job.model().label, "model_licence": job.model().licence, "tier": runner.tier,
        "seed": job.seed, "steps": job.model().mlx_steps if runner.backend == "mlx" else job.model().steps,
        "size": [WIDTH, HEIGHT], "backend": runner.backend,
        "runtime": "mflux" if runner.backend == "mlx" else f"stable-diffusion.cpp {SDCPP_TAG}",
        "references": [{"file": p.name, "sha256": file_sha256(p)} for p in job.refs],
        "border_trimmed": trimmed,
        "silhouette_fit": None if fit is None else round(fit, 4),
        "seconds": round(seconds, 1), "made": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
        "digital_source_type": "http://cv.iptc.org/newscodes/digitalsourcetype/trainedAlgorithmicMedia",
    }
    out.with_suffix(".json").write_text(json.dumps(meta, indent=2) + "\n", encoding="utf-8")
    if preview:
        render(out, out_dir / "previews" / out.name)
    return out


def is_blank(path: Path) -> bool:
    import numpy as np
    from PIL import Image

    return float(np.asarray(Image.open(path).convert("L"), dtype=np.float32).std()) < 2.0


def _sides(img):
    return {"top": img, "bottom": img[::-1], "left": img.transpose(1, 0, 2), "right": img.transpose(1, 0, 2)[::-1]}


def border_widths(rgb) -> list[int]:
    """How far a paper margin reaches in from each side (top, bottom, left, right), 0 where none.

    Models asked for a poster or a print often paint it lying on paper: a flat margin, sometimes
    with a thin dark rule inside it. On a folder that margin becomes a blank tab and blank edges.
    A side counts only when a flat band of one colour ends at an edge the art starts on, most of
    whose pixels are something else; a studio backdrop that simply surrounds the subject carries
    on past where the subject begins, so it isn't cut.
    """
    import numpy as np

    return [_margin(lines) for lines in _sides(rgb.astype(np.int16)).values()]


def _margin(lines) -> int:
    """border_widths for one side, its lines ordered from the edge inwards."""
    import numpy as np

    n, span = lines.shape[0], lines.shape[1]
    middle = lines[:, span // 20: span - span // 20]  # a margin on the next side doesn't spoil the line
    paper = np.median(middle[0], axis=0)
    limit = n // 5

    def share(i: int) -> float:
        return float((np.abs(middle[i] - paper).max(axis=1) <= 28).mean())

    def dark(i: int) -> bool:
        return i < limit and float((middle[i].max(axis=1) < 110).mean()) >= 0.5

    margin = 0
    while margin < limit and share(margin) >= 0.96:
        margin += 1
    if margin < max(4, n // 200):
        return 0
    # The art has to start within a few anti-aliased lines of the margin's end.
    start = next((i for i in range(margin, min(margin + max(4, n // 100), limit)) if share(i) < 0.5), None)
    if start is None:
        return 0
    # A printed frame line inside the margin: mostly dark lines, soft at both edges.
    edge = start
    first = next((i for i in range(start, start + 3) if dark(i)), None)
    if first is not None:
        edge = first
        while edge - first < max(4, n // 60) and dark(edge):
            edge += 1
    return edge + 3


def ragged_width(rgb, side: str, paper) -> int:
    """How many lines in from `side` are still mostly `paper`: a watercolour's torn or bleeding
    edge, which never ends as cleanly as border_widths wants. Only asked once the other sides have
    shown the picture sits on paper."""
    import numpy as np

    lines = _sides(rgb.astype(np.int16))[side]
    n, span = lines.shape[0], lines.shape[1]
    middle = lines[:, span // 20: span - span // 20]
    width = 0
    while width < n // 10 and float((np.abs(middle[width] - paper).max(axis=1) <= 28).mean()) >= 0.6:
        width += 1
    return width + 3 if width >= 4 else 0


def trim_border(path: Path, raw_dir: Path) -> bool:
    """Cuts a paper margin off a picture and fills the frame again. True when it cut something."""
    import numpy as np
    from PIL import Image

    img = Image.open(path).convert("RGB")
    rgb = np.asarray(img)
    widths = border_widths(rgb)
    if sum(1 for x in widths if x) < 3:
        return False
    names = ("top", "bottom", "left", "right")
    if 0 in widths:
        widest = names[widths.index(max(widths))]
        paper = np.median(_sides(rgb.astype(np.int16))[widest][0], axis=0)
        missing = widths.index(0)
        widths[missing] = ragged_width(rgb, names[missing], paper)
    top, bottom, left, right = widths
    raw_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy2(path, raw_dir / path.name)
    w, h = img.size
    box = (left, top, w - right, h - bottom)
    inner = img.crop(box)
    # Cover-fit back to the original frame, centred, so nothing is stretched.
    scale = max(w / inner.width, h / inner.height)
    resized = inner.resize((round(inner.width * scale), round(inner.height * scale)), Image.LANCZOS)
    x0, y0 = (resized.width - w) // 2, (resized.height - h) // 2
    resized.crop((x0, y0, x0 + w, y0 + h)).save(path)
    say(f"{path.name}: cut a paper margin (top {top}, bottom {bottom}, left {left}, right {right} px)")
    return True


def cut_folder(path: Path, silhouette: Path, raw_dir: Path) -> float:
    """Cuts a whole-folder picture out along FolderSkin's own silhouette. Returns how well it fitted.

    The model repaints our blank folder and keeps its shape, but not always its exact scale: it
    came back ~2% smaller in testing, and its backdrop drifts from magenta to purple. So the
    painted folder is found against whatever backdrop it has, our silhouette is fitted to it, and
    the silhouette becomes the alpha. No colour keying, so no pink fringe, and the edge is ours.
    Below 0.95 overlap the model changed the shape itself; the picture stays on its backdrop.
    """
    import numpy as np
    from PIL import Image, ImageFilter

    img = Image.open(path).convert("RGB")
    rgb = np.asarray(img).astype(np.int16)
    ring = np.concatenate([rgb[:4].reshape(-1, 3), rgb[-4:].reshape(-1, 3),
                           rgb[:, :4].reshape(-1, 3), rgb[:, -4:].reshape(-1, 3)])
    backdrop = np.median(ring, axis=0)
    distance = np.abs(rgb - backdrop).max(axis=2).astype(np.float32)
    # A shadow the model casts on the backdrop is the backdrop's own colour, darker: same direction
    # in RGB. It must not count when finding the folder's box, or the box grows by the shadow's
    # width. Only there: inside the folder a red lantern can point the same way.
    lengths = np.linalg.norm(rgb, axis=2) * np.linalg.norm(backdrop) + 1e-6
    shadow = (rgb @ backdrop) / lengths > 0.985
    painted = (distance > 60) & ~shadow
    h, w = painted.shape
    rows = np.where(painted.sum(axis=1) > w * 0.02)[0]
    cols = np.where(painted.sum(axis=0) > h * 0.02)[0]
    if not len(rows) or not len(cols):
        return 0.0
    x0, x1, y0, y1 = cols[0], cols[-1] + 1, rows[0], rows[-1] + 1

    sil = Image.open(silhouette).convert("L")
    sil = sil.crop(sil.getbbox())
    fitted = Image.new("L", (w, h), 0)
    fitted.paste(sil.resize((x1 - x0, y1 - y0), Image.LANCZOS), (int(x0), int(y0)))
    # A pixel in from the model's own edge, where it blends into the backdrop.
    fitted = fitted.filter(ImageFilter.MinFilter(3))
    inside = np.asarray(fitted) > 127
    solid = (distance > 60) & ~(shadow & ~inside)
    fit = float((inside & solid).sum() / max(1, (inside | solid).sum()))
    if fit < 0.95:
        say(f"{path.name}: the model changed the folder's shape (fit {fit:.3f}); left on its backdrop")
        return fit

    # The model's edge wanders a few pixels either side of ours. In a band just inside our edge,
    # backdrop-coloured pixels go too, and a pixel that is part backdrop gets the backdrop taken
    # out of its colour, so the edge carries the painting's colours instead of a pink rim. Deeper
    # in, a pink lantern stays a pink lantern.
    alpha = np.asarray(fitted).astype(np.float32) / 255
    band = alpha > 0
    band &= ~(np.asarray(fitted.filter(ImageFilter.MinFilter(21))) > 0)
    keep = np.clip((distance - 40) / 80, 0, 1)
    alpha = np.where(band, alpha * keep, alpha)
    colour = rgb.astype(np.float32)
    share = np.where(band, keep, 1.0)[..., None]
    unmixed = (colour - (1 - share) * backdrop) / np.maximum(share, 0.05)
    colour = np.where(band[..., None], np.clip(unmixed, 0, 255), colour)
    if min(backdrop[0], backdrop[2]) - backdrop[1] > 100:
        # The model shades the folder's rim with the magenta around it; take that cast back out.
        spill = np.clip(np.minimum(colour[..., 0], colour[..., 2]) - colour[..., 1], 0, None) * band
        colour[..., 0] -= spill
        colour[..., 2] -= spill

    raw_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy2(path, raw_dir / path.name)
    rgba = np.dstack([colour, alpha * 255]).round().astype(np.uint8)
    Image.fromarray(rgba, "RGBA").save(path)
    say(f"{path.name}: cut out along FolderSkin's silhouette (fit {fit:.3f})")
    return fit


def render(picture: Path, dest: Path, size: int = 512) -> str:
    """Draws `picture` as the folder the app makes of it; says whether it read as artwork or a folder."""
    dest.parent.mkdir(parents=True, exist_ok=True)
    res = subprocess.run([str(tools_binary()), "render", str(picture), "--out", str(dest), "--size", str(size)],
                         capture_output=True, text=True)
    line = (res.stdout or res.stderr).strip()
    say(line)
    return line


def contact_sheet(previews: list[Path], dest: Path, cell: int = 256, cols: int = 4) -> None:
    from PIL import Image, ImageDraw

    if not previews:
        return
    rows = (len(previews) + cols - 1) // cols
    sheet = Image.new("RGB", (cols * cell, rows * (cell + 20)), (236, 236, 236))
    draw = ImageDraw.Draw(sheet)
    for i, p in enumerate(previews):
        x, y = (i % cols) * cell, (i // cols) * (cell + 20)
        img = Image.open(p).convert("RGBA").resize((cell, cell), Image.LANCZOS)
        sheet.paste(img, (x, y), img)
        draw.text((x + 6, y + cell + 3), p.stem[:36], fill=(40, 40, 40))
    sheet.save(dest)
    say(f"contact sheet: {dest}")


# ---------- commands ----------


def settings(args: argparse.Namespace) -> tuple[Machine, str, str]:
    m = detect()
    backend = args.backend if args.backend != "auto" else pick_backend(m)
    tier = args.tier if args.tier != "auto" else pick_tier(m)
    return m, backend, tier


def cmd_doctor(args: argparse.Namespace) -> None:
    m, backend, tier = settings(args)
    print(f"machine:  {m.describe()}")
    print(f"backend:  {backend}   tier: {tier}   home: {home()}")
    if backend == "mlx":
        print(f"mflux:    {'installed' if shutil.which('mflux-generate-z-image-turbo') else 'not installed'}")
    else:
        exe = sd_cli(backend)
        print(f"runtime:  {exe if exe.is_file() else 'not installed'}")
        if exe.is_file():
            devices = _run([str(exe), "--list-devices"])
            print("devices:  " + ("\n          ".join(devices.splitlines()) or "(none reported)"))
        for model in MODELS.values():
            files = model.files(tier)
            have = sum(f.local.is_file() for f in files.values())
            gb = sum(f.size for f in files.values()) / 1e9
            print(f"model:    {model.label}: {have}/{len(files)} files ({gb:.1f} GB with shared parts)")
    if backend == "cpu":
        print("note:     no GPU backend; expect minutes per picture")


def cmd_setup(args: argparse.Namespace) -> None:
    m, backend, tier = settings(args)
    say(f"{m.describe()} -> {backend}, {tier}")
    install_webp(m)
    if backend == "mlx":
        install_mlx()
        return
    install_sdcpp(m, backend, args.runtime)
    install_models(tier)


def cmd_pack(args: argparse.Namespace) -> None:
    """`folderskin-tools packs make` with everything after `pack`, and cwebp on hand."""
    res = subprocess.run([str(tools_binary()), "packs", "make", *args.rest], env=tools_env(), cwd=repo_root())
    sys.exit(res.returncode)


def jobs_from(args: argparse.Namespace) -> list[Job]:
    refs = tuple(Path(r).resolve() for r in args.ref or [])
    for r in refs:
        if not r.is_file():
            sys.exit(f"{r} doesn't exist")
    first = args.seed if args.seed is not None else secrets.randbelow(2**31)
    return [Job(args.idea, args.style, args.shape, refs, first + i, args.name if args.n == 1 else "", args.model)
            for i in range(args.n)]


def runner_for(args: argparse.Namespace) -> Runner:
    m, backend, tier = settings(args)
    return Runner(backend, tier, m.vram_gb)


def cmd_gen(args: argparse.Namespace) -> None:
    runner = runner_for(args)
    out_dir = Path(args.out).resolve()
    made = [generate(j, runner, out_dir, preview=not args.no_preview) for j in jobs_from(args)]
    if len(made) > 1 and not args.no_preview:
        contact_sheet([out_dir / "previews" / p.name for p in made], out_dir / "previews" / "_sheet.png")
    for p in made:
        print(p)


def cmd_batch(args: argparse.Namespace) -> None:
    """briefs.json: [{"idea": ..., "style": ..., "shape": ..., "refs": [...], "n": 2, "name": ...}, ...]"""
    runner = runner_for(args)
    briefs_path = Path(args.briefs).resolve()
    briefs = json.loads(briefs_path.read_text(encoding="utf-8"))
    out_dir = Path(args.out).resolve()
    made = []
    for b in briefs:
        n = int(b.get("n", 1))
        first = int(b["seed"]) if "seed" in b else secrets.randbelow(2**31)
        refs = tuple((briefs_path.parent / r).resolve() for r in b.get("refs", []))
        for i in range(n):
            name = b.get("name", "") if n == 1 else (f"{b['name']}-{i + 1}" if b.get("name") else "")
            job = Job(b["idea"], b.get("style", args.style), b.get("shape", "artwork"), refs, first + i, name,
                      b.get("model", "auto"))
            made.append(generate(job, runner, out_dir, preview=True))
    contact_sheet([out_dir / "previews" / p.name for p in made], out_dir / "previews" / "_sheet.png")
    print(f"{len(made)} pictures in {out_dir}")


def folders_under(root: Path, depth: int) -> list[Path]:
    """Folders to theme, `depth` levels down, leaving out hidden, system and tool folders."""
    skip = {"node_modules", "target", "__pycache__", "venv", ".git"}
    found, level = [], [root]
    for _ in range(depth):
        level = sorted(d for parent in level for d in parent.iterdir()
                       if d.is_dir() and not d.name.startswith((".", "$")) and d.name not in skip)
        found += level
    return found


def cmd_theme(args: argparse.Namespace) -> None:
    """Every folder under a root painted from its own name, in one style, and applied if asked.

    The style holds a drive together; each folder gets the next seed, since one seed for all of
    them painted every folder the same picture. A folder that already has a picture in --out is
    skipped, so an overnight run that stopped carries on where it was.
    """
    runner = runner_for(args)
    root = Path(args.root).resolve()
    if not root.is_dir():
        sys.exit(f"{root} isn't a folder")
    out_dir = Path(args.out).resolve() if args.out else Path("fsgen-out", f"theme-{slug(root.name)}").resolve()
    seed = args.seed if args.seed is not None else secrets.randbelow(2**31)
    folders = folders_under(root, args.depth)
    say(f"{len(folders)} folders under {root}, style {args.style!r}, seed {seed}")
    names: set[str] = set()
    for i, folder in enumerate(folders):
        name = slug(str(folder.relative_to(root)), 60)
        while name in names:
            name += "-x"
        names.add(name)
        picture = out_dir / f"{name}.png"
        if not picture.is_file():
            # The name bare, and never the word "folder": quoted, a model letters it (and misspells
            # it); told "folder", it paints a folder.
            idea = f"one clear, recognisable object or scene that stands for {folder.name}"
            generate(Job(idea, args.style, args.shape, (), seed + i, name, args.model), runner, out_dir, preview=True)
        if args.apply:
            res = subprocess.run([str(tools_binary()), "apply", str(folder), "--image", str(picture)],
                                 capture_output=True, text=True)
            say((res.stdout or res.stderr).strip())
    previews = [out_dir / "previews" / f"{n}.png" for n in sorted(names)]
    contact_sheet([p for p in previews if p.is_file()], out_dir / "previews" / "_sheet.png")
    if not args.apply:
        say("nothing applied; look at the sheet, then run again with --apply (revert with folderskin-tools revert)")


def cmd_styles(_: argparse.Namespace) -> None:
    for key, text in STYLES.items():
        print(f"{key:14} {text}")


def main() -> None:
    ap = argparse.ArgumentParser(prog="fsgen", description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)

    def common(p: argparse.ArgumentParser) -> None:
        p.add_argument("--backend", default="auto", choices=["auto", "cuda", "vulkan", "metal", "cpu", "mlx"])
        p.add_argument("--tier", default="auto", choices=["auto", "q8", "q4"],
                       help="q8: best, needs ~24 GB RAM; q4: smaller and a little softer")

    p = sub.add_parser("doctor", help="what this machine is and what fsgen would use")
    common(p)
    p.set_defaults(fn=cmd_doctor)

    p = sub.add_parser("setup", help="download the runtime and the models")
    common(p)
    p.add_argument("--runtime", default=SDCPP_TAG, help=f"stable-diffusion.cpp release tag, or latest (default {SDCPP_TAG})")
    p.set_defaults(fn=cmd_setup)

    p = sub.add_parser("gen", help="paint pictures from an idea")
    common(p)
    p.add_argument("idea", help="the subject and scene, e.g. 'a retro film camera on a desk'")
    p.add_argument("--style", default="none", help="a key from `styles`, or your own words")
    p.add_argument("--shape", default="artwork", choices=["artwork", "folder"],
                   help="artwork: wrapped onto FolderSkin's folder (default); folder: the whole folder, on magenta")
    p.add_argument("--ref", action="append", help="a reference picture; repeat for several")
    p.add_argument("-n", type=int, default=1, help="how many (seeds follow on from --seed)")
    p.add_argument("--seed", type=int)
    p.add_argument("--model", default="auto", choices=["auto", *MODELS],
                   help="for artwork without references; pictures always go to klein")
    p.add_argument("--name", default="", help="file name without extension (with -n 1)")
    p.add_argument("--out", default="fsgen-out")
    p.add_argument("--no-preview", action="store_true")
    p.set_defaults(fn=cmd_gen)

    p = sub.add_parser("batch", help="paint every brief in a JSON file")
    common(p)
    p.add_argument("briefs")
    p.add_argument("--style", default="none", help="style for briefs that don't name one")
    p.add_argument("--out", default="fsgen-out")
    p.set_defaults(fn=cmd_batch)

    p = sub.add_parser("theme", help="paint every folder under a root from its name, in one style")
    common(p)
    p.add_argument("root")
    p.add_argument("--style", default="none", help="a key from `styles`, or your own words")
    p.add_argument("--shape", default="artwork", choices=["artwork", "folder"])
    p.add_argument("--model", default="klein", choices=["auto", *MODELS],
                   help="klein by default: twice as fast, and a drive has many folders")
    p.add_argument("--depth", type=int, default=1, help="how many levels of folders below the root")
    p.add_argument("--seed", type=int, help="the first folder's seed; the next folder gets the next one")
    p.add_argument("--apply", action="store_true", help="put each picture on its folder, as the app would")
    p.add_argument("--out", help="where the pictures go (default fsgen-out/theme-<root>)")
    p.set_defaults(fn=cmd_theme)

    p = sub.add_parser("pack", help="make a pack from the pictures: folderskin-tools packs make, with cwebp",
                       description="Everything after `pack` goes to `folderskin-tools packs make`.")
    p.add_argument("rest", nargs=argparse.REMAINDER)
    p.set_defaults(fn=cmd_pack)

    p = sub.add_parser("styles", help="list the style presets")
    p.set_defaults(fn=cmd_styles)

    args = ap.parse_args()
    args.fn(args)


if __name__ == "__main__":
    main()
