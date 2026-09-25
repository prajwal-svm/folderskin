import { type CSSProperties, useCallback, useEffect, useRef, useState } from "react";
import { INTRO_FRAMES, LOGO } from "../assets/onboarding";
import {
  continueLabel,
  defaultPick,
  frameAt,
  fromProgress,
  INTRO_SLOTS,
  INTRO_SPINS,
  INTRO_WAVE_TIMES,
  installFraction,
  installLine,
  type InstallState,
  spinFrameAt,
  toInstall,
} from "../lib/onboarding";
import { licenseLabel } from "../lib/packs";
import { api, errorMessage, type CommunityPack } from "../lib/tauri";
import { reducesMotion } from "../state/prefs";
import { applyTheme, loadThemePref } from "../state/theme";
import { OkBadge } from "./OkBadge";
import { PackPreview, prefetchPreview } from "./PackPreview";
import { ArrowLeftIcon } from "./icons/arrow-left";
import { ArrowRightIcon } from "./icons/arrow-right";
import { CheckIcon } from "./icons/check";
import { DownloadIcon } from "./icons/download";
import { LoaderIcon } from "./icons/loader";
import { RefreshCwIcon } from "./icons/refresh-cw";
import { clip } from "../lib/names";

/** The middle folder of the intro, the one that lands on the logo. */
const HERO = Math.floor(INTRO_SLOTS / 2);

// The intro's timeline, in ms from when its pictures are ready: the waves of new skins
// (INTRO_WAVE_TIMES), then the side folders fold into the middle one while it flips through
// INTRO_SPINS more, until it lands on the logo, which then makes room for the words.
const LAND_AT = 3400;
const SPIN_EVERY = 75;
const LOGO_AT = LAND_AT + INTRO_SPINS * SPIN_EVERY + 60;
const WELCOME_AT = LOGO_AT + 650;

/** How long the pictures may take to decode before the intro starts anyway. */
const DECODE_WAIT = 700;
/** How long "All set" shows before the app opens. */
const DONE_PAUSE = 900;

type Phase = "row" | "landing" | "logo" | "welcome";

/** Resolves once every picture is decoded, or after `wait` ms, whichever comes first. */
function decodeAll(sources: string[], wait: number): Promise<void> {
  const decoded = Promise.all(
    sources.map((src) => {
      const img = new Image();
      img.src = src;
      return img.decode().catch(() => {});
    }),
  ).then(() => {});
  return Promise.race([decoded, new Promise<void>((resolve) => setTimeout(resolve, wait))]);
}

/**
 * The first launch: a welcome where folders flip through skins and land on the FolderSkin logo,
 * then a step that adds the first community packs (Classic Art, picked for you, under whatever id
 * it has now). It fills the window until `onDone`, fades away (`leaving`), then calls `onGone` for
 * the app to open. It shows once: finishing it tells the app not to show it again.
 */
export function Onboarding({ leaving, onDone, onGone }: { leaving: boolean; onDone: () => void; onGone: () => void }) {
  const [step, setStep] = useState<"welcome" | "packs">("welcome");
  /** The intro plays once; coming back from the packs shows its last frame. */
  const played = useRef(false);
  const setup = usePackSetup();
  const finished = useRef(false);

  // The welcome is light unless dark was chosen before, and the window's glass turns light with it.
  // The app puts back the chosen look, or the system's, when it opens.
  useEffect(() => {
    const theme = loadThemePref() === "dark" ? "dark" : "light";
    applyTheme(theme);
    api.setWindowTheme(theme).catch(() => {});
  }, []);

  /** Remembers that the onboarding is done and opens the app, once. */
  const finish = useCallback(() => {
    if (finished.current) return;
    finished.current = true;
    // If it can't be remembered, the onboarding shows again next time, which is harmless.
    api
      .finishOnboarding()
      .catch(() => {})
      .finally(onDone);
  }, [onDone]);

  return (
    <div
      className={leaving ? "onboard is-leaving" : "onboard"}
      onAnimationEnd={(e) => {
        if (leaving && e.target === e.currentTarget) onGone();
      }}
    >
      <div className="onboard-drag" data-tauri-drag-region />
      {step === "welcome" ? (
        <Welcome
          replay={!played.current}
          onNext={() => {
            played.current = true;
            setStep("packs");
          }}
        />
      ) : (
        <PackStep setup={setup} onBack={() => setStep("welcome")} onFinish={finish} />
      )}
    </div>
  );
}

// ---------- the welcome ----------

/**
 * Five folders trying on skin after skin in waves, the middle one biggest; then the side ones fold
 * into it, it flips through a few more and lands on the logo, and the name and "Let's go" appear.
 * A click or a key skips to the end. Without `replay`, or with reduced motion, only the end shows.
 */
function Welcome({ replay, onNext }: { replay: boolean; onNext: () => void }) {
  const [animate] = useState(() => replay && !reducesMotion());
  const [phase, setPhase] = useState<Phase>(animate ? "row" : "welcome");
  const [ready, setReady] = useState(!animate);
  const [step, setStep] = useState(0);
  /** How many quick frames the middle folder has flipped through on its way to the logo. */
  const [spin, setSpin] = useState(0);
  /** The end is shown at once rather than arrived at: skipped, replayed or reduced motion. */
  const [instant, setInstant] = useState(!animate);
  const timers = useRef<number[]>([]);
  const skipped = useRef(false);
  const next = useRef<HTMLButtonElement>(null);

  const skip = useCallback(() => {
    skipped.current = true;
    timers.current.forEach(clearTimeout);
    timers.current = [];
    setInstant(true);
    setReady(true);
    setPhase("welcome");
  }, []);

  useEffect(() => {
    if (!animate) return;
    let live = true;
    const at = (ms: number, run: () => void) =>
      timers.current.push(
        window.setTimeout(() => {
          if (live && !skipped.current) run();
        }, ms),
      );
    void decodeAll([...INTRO_FRAMES, LOGO], DECODE_WAIT).then(() => {
      if (!live || skipped.current) return;
      setReady(true);
      INTRO_WAVE_TIMES.forEach((ms, i) => i > 0 && at(ms, () => setStep(i)));
      at(LAND_AT, () => setPhase("landing"));
      for (let k = 1; k <= INTRO_SPINS; k++) at(LAND_AT + (k - 1) * SPIN_EVERY, () => setSpin(k));
      at(LOGO_AT, () => setPhase("logo"));
      at(WELCOME_AT, () => setPhase("welcome"));
    });
    return () => {
      live = false;
      timers.current.forEach(clearTimeout);
      timers.current = [];
    };
  }, [animate]);

  // Any key skips the intro, except the app's shortcuts (⌘Q and the like).
  useEffect(() => {
    if (phase === "welcome") return;
    const onKey = (e: KeyboardEvent) => {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      e.preventDefault();
      skip();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [phase, skip]);

  // "Let's go" takes the focus once it's there, late enough that the key that skipped the intro
  // has come back up and can't press it too.
  useEffect(() => {
    if (phase !== "welcome") return;
    const t = window.setTimeout(() => next.current?.focus({ preventScroll: true }), instant ? 300 : 700);
    return () => clearTimeout(t);
  }, [phase, instant]);

  const landed = phase === "logo" || phase === "welcome";

  return (
    <section
      className={`welcome is-${phase}${instant ? " is-instant" : ""}`}
      onClick={phase === "welcome" ? undefined : skip}
      aria-labelledby="welcome-title"
    >
      <div className="welcome-stage" aria-hidden="true">
        <span className="welcome-halo" />
        {ready && (
          <div className="welcome-row">
            {Array.from({ length: INTRO_SLOTS }, (_, slot) =>
              // Arriving at the end at once, the side folders have nothing to do.
              instant && landed && slot !== HERO ? null : (
              <span key={slot} className={`welcome-slot slot-${slot}`} style={{ "--slot": slot } as CSSProperties}>
                {slot === HERO && landed ? (
                  <img key="logo" className="welcome-skin is-logo" src={LOGO} alt="" draggable={false} />
                ) : slot === HERO && spin > 0 ? (
                  <Flip key={`spin-${spin}`} from={null} to={spinFrameAt(spin)} fast />
                ) : (
                  <Flip key={`wave-${step}`} from={step > 0 ? frameAt(slot, step - 1) : null} to={frameAt(slot, step)} first={step === 0} />
                )}
              </span>
              ),
            )}
          </div>
        )}
      </div>
      <div className="welcome-copy">
        <h1 id="welcome-title" className="welcome-title">
          Folder<span className="brand-accent">Skin</span>
        </h1>
        <p className="welcome-tagline">Give any folder a skin.</p>
        <button ref={next} type="button" className="btn btn-primary btn-lg welcome-next" onClick={onNext}>
          Let's go
          <ArrowRightIcon size={17} />
        </button>
        <p className="welcome-fine">Free · Open source · No account</p>
      </div>
    </section>
  );
}

/**
 * One folder changing skin: the new one pours down over the old one, which fades beneath it.
 * `first` rises into place instead; `fast` is the quick flip of the landing.
 */
function Flip({ from, to, first, fast }: { from: number | null; to: number; first?: boolean; fast?: boolean }) {
  const kind = fast ? "is-fast" : first ? "is-first" : "is-in";
  return (
    <>
      {from !== null && <img className="welcome-skin is-out" src={INTRO_FRAMES[from]} alt="" draggable={false} />}
      <img className={`welcome-skin ${kind}`} src={INTRO_FRAMES[to]} alt="" draggable={false} />
    </>
  );
}

// ---------- the packs ----------

type PackSetup = ReturnType<typeof usePackSetup>;

/**
 * The packs step's state, kept above it so going back to the welcome and forward again doesn't
 * lose it. The list starts loading as soon as the onboarding opens, so it's there by the time
 * "Let's go" is pressed.
 */
function usePackSetup() {
  const [packs, setPacks] = useState<CommunityPack[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [picked, setPicked] = useState<ReadonlySet<string>>(() => new Set());
  /** The pack picked for them when the list arrived, which the heading names. */
  const [suggested, setSuggested] = useState<CommunityPack | null>(null);
  const [states, setStates] = useState<Record<string, InstallState>>({});
  /** The pack being added right now; nothing else can happen until it's done. */
  const [current, setCurrent] = useState<string | null>(null);
  const live = useRef(true);

  useEffect(() => {
    live.current = true;
    return () => {
      live.current = false;
    };
  }, []);

  const load = useCallback(() => {
    setLoadError(null);
    setPacks(null);
    api
      .communityPacks()
      .then(({ packs: list, moved }) => {
        if (!live.current) return;
        const pick = defaultPick(list, moved) ?? null;
        setPacks(list);
        setSuggested(pick);
        setPicked(new Set(pick ? [pick.id] : []));
        list.slice(0, 8).forEach((p) => prefetchPreview(p.preview));
      })
      .catch((e) => {
        if (live.current) setLoadError(errorMessage(e));
      });
  }, []);
  useEffect(load, [load]);

  const toggle = useCallback((id: string) => {
    setPicked((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const setState = (id: string, state: InstallState) => {
    if (live.current) setStates((prev) => ({ ...prev, [id]: state }));
  };

  /**
   * Adds packs one after another and resolves to how many couldn't be added. A pack is added
   * whole or not at all, so one that fails leaves nothing behind and can simply be tried again.
   */
  const install = useCallback(async (list: CommunityPack[]): Promise<number> => {
    let failed = 0;
    setStates((prev) => ({ ...prev, ...Object.fromEntries(list.map((p) => [p.id, { kind: "queued" } as InstallState])) }));
    for (const pack of list) {
      if (!live.current) break;
      setCurrent(pack.id);
      try {
        const skins = await api.addPack(pack.id, (p) => setState(pack.id, fromProgress(p)));
        setState(pack.id, { kind: "done", count: skins.length });
        if (live.current) setPacks((prev) => prev?.map((p) => (p.id === pack.id ? { ...p, added: true } : p)) ?? prev);
      } catch (e) {
        failed++;
        setState(pack.id, { kind: "failed", error: errorMessage(e) });
      }
    }
    if (live.current) setCurrent(null);
    return failed;
  }, []);

  return { packs, loadError, picked, suggested, states, current, load, toggle, install };
}

function PackStep({ setup, onBack, onFinish }: { setup: PackSetup; onBack: () => void; onFinish: () => void }) {
  const { packs, loadError, picked, suggested, states, current, load, toggle, install } = setup;
  const [allSet, setAllSet] = useState(false);
  const primary = useRef<HTMLButtonElement>(null);
  const running = current !== null;

  // The list fades out behind the footer, so its stylesheet needs the footer's height, which grows
  // when the note wraps.
  const section = useRef<HTMLElement>(null);
  const foot = useRef<HTMLElement>(null);
  useEffect(() => {
    const el = foot.current;
    if (!el) return;
    const measure = () => section.current?.style.setProperty("--foot", `${el.offsetHeight}px`);
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const list = packs ?? [];
  /** A pack's install state, while it's still wanted: unpicking a pack that failed forgets it. */
  const stateOf = (p: CommunityPack) => (p.added || picked.has(p.id) ? states[p.id] : undefined);
  const failed = new Set(list.filter((p) => stateOf(p)?.kind === "failed").map((p) => p.id));
  const next = toInstall(list, picked, failed);
  const anyAdded = list.some((p) => p.added);
  const addedSkins = list.reduce((sum, p) => {
    const state = states[p.id];
    return sum + (state?.kind === "done" ? state.count : 0);
  }, 0);

  // The main button has the focus when the packs arrive, so Return adds them.
  useEffect(() => {
    if (packs) primary.current?.focus({ preventScroll: true });
  }, [packs]);

  // "All set" shows for a moment, then the app opens.
  useEffect(() => {
    if (!allSet) return;
    const t = window.setTimeout(onFinish, DONE_PAUSE);
    return () => clearTimeout(t);
  }, [allSet, onFinish]);

  /** Adds packs; when that leaves nothing failed and nothing else picked, the app opens. */
  const run = async (packsToAdd: CommunityPack[]) => {
    const others = [...next, ...list.filter((p) => failed.has(p.id))].filter((p) => !packsToAdd.includes(p));
    const failures = await install(packsToAdd);
    if (failures === 0 && others.length === 0) setAllSet(true);
  };

  const onPrimary = () => {
    if (running || allSet) return;
    if (next.length) void run(next);
    else onFinish();
  };

  const currentPack = list.find((p) => p.id === current);
  const failures = list.flatMap((p) => {
    const state = stateOf(p);
    return state?.kind === "failed" ? [{ name: p.name, error: state.error }] : [];
  });
  // One reason for every failure (usually no connection) is worth saying; several are in their tooltips.
  const reason = failures.length && failures.every((f) => f.error === failures[0].error) ? failures[0].error : null;
  const note = allSet
    ? `${addedSkins} ${addedSkins === 1 ? "skin is" : "skins are"} in your library.`
    : running && currentPack
      ? `Adding ${clip(currentPack.name)}. This takes a few seconds.`
      : failures.length
        ? `${failures.map((f) => clip(f.name)).join(" and ")} couldn't be added${reason ? `: ${reason}.` : `. Try again, or add ${failures.length === 1 ? "it" : "them"} later from Community.`}`
        : packs
          ? "You can add or remove packs any time from Community."
          : loadError
            ? ""
            : "Getting the packs";

  return (
    <section className="packstep" aria-labelledby="packs-title" ref={section}>
      <header className="packstep-head">
        <h1 id="packs-title" className="packstep-title">
          Start with a few skins
        </h1>
        <p className="packstep-sub">
          Packs are free sets of skins people share through FolderSkin.{" "}
          {suggested ? `${clip(suggested.name)} is picked for you; add more if you like.` : packs ? "Pick any you like." : ""}
        </p>
      </header>

      <div className="packstep-body">
        {loadError ? (
          <div className="empty packstep-error">
            <span className="empty-glyph">
              <DownloadIcon size={20} />
            </span>
            <p className="empty-title">The packs didn't load</p>
            <p className="empty-text">
              {loadError.charAt(0).toUpperCase() + loadError.slice(1)}. You can carry on without them and add packs later from
              Community.
            </p>
            <button type="button" className="btn btn-secondary" onClick={load}>
              <RefreshCwIcon size={15} />
              Try again
            </button>
          </div>
        ) : (
          <ul className="packstep-grid" aria-busy={packs === null}>
            {packs === null
              ? [0, 1, 2].map((i) => (
                  <li key={i} className="onboard-pack is-skeleton" aria-hidden="true">
                    <span className="onboard-pack-hit">
                      <span className="pack-preview">
                        <span className="pack-preview-blank" />
                      </span>
                      <span className="onboard-pack-line" />
                      <span className="onboard-pack-line is-short" />
                    </span>
                  </li>
                ))
              : packs.map((p, i) => (
                  <PackCard
                    key={p.id}
                    pack={p}
                    index={i}
                    picked={picked.has(p.id)}
                    state={stateOf(p)}
                    running={running}
                    locked={running || allSet}
                    onToggle={() => toggle(p.id)}
                    onRetry={() => void run([p])}
                  />
                ))}
          </ul>
        )}
      </div>

      {/* The packs go soft and fade out as they scroll down to the buttons, which sit on the page itself. */}
      <div className="packstep-frost" aria-hidden="true">
        <i />
        <i />
        <i />
      </div>

      <footer className="packstep-foot" ref={foot}>
        <button type="button" className="btn btn-ghost" disabled={running || allSet} onClick={onBack}>
          <ArrowLeftIcon size={15} />
          Back
        </button>
        <p className="packstep-note" role="status" aria-live="polite">
          {note}
        </p>
        <button
          ref={primary}
          type="button"
          className={allSet ? "btn btn-primary btn-lg is-done" : "btn btn-primary btn-lg"}
          disabled={running || allSet || (packs === null && !loadError)}
          aria-busy={running}
          onClick={onPrimary}
        >
          {allSet ? (
            <>
              <OkBadge size={18} playOnMount /> All set
            </>
          ) : running ? (
            <>
              <LoaderIcon /> Adding
            </>
          ) : loadError ? (
            "Continue without packs"
          ) : packs ? (
            continueLabel(next, anyAdded)
          ) : (
            "Continue"
          )}
        </button>
      </footer>
    </section>
  );
}

function PackCard({
  pack,
  index,
  picked,
  state,
  running,
  locked,
  onToggle,
  onRetry,
}: {
  pack: CommunityPack;
  index: number;
  picked: boolean;
  state: InstallState | undefined;
  /** Some pack is being added, so nothing can be changed. */
  running: boolean;
  /** Nothing can be picked or unpicked any more. */
  locked: boolean;
  onToggle: () => void;
  onRetry: () => void;
}) {
  const done = pack.added || state?.kind === "done";
  const checked = done || picked;
  const busy = state && (state.kind === "queued" || state.kind === "download" || state.kind === "save");
  const cls = ["onboard-pack", checked && "is-picked", (locked || done) && "is-locked"].filter(Boolean).join(" ");

  return (
    <li className={cls} style={{ animationDelay: `${Math.min(index, 8) * 60}ms` }}>
      <button
        type="button"
        role="checkbox"
        aria-checked={checked}
        aria-label={`${pack.name}, ${pack.count} skins by ${pack.author}`}
        className="onboard-pack-hit"
        disabled={locked || done}
        onClick={onToggle}
      >
        <PackPreview src={pack.preview} count={pack.count} grid />
        <span className="onboard-pack-tick" aria-hidden="true">
          {checked && <CheckIcon size={13} playOnMount />}
        </span>
        <span className="onboard-pack-name" data-tip={pack.name} data-tip-overflow>
          {pack.name}
        </span>
        <span className="onboard-pack-by">
          {pack.count} {pack.count === 1 ? "skin" : "skins"} · @{pack.author} · {licenseLabel(pack.license)}
        </span>
      </button>
      <div className={state?.kind === "failed" ? "onboard-pack-status is-error" : done ? "onboard-pack-status is-ok" : "onboard-pack-status"}>
        {busy ? (
          <>
            <span className="onboard-bar" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(installFraction(state) * 100)}>
              <span style={{ transform: `scaleX(${installFraction(state)})` }} />
            </span>
            <span>{installLine(state)}</span>
          </>
        ) : state?.kind === "failed" ? (
          <span className="onboard-pack-failed">
            <span data-tip={state.error}>Couldn't add it</span>
            <button type="button" className="link-btn onboard-retry" disabled={running} onClick={onRetry}>
              <RefreshCwIcon size={13} />
              Try again
            </button>
          </span>
        ) : done ? (
          <span className="onboard-pack-ok">
            <OkBadge size={14} playOnMount={state?.kind === "done"} />
            {state?.kind === "done" ? installLine(state) : "Already in your library"}
          </span>
        ) : null}
      </div>
    </li>
  );
}
