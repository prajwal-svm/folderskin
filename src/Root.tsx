import { useCallback, useEffect, useState } from "react";
import App from "./App";
import { Onboarding } from "./components/Onboarding";
import { api } from "./lib/tauri";

type Stage = "checking" | "onboarding" | "leaving" | "app";

/** How long the onboarding's fade may take before the app takes over regardless. */
const LEAVE_WAIT = 1200;

/**
 * The first-launch onboarding when the app asks for it, and the app itself after that and on every
 * other launch. The onboarding has no background of its own (the window's glass shows through, as
 * behind the sidebar), so the app waits until it has faded away, then fades in.
 */
export default function Root() {
  const [stage, setStage] = useState<Stage>("checking");
  /** The onboarding just finished, so the app fades in rather than being there at once. */
  const [arriving, setArriving] = useState(false);

  useEffect(() => {
    let live = true;
    api
      .onboardingNeeded()
      .then((needed) => live && setStage(needed ? "onboarding" : "app"))
      // If the app can't say, it opens as usual rather than risk an onboarding on every launch.
      .catch(() => live && setStage("app"));
    return () => {
      live = false;
    };
  }, []);

  const leave = useCallback(() => setStage("leaving"), []);
  const gone = useCallback(() => {
    setArriving(true);
    setStage("app");
  }, []);

  // The fade's end may never be reported (a hidden window doesn't animate), so it has a deadline.
  useEffect(() => {
    if (stage !== "leaving") return;
    const t = window.setTimeout(gone, LEAVE_WAIT);
    return () => clearTimeout(t);
  }, [stage, gone]);

  if (stage === "checking") return null;
  if (stage === "app")
    return arriving ? (
      <div className="app-arrive">
        <App />
      </div>
    ) : (
      <App />
    );
  return <Onboarding leaving={stage === "leaving"} onDone={leave} onGone={gone} />;
}
