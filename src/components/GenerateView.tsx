import { IconSparkles } from "./icons";

export function GenerateView() {
  return (
    <section className="view">
      <header className="view-head">
        <h2 className="view-title">
          Generate <span className="mark">with AI</span>
        </h2>
        <p className="view-sub">Describe a look, get a skin. Bring your own key; nothing runs without it.</p>
      </header>
      <div className="cards">
        <article className="card card-wide">
          <span className="card-glyph">
            <IconSparkles size={20} />
          </span>
          <span className="chip">Coming next</span>
          <h3 className="card-title">Prompt → folder skin</h3>
          <p className="card-text">
            You will paste an image-model API key of your choice, type a prompt such as "brushed copper with a soft
            vignette", and get a 1024 × 958 skin rendered straight into the gallery. Keys stay on your machine.
          </p>
          <div className="prompt-row" aria-hidden="true">
            <span className="prompt-fake">brushed copper with a soft vignette</span>
            <button type="button" className="btn btn-primary" disabled>
              <span className="btn-badge">
                <IconSparkles size={14} />
              </span>
              Generate
            </button>
          </div>
        </article>
      </div>
    </section>
  );
}
