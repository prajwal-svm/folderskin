// Written by scripts/icon-packs.mjs --all; don't edit by hand. Rebuild after changing a pack there.

/** One icon pack the composer offers: built in, or downloaded from the `icons-v1` release. */
export type IconPackInfo = {
  id: string;
  name: string;
  version: string;
  license: string;
  source: string;
  style: "stroke" | "fill";
  /** Logos that belong to their owners: fine on your own folders, not in a shared pack. */
  brands: boolean;
  builtin: boolean;
  count: number;
  bytes: number;
  sha256: string;
};

/** The release the downloadable packs are published under. */
export const ICON_RELEASE = "icons-v1";

export const ICON_PACKS: IconPackInfo[] = [
  {
    "id": "lucide",
    "name": "Lucide",
    "version": "1.47.0",
    "license": "ISC",
    "source": "https://lucide.dev",
    "style": "stroke",
    "brands": false,
    "builtin": true,
    "count": 2112,
    "bytes": 527972,
    "sha256": "0471428bbb1e0e7dd1238f0401b0787e130e925517838c35427fed130b2f0b5f"
  },
  {
    "id": "tabler",
    "name": "Tabler",
    "version": "3.48.0",
    "license": "MIT",
    "source": "https://tabler.io/icons",
    "style": "stroke",
    "brands": false,
    "builtin": false,
    "count": 5166,
    "bytes": 1574951,
    "sha256": "2928fec5bfe171745b68d19d4750e35dcae07f65cfb912f50d5b0dd46a1928ff"
  },
  {
    "id": "phosphor",
    "name": "Phosphor",
    "version": "2.1.1",
    "license": "MIT",
    "source": "https://phosphoricons.com",
    "style": "fill",
    "brands": false,
    "builtin": false,
    "count": 1512,
    "bytes": 750328,
    "sha256": "60b6830ad657079a87d3d6ea477530595bbe99bde27d934a3678dd083f3c3e8a"
  },
  {
    "id": "phosphor-fill",
    "name": "Phosphor Fill",
    "version": "2.1.1",
    "license": "MIT",
    "source": "https://phosphoricons.com",
    "style": "fill",
    "brands": false,
    "builtin": false,
    "count": 1512,
    "bytes": 678488,
    "sha256": "92298fb973f4096f2504e360136427bf19aa5994b367a60e19050eb6ff2583aa"
  },
  {
    "id": "heroicons",
    "name": "Heroicons",
    "version": "2.2.0",
    "license": "MIT",
    "source": "https://heroicons.com",
    "style": "stroke",
    "brands": false,
    "builtin": false,
    "count": 324,
    "bytes": 92679,
    "sha256": "140925da0cf50f8660878b7ac8eef7d251c42113721ac99c0778844bf1dcf5fa"
  },
  {
    "id": "heroicons-solid",
    "name": "Heroicons Solid",
    "version": "2.2.0",
    "license": "MIT",
    "source": "https://heroicons.com",
    "style": "fill",
    "brands": false,
    "builtin": false,
    "count": 324,
    "bytes": 153309,
    "sha256": "b82f18e7aa70b04f7f1f1895cdc973c097543411c89923ae4b48a9f2e8aaa163"
  },
  {
    "id": "iconoir",
    "name": "Iconoir",
    "version": "7.12.1",
    "license": "MIT",
    "source": "https://iconoir.com",
    "style": "stroke",
    "brands": false,
    "builtin": false,
    "count": 1378,
    "bytes": 473041,
    "sha256": "97b51444a00b89a930b500d406bf932ffa06679390af082a800a315679ac1e1d"
  },
  {
    "id": "bootstrap",
    "name": "Bootstrap Icons",
    "version": "1.13.1",
    "license": "MIT",
    "source": "https://icons.getbootstrap.com",
    "style": "fill",
    "brands": false,
    "builtin": false,
    "count": 2077,
    "bytes": 973927,
    "sha256": "047d2b0f9c9013ffc12b27ba950bcd1ceac40ffee77c2cf8308f32dbd6e9a69a"
  },
  {
    "id": "lobe",
    "name": "Lobe Icons",
    "version": "1.95.1",
    "license": "MIT",
    "source": "https://lobehub.com/icons",
    "style": "fill",
    "brands": true,
    "builtin": false,
    "count": 327,
    "bytes": 541796,
    "sha256": "5dd7908d13e71763d318ac8198d5644c6df4eb8cf5a991d316d0affd981ae491"
  },
  {
    "id": "simple-icons",
    "name": "Simple Icons",
    "version": "16.32.0",
    "license": "CC0-1.0",
    "source": "https://simpleicons.org",
    "style": "fill",
    "brands": true,
    "builtin": false,
    "count": 3461,
    "bytes": 4759910,
    "sha256": "0ebac200f7bf7032aadc518cae8f901098e55ff066bae79ec274f64d3c768a7d"
  }
];
