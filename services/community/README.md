# FolderSkin's community service

A Cloudflare Worker with a D1 database and three R2 buckets, at `https://community.folderskin.app`.
It does three things:

- **Sharing a pack without a GitHub account.** The app verifies the computer once, sends packs
  here signed with the computer's key, and each waits for the maintainer's review. `src/index.ts`
  describes the flow, and `wrangler.toml` how to set it up.
- **Publishing approved packs.** Approval gives a pack an id of its own and asks
  folderskin-community's packs workflow to publish it (`src/publish.ts`, below). The workflow pulls
  it with `folderskin-tools community pull`, checks it, commits it, and uploads the catalog it
  builds back here, into the bucket behind `https://packs.folderskin.app`.
- **Counting installs**, for the counts on folderskin.app's gallery (`src/installs.ts`, below).

## The API

| | |
|---|---|
| `GET /v1/status` | whether sharing is open |
| `GET /verify`, `POST /v1/keys/verify` | the person check that verifies a computer |
| `GET`, `POST /v1/me` | who a computer is to the service (signed) |
| `/v1/submissions/…`, `DELETE /v1/packs/<submission>` | sending a pack for review, and taking it back (signed) |
| `POST /v1/reports` | reporting a pack, no account needed |
| `/v1/admin/…` | the maintainer's side, signed with a key in `ADMIN_KEYS` (`src/admin.ts` lists it) |
| `POST /v1/admin/keys/<key>/unban`, `POST /v1/admin/networks/<network>/unban` | lifting a ban |
| `PUT /v1/admin/tree/<path>` | one file of the catalog, from folderskin-community's workflow |
| `GET /v1/exports/pending` | how many approved packs wait to be published |
| `POST /v1/packs/<id>/installs` | one install of a community pack |
| `GET /v1/packs/installs` | every pack's install count |

Every error is `{"error": {"code", "message"}}`, the message a sentence the app shows as it is. A
refusal that ends at a known time also has `retry_after`, in seconds.

## Pack ids

A pack approved here gets a new id: its name's slug cut to 33 characters, a dash and six
characters drawn at random from `abcdefghijklmnopqrstuvwxyz234567`, such as `classic-art-k7q2mx`.
It is the same contract as `folderskin_core::pack::new_id` (`newPackId` and `isGeneratedId` in
`src/text.ts`). Names can repeat as often as people like; ids never do. A new id is drawn again
if another approved pack has it, as its id or the folder it was pulled into, or if
folderskin-community's published `index.json` lists it as a pack or as a renamed pack's old id.
An id never changes once a pack has it.

## Publishing

The moment a pack is approved, from the admin API or a phone link, the service sends
folderskin-community a `repository_dispatch`:

```
POST https://api.github.com/repos/prajwal-svm/folderskin-community/dispatches
{"event_type": "pack-approved", "client_payload": {"submission": "sub_…", "pack": "<id>"}}
```

It goes out after the approval has answered (`ctx.waitUntil`), so an approval never waits on
GitHub or fails because of it. It needs the secret `GITHUB_DISPATCH_TOKEN`: a fine-grained token
for prajwal-svm/folderskin-community alone, with Contents read and write. Without it, or when
GitHub turns the request down or can't be reached, nothing is lost: the workflow asks
`GET /v1/exports/pending` every 15 minutes, which answers `{"pending": 2}` (how many approved packs
haven't been pulled yet, and nothing about them) with `Cache-Control: public, max-age=60`, and does
the work when it isn't 0. The count comes from an index of its own, and each Worker isolate reads
it at most once a minute, so asking often costs next to nothing. Every attempt is written to
`events`, and one GitHub turned down shows in the daily digest, since a token that has run out
would cause it.

The workflow uploads the catalog it builds with `PUT /v1/admin/tree/<path>`, one file at a time,
signed with its own key in `ADMIN_KEYS`, into the `PACKS` bucket (`folderskin-packs`), which
everyone reads at `https://packs.folderskin.app`. So no R2 token ever leaves Cloudflare.

- `<path>` is `v2/` and a path of letters, digits, `.`, `_`, `-` and `/`, at most 200 characters,
  with no `..` and no empty or `.` part.
- The request is signed as every admin request is, and also carries the body's SHA-256 as
  `X-Content-SHA256`: the signature is checked against that, and R2 checks the bytes against it, so
  the Worker never reads or hashes a file of up to 64 MB. It needs a `Content-Length`.
- `.json` is stored as `application/json`, `.webp` `image/webp`, `.png` `image/png`, `.jpg` and
  `.jpeg` `image/jpeg`, `.gz` `application/gzip`, and anything else `application/octet-stream`.
- `v2/head.json` is served with `Cache-Control: public, max-age=60`; everything else is named
  after its contents, so it gets `public, max-age=31536000, immutable`.
- The answer is `{"stored": true}`; a body that isn't the one signed for is `400 mismatch`, and
  nothing is stored.

Uploads count against the burst limit like any other request (120 a minute from one network), so
the workflow should upload only the files that are new, and wait out a `429 slow_down`.

## Limits

Every number is in `src/limits.ts`, except the burst limit, which is the `BURST` binding's in
`wrangler.toml`. The pictures' limits are `folderskin_core::pack`'s.

**Lossless pictures.** A pack shared here keeps every pixel: its pictures are PNG, or WebP whose
picture is in a `VP8L` chunk, as a simple `RIFF…WEBPVP8L` file or an extended (`VP8X`) one, never
animated. Only headers are read, as for every other check: a `.jpg` or `.jpeg` in a pack is
turned away when it opens, and a WebP whose picture is lossy (`VP8 `) when it arrives. Packs
already published keep whatever pictures they have.

| | |
|---|---|
| A pack's pictures | PNG or lossless WebP, 256 to 1024 px a side, 1.5 MB each; at most 50 pictures and 40 MB in all |
| A computer on probation (new) | 2 packs and 100 pictures a day, 1 pack waiting at a time |
| An active computer (a pack of theirs approved) | 3 packs and 150 pictures a day, 3 waiting |
| A trusted computer (set by the maintainer) | 10 packs and 500 pictures a day, 10 waiting |
| One network (an IPv4 /24 or IPv6 /48) a day | 5 computers verified, 6 packs, 300 pictures, 10 reports |
| The whole service a day | 1,500 pictures (`GLOBAL_DAILY_PICTURES`), 150 packs waiting (`MAX_WAITING`) |
| The burst limit | 120 requests a minute from one network |
| Backing off | a wait of 60 s after a strike, doubling with each strike, 24 h at most |
| Strikes | start again from nothing after 24 h without one |
| Marks | one for each pack turned down, kept 30 days; the 3rd in 30 days bans the key for 30 days |
| Abuse | the key banned for good, and the network the pack came from for 30 days |
| Networks | the 2nd key banned from one network in 30 days bans the network for 30 days |

**Backing off.** Every sharing request that is refused is a strike on its key and on its
network: a quota spent, too many packs waiting, the burst limit, or a request while cooling down.
After a strike, every sharing request (the four under `/v1/submissions`) waits
`min(60 s × 2^(strikes − 1), 24 h)` from the latest strike, and each one sent meanwhile is refused
and is a strike itself. Listing and withdrawing one's own packs, and everything else, never wait.
A full review queue and a pause are the service's own doing, so they are no strike. The burst
limit is checked before a request's signature, so it strikes the network alone, and only when the
network isn't cooling down already: a flood mustn't turn into a database write for each request.
Its answer is `slow_down` with `Retry-After: 60`, unless the network now has longer to wait: then
it is `cooling_down`, with the real wait, so the app doesn't come back in a minute only to be
turned away again.

**Bans.** Each pack the maintainer turns down or takes down is a mark on its key. Turning one down
as abuse (the maintainer's `"ban": true`, or the phone page's box, or a reason of `sexual`, `minor`
or `hate`) bans its key for good, with the tier, and the network it came from for 30 days; a
reason that bans by itself, such as `deceptive`, bans the key for good. A banned network can't
verify new computers, so a ban can't be dodged with a fresh key, and can't share. The decision's
answer lists the bans it made, the queue lists every ban in force, the day's bans are in the
digest, and `POST /v1/admin/keys/<key>/unban` and `POST /v1/admin/networks/<network>/unban` lift
them (lifting a key's ban for good puts it back on probation).

| status | code | when |
|---|---|---|
| 400 | `lossy_picture` | a picture is a JPEG or a lossy WebP; FolderSkin 0.1.7 sends every one as lossless WebP |
| 400 | `too_large` | a picture is over 1.5 MB |
| 400 | `pack_too_large` | the pack's pictures come to over 40 MB, checked when it opens and again when it is sent for review |
| 429 | `quota` | a daily quota of the computer's or its network's is spent |
| 429 | `waiting` | the computer has as many packs waiting as its tier allows |
| 429 | `slow_down` | the network's burst limit is spent (`Retry-After: 60`) |
| 429 | `cooling_down` | the computer or its network is cooling down: `Retry-After` and `retry_after` say for how long |
| 403 | `banned` | banned: `retry_after` says until when, unless it is for good |
| 503 | `queue_full` | the day's pictures, or the review queue, have run out |
| 503 | `paused` | the maintainer has paused sharing |

Each check is one D1 statement, the account's own, and a refusal writes at most one more. The
daily cron deletes what has run out.

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

**Renamed packs.** When `index.json` has a `moved` map (`{"classic-art": "classic-art-k7q2mx"}`),
an add under an old id counts toward the new one, since FolderSkin 0.1.4 to 0.1.6 still know packs
by the ids they had; an add under each from the same network on the same day counts once. The
counts from before a rename are moved once, after folderskin-community's rename has landed, with
the SQL `scripts/moved-installs.mjs` makes from its `moved.json`. From this folder:

```sh
node scripts/moved-installs.mjs <path to folderskin-community>/moved.json > /tmp/moved-installs.sql
pnpm exec wrangler d1 execute folderskin-community --remote --file=/tmp/moved-installs.sql
```

Each old id's count is added to its new id's (summed when both have one) and the old row deleted,
and today's once-a-network records move too. Running it a second time changes nothing.

`GET /v1/packs/installs` answers `{"version": 1, "installs": {"classic-art-k7q2mx": 42}}`: every
pack counted at least once. It has `Cache-Control: public, max-age=300`, and each Worker isolate
reads D1 for it at most once a minute. Pages on `https://folderskin.app` and
`https://www.folderskin.app` may read it (`Access-Control-Allow-Origin`, with `Vary: Origin`); a
plain `GET` needs no preflight, and an `OPTIONS` is answered anyway. It needs nothing but D1, so it
works before any secret is set; the burst limit covers it once `IP_SALT` is.

## What is kept

The `installs` table holds a count per pack id. `installs_seen` holds, for each add counted today,
the UTC day, the pack id and an HMAC of the network the request came from (its IPv4 /24 or IPv6
/48, as the quotas count it), under a key made from `IP_SALT` for installs and for that day alone,
so it can't be matched with the quotas' rows or with another day's. The daily cron deletes every
`installs_seen` row from before today. No address, user agent or other header is stored.

Penalties are the one place a network is recognised from one day to the next, since a ban lasts a
month. `penalties` and `marks` hold an HMAC of the network under a key made from `IP_SALT` for
penalties alone, which doesn't change from day to day, and only for a network with a strike or a
ban; the daily cron deletes each row once nothing in it counts any more. A submission keeps the
same hash of the network it was sent from, so turning it down can ban the network, until 30 days
after its decision.

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

`migrations/0002_installs.sql` adds the two install tables, `migrations/0003_penalties.sql` the
`penalties` and `marks` tables and each submission's `network`, and `migrations/0004_pending_index.sql`
the index the pending count reads; until they are applied, counting, sharing and decisions fail,
and the pending count reads every approved pack. The `PACKS` binding needs the
`folderskin-packs` bucket, with `packs.folderskin.app` as its custom domain. Secrets are set with
`wrangler secret put` and never go in the repository; `wrangler.toml` lists them,
`GITHUB_DISPATCH_TOKEN` among them.

The service asks for version 2 of the pack terms (`TERMS_VERSION`, the terms for sharing through
this service alone), and turns away a pack sent under any other. The app agrees to whichever
version `/v1/status` names, so deploy it together with the version 2 text of
`docs/PACK-TERMS.md`, or people agree to a version they weren't shown.
