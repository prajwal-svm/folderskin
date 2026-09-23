/**
 * The few pages the service draws itself: the person check the app opens in the browser, and the
 * pages behind the maintainer's phone links. Plain HTML with one small script of its own; anything
 * from a request or the database goes through `escapeHtml`.
 */
import { escapeHtml as e } from "./http";
import { REASONS } from "./terms";

const STYLE = `
:root { color-scheme: light dark; --fg: #1d1b19; --muted: #6b645c; --bg: #faf7f2; --card: #fff; --line: #e6dfd5; --accent: #c2410c; --ok: #15803d; --bad: #b91c1c; }
@media (prefers-color-scheme: dark) { :root { --fg: #f3efe9; --muted: #a8a097; --bg: #1a1816; --card: #24211e; --line: #37322d; } }
* { box-sizing: border-box; }
body { margin: 0; font: 16px/1.5 system-ui, -apple-system, "Segoe UI", sans-serif; color: var(--fg); background: var(--bg); }
main { max-width: 34rem; margin: 0 auto; padding: 2.5rem 1.25rem 3rem; }
h1 { font-size: 1.4rem; margin: 0 0 .75rem; }
p, li { color: var(--muted); }
.card { background: var(--card); border: 1px solid var(--line); border-radius: 14px; padding: 1.1rem 1.2rem; margin: 1rem 0; }
.who { color: var(--fg); font-weight: 600; }
#status { min-height: 1.5em; font-weight: 500; color: var(--fg); }
#status.ok { color: var(--ok); }
#status.error { color: var(--bad); }
img.sheet { width: 100%; height: auto; border-radius: 10px; border: 1px solid var(--line); margin: .5rem 0; }
form { display: flex; gap: .6rem; flex-wrap: wrap; align-items: center; margin: .6rem 0; }
button, select { font: inherit; padding: .55rem .9rem; border-radius: 10px; border: 1px solid var(--line); background: var(--card); color: var(--fg); }
button.go { background: var(--accent); color: #fff; border-color: transparent; }
button.danger { background: var(--bad); color: #fff; border-color: transparent; }
dl { display: grid; grid-template-columns: max-content 1fr; gap: .25rem .9rem; margin: 0; }
dt { color: var(--muted); }
dd { margin: 0; overflow-wrap: anywhere; }
dd.lines { white-space: pre-line; }
ul.flags { padding-left: 1.2rem; margin: .3rem 0; }
`;

function page(title: string, body: string, head = ""): string {
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex, nofollow">
<title>${e(title)}</title>
<style>${STYLE}</style>
${head}
</head>
<body><main>${body}</main></body>
</html>`;
}

/** The person check. The script reads the app's signed link from the address and does the rest. */
export function verifyPage(siteKey: string): string {
  return page(
    "Verify this computer · FolderSkin",
    `<h1>Verify this computer</h1>
<p>FolderSkin asks this once per computer before it shares packs without GitHub, to keep automated uploads out of the review queue.</p>
<div class="card">
  <p>Your packs will be credited to <span class="who" id="handle">…</span>.</p>
  <div id="challenge" data-sitekey="${e(siteKey)}"></div>
  <p id="status" role="status" aria-live="polite"></p>
</div>
<p>Nothing about you is kept apart from that name and this computer's public key. Your network address is only ever stored scrambled, and the scrambling changes every day.</p>`,
    `<script src="/verify.js"></script>
<script src="https://challenges.cloudflare.com/turnstile/v0/api.js?onload=folderskinChallenge&render=explicit" async defer></script>`,
  );
}

/** The verify page's script, served from the same origin so the page's policy can refuse inline script. */
export const VERIFY_SCRIPT = `"use strict";
(function () {
  var params = new URLSearchParams(location.search);
  var link = { k: params.get("k"), n: params.get("n"), t: params.get("t"), h: params.get("h"), s: params.get("s") };
  var status = function () { return document.getElementById("status"); };
  function say(text, kind) { var el = status(); el.textContent = text; el.className = kind || ""; }
  function complete() { return Object.keys(link).every(function (k) { return typeof link[k] === "string" && link[k].length > 0; }); }
  window.folderskinChallenge = function () {
    if (!complete()) { say("This link is incomplete. Start again from FolderSkin.", "error"); return; }
    document.getElementById("handle").textContent = link.h;
    var box = document.getElementById("challenge");
    window.turnstile.render(box, {
      sitekey: box.getAttribute("data-sitekey"),
      action: "verify",
      cData: link.n,
      callback: function (token) {
        say("Checking…");
        fetch("/v1/keys/verify", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ k: link.k, n: link.n, t: link.t, h: link.h, s: link.s, token: token })
        }).then(function (res) {
          return res.json().catch(function () { return {}; }).then(function (body) {
            if (res.ok) {
              document.getElementById("handle").textContent = body.handle;
              say("Done. You can close this tab and go back to FolderSkin.", "ok");
            } else {
              say((body && body.error && body.error.message) || "That didn't work. Start again from FolderSkin.", "error");
            }
          });
        }).catch(function () { say("FolderSkin's service couldn't be reached. Check your connection and try again.", "error"); });
      },
      "error-callback": function () { say("The check couldn't run. Reload the page to try again.", "error"); }
    });
  };
})();
`;

/** A page with a title and a sentence, for anything that is simply an answer. */
export function messagePage(title: string, text: string): string {
  return page(`${title} · FolderSkin`, `<h1>${e(title)}</h1><p>${e(text)}</p>`);
}

export type ReviewInfo = {
  name: string;
  handle: string;
  tier: string;
  status: string;
  pictures: number;
  license: string;
  source: string;
  notes: string;
  flags: { code: string; severity: string; detail: string }[];
  sheets: number;
  reports: { reason: string; details: string }[];
};

function details(info: ReviewInfo, token: string): string {
  const flags = info.flags.length
    ? `<ul class="flags">${info.flags.map((f) => `<li>${e(f.severity === "high" ? "Urgent: " : "")}${e(f.code)} (${e(f.detail)})</li>`).join("")}</ul>`
    : "none";
  const reports = info.reports.length
    ? `<ul class="flags">${info.reports.map((r) => `<li>${e(r.reason)}: ${e(r.details || "no details")}</li>`).join("")}</ul>`
    : "none";
  const sheets = Array.from({ length: info.sheets }, (_, n) => `<img class="sheet" alt="Contact sheet ${n + 1}" src="/l/${e(token)}/sheets/${n}">`).join("");
  return `<div class="card"><dl>
<dt>Pack</dt><dd>${e(info.name)}</dd>
<dt>By</dt><dd>${e(info.handle)} (${e(info.tier)})</dd>
<dt>Status</dt><dd>${e(info.status)}</dd>
<dt>Pictures</dt><dd>${info.pictures}</dd>
<dt>Licence</dt><dd>${e(info.license)}</dd>
<dt>Source</dt><dd>${e(info.source)}</dd>
<dt>Credits</dt><dd class="lines">${e(info.notes || "none given")}</dd>
<dt>Flags</dt><dd>${flags}</dd>
<dt>Reports</dt><dd>${reports}</dd>
</dl></div>${sheets}`;
}

function reasonOptions(selected = ""): string {
  return Object.entries(REASONS)
    .map(([code, r]) => `<option value="${e(code)}"${code === selected ? " selected" : ""}>${e(code)}: rule ${r.term}</option>`)
    .join("");
}

/** A pack waiting for a decision, with the three things the link can do to it. */
export function reviewPage(info: ReviewInfo, token: string): string {
  const waiting = info.status === "pending" || info.status === "flagged";
  const actions = waiting
    ? `<form method="post"><input type="hidden" name="decision" value="approve"><button class="go" type="submit">Approve and publish</button></form>
<form method="post"><input type="hidden" name="decision" value="reject"><select name="reason" aria-label="Reason">${reasonOptions("quality")}</select><button type="submit">Turn down</button></form>`
    : `<p>This pack is ${e(info.status)}, so there's nothing to decide.</p>`;
  const live = waiting || info.status === "approved";
  const remove = live
    ? `<form method="post"><input type="hidden" name="decision" value="takedown"><select name="reason" aria-label="Reason">${reasonOptions("sexual")}</select><button class="danger" type="submit">Take down</button></form>`
    : "";
  return page(`Review ${info.name} · FolderSkin`, `<h1>Review a pack</h1>${details(info, token)}${actions}${remove}
<p>Each button works once. This link runs out in a few days.</p>`);
}

/** A pack someone reported, with the button that takes it down. */
export function takedownPage(info: ReviewInfo, token: string): string {
  const live = ["pending", "flagged", "approved"].includes(info.status);
  const action = live
    ? `<form method="post"><input type="hidden" name="decision" value="takedown"><select name="reason" aria-label="Reason">${reasonOptions("sexual")}</select><button class="danger" type="submit">Take it down now</button></form>`
    : `<p>This pack is ${e(info.status)} already.</p>`;
  return page(`Take down ${info.name} · FolderSkin`, `<h1>Take down a pack</h1>${details(info, token)}${action}
<p>Taking it down removes it from FolderSkin's storage at once. If it was already pulled into the repository, remove it there too.</p>`);
}

/** The switch that pauses (or resumes) sharing without GitHub. */
export function pausePage(paused: boolean): string {
  const action = paused
    ? `<form method="post"><input type="hidden" name="decision" value="resume"><button class="go" type="submit">Resume sharing</button></form>`
    : `<form method="post"><input type="hidden" name="decision" value="pause"><button class="danger" type="submit">Pause sharing now</button></form>`;
  return page(
    "Pause sharing · FolderSkin",
    `<h1>${paused ? "Sharing is paused" : "Pause sharing without GitHub"}</h1>
<p>${paused ? "No new packs or verifications are taken until you resume." : "Pausing stops new packs and verifications at once. Packs already published stay up."}</p>${action}`,
  );
}
