# FolderSkin's community service

A Cloudflare Worker with a D1 database and two R2 buckets, at `https://community.folderskin.app`.
It does two things:

- **Sharing a pack without a GitHub account.** The app verifies the computer once, sends packs
  here signed with the computer's key, and each waits for the maintainer's review. Approved packs
  are pulled into folderskin-community with `folderskin-tools community pull`. `src/index.ts`
  describes the flow, and `wrangler.toml` how to set it up.
- **Counting installs**, for the counts on folderskin.app's gallery (`src/installs.ts`, below).

## The API

| | |
|---|---|
| `GET /v1/status` | whether sharing is open |
| `GET /verify`, `POST /v1/keys/verify` | the person check that verifies a computer |
| `GET`, `POST /v1/me` | who a computer is to the service (signed) |
| `/v1/submissions/…`, `DELETE /v1/packs/<submission>` | sending a pack for review, and taking it back (signed) |
| `POST /v1/reports` | reporting a pack, no account needed |
| `/v1/admin/…` | the maintainer's side, signed with a key in `ADMIN_KEYS` |
| `POST /v1/packs/<id>/installs` | one install of a community pack |
| `GET /v1/packs/installs` | every pack's install count |

Every error is `{"error": {"code", "message"}}`, the message a sentence the app shows as it is.

## Install counts

Once a pack from Community has been added, the app sends `POST /v1/packs/<id>/installs` with no
body. The answer is `{"counted": true}`, or `{"counted": false}` when the same network added that
pack earlier the same UTC day; the app doesn't wait for either.

| status | code | when |
|---|---|---|
| 400 | `bad_pack` | the id isn't a pack id (lower-case words joined by single dashes, at most 40 characters) |
| 404 | `unknown_pack` | no pack with that id is in folderskin-community's published `index.json` |
| 429 | `slow_down` | the network's burst limit (`BURST`) is spent |
| 503 | `not_configured` | `IP_SALT` isn't set |
| 503 | `index_unavailable` | `index.json` couldn't be read from GitHub, and no copy from the last day is kept |

The service reads `index.json` from
`https://raw.githubusercontent.com/prajwal-svm/folderskin-community/main/index.json` at most every
five minutes in each Worker isolate (and Cloudflare caches the fetch as long), and goes on with the
last copy for up to a day while GitHub can't be reached.

`GET /v1/packs/installs` answers `{"version": 1, "installs": {"classic-art": 42}}`: every pack
counted at least once. It has `Cache-Control: public, max-age=300`, and each Worker isolate reads
D1 for it at most once a minute. Pages on `https://folderskin.app` and `https://www.folderskin.app`
may read it (`Access-Control-Allow-Origin`, with `Vary: Origin`); a plain `GET` needs no
preflight, and an `OPTIONS` is answered anyway. It needs nothing but D1, so it works before any
secret is set; the burst limit covers it once `IP_SALT` is.

**What is kept.** The `installs` table holds a count per pack id. `installs_seen` holds, for each
add counted today, the UTC day, the pack id and an HMAC of the network the request came from (its
IPv4 /24 or IPv6 /48, as the quotas count it), under a key made from `IP_SALT` for installs and
for that day alone, so it can't be matched with the quotas' rows or with another day's. The
daily cron deletes every `installs_seen` row from before today. No address, user agent or other
header is stored.

## Tests

```sh
pnpm install
pnpm typecheck
pnpm test
```

The tests run inside workerd with local D1, R2 and rate-limit simulators (`vitest.config.ts`),
with every migration in `migrations/` applied, and never go online: GitHub, Turnstile and the
webhooks are stubbed in the tests that need them.

To try it by hand, `pnpm migrate:local` and `pnpm dev` (`wrangler dev`, which reads secrets from
`.dev.vars`; `IP_SALT=anything` is enough for counting), then:

```sh
curl -X POST http://localhost:8787/v1/packs/classic-art/installs
curl http://localhost:8787/v1/packs/installs
```

A FolderSkin started with `FOLDERSKIN_COMMUNITY_API=http://localhost:8787` reports its installs
there (a development build reports nowhere else).

## Deploying

The maintainer deploys, from this folder: apply the migrations to the live database first, then
deploy the Worker.

```sh
pnpm migrate:remote
pnpm run deploy
```

`migrations/0002_installs.sql` adds the two install tables; until it is applied, counting and
reading the counts fail. Secrets are set with `wrangler secret put` and never go in the
repository; `wrangler.toml` lists them.
