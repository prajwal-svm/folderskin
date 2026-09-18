import React from "react";
import ReactDOM from "react-dom/client";
import { LazyMotion, MotionConfig, domMin } from "motion/react";
import App from "./App";
import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/components.css";
import "./styles/shell.css";
import "./styles/gallery.css";
import "./styles/stage.css";
import "./styles/studio.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {/* `m` components get only the features loaded here. `domMin` is animation without
        gestures: the icons are driven by controls, not by props like `whileHover`. `strict`
        throws if a full `motion` component slips in and drags the whole library along. */}
    <LazyMotion features={domMin} strict>
      <MotionConfig reducedMotion="user">
        <App />
      </MotionConfig>
    </LazyMotion>
  </React.StrictMode>,
);
