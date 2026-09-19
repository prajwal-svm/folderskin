import React from "react";
import ReactDOM from "react-dom/client";
import { LazyMotion, MotionConfig, domMin } from "motion/react";
import Root from "./Root";
import { lockDown } from "./lib/lockdown";
import { applyTheme, loadThemePref, resolveTheme } from "./state/theme";
import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/components.css";
import "./styles/shell.css";
import "./styles/gallery.css";
import "./styles/stage.css";
import "./styles/studio.css";
import "./styles/composer.css";
import "./styles/community.css";
import "./styles/onboarding.css";
import "./styles/updates.css";

// The theme is known before anything draws, so a first launch's onboarding opens in it too.
applyTheme(resolveTheme(loadThemePref()));

// No right-click menu or browser shortcuts in a release build (lib/lockdown.ts).
if (import.meta.env.PROD) lockDown();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {/* `m` components get only the features loaded here. `domMin` is animation without
        gestures: the icons are driven by controls, not by props like `whileHover`. `strict`
        throws if a full `motion` component slips in and drags the whole library along. */}
    <LazyMotion features={domMin} strict>
      <MotionConfig reducedMotion="user">
        <Root />
      </MotionConfig>
    </LazyMotion>
  </React.StrictMode>,
);
