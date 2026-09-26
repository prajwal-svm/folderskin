import { forwardRef, useCallback, useEffect, useId, useImperativeHandle, useMemo, useRef, useState, type FormEvent, type KeyboardEvent } from "react";
import type { AiModel, AiPreset, AiProvider, SavedPrompt } from "../../lib/tauri";
import type { ChatRef, RefRole, Shape } from "../../state/chats";
import { filterShapes, shapeName, shapeNote, type ShapeInfo } from "../../lib/shapes";
import { lookName, sameLook, STYLE_GROUPS, STYLES, styleDescription, styleGroupName, styleName, type Look } from "../../lib/styles";
import { ideaName } from "../../lib/prompts";
import { promptLook, promptText } from "../../state/savedPrompts";
import { findTrigger, moveActive, namesSomeone, removeTrigger, replaceTrigger, rowsOf, slashMenu, suggestName, takenBy, type MenuGroup, type MenuItem, type Trigger } from "../../lib/promptMenu";
import { ProviderLogo } from "../ProviderLogo";
import { ArrowUpIcon } from "../icons/arrow-up";
import { CpuIcon, XIcon } from "../icons/composer";
import { LoaderIcon } from "../icons/loader";
import { PaletteIcon } from "../icons/palette";
import { PaperclipIcon } from "../icons/paperclip";
import { SlidersHorizontalIcon } from "../icons/sliders-horizontal";
import { useT } from "../../i18n";
import { providerName } from "../../lib/providerNames";
import { PromptMenu } from "./PromptMenu";
import { RefChip } from "./RefChip";
import { ShapePicker } from "./ShapePicker";

/**
 * How many of the person's own pictures a model takes: as many as one request carries, less the
 * blank template a whole folder is painted on, which goes first; none when it takes no pictures.
 */
export function refLimit(model: AiModel | undefined, template: boolean): number {
  if (!model?.accepts_reference) return 0;
  return Math.max(0, model.max_references - (template ? 1 : 0));
}

/** Whether a picture of `shape` made as `make` is painted on the shape's blank template. */
export function paintsOnTemplate(shape: ShapeInfo | undefined, make: Shape): boolean {
  return make === "folder" && shape?.family !== "free";
}

/**
 * Where an idea is written: the words, what goes with them (the look picked, the reference
 * pictures and what each is for, and a picture a model can't take is marked and left out, never
 * silently dropped), the shape it's for, the model it goes to (which opens the provider settings)
 * and send. Enter sends, Shift+Enter starts a new line.
 *
 * "@" typed in the box opens the shapes, and "/" the user's own prompts, the styles and the ideas
 * to start from; each narrows as more is typed, the arrow keys and Enter or Tab choose, Escape puts
 * it away, and what was typed to open it goes once something is chosen.
 */
export const PromptBox = forwardRef<
  HTMLTextAreaElement,
  {
    idea: string;
    onIdea: (idea: string) => void;
    placeholder: string;
    rows: number;
    refs: ChatRef[];
    /** A reference picture is being copied in. */
    adding: boolean;
    onAddRef: () => void;
    onRemoveRef: (id: string) => void;
    onRefRole: (id: string, role: RefRole) => void;
    shapes: ShapeInfo[];
    /** The shape the chat's pictures are for. */
    shape: ShapeInfo | undefined;
    onShape: (id: string) => void;
    /** For a shape with a base: the whole of it, or just the art. */
    make: Shape;
    onMake: (make: Shape) => void;
    /** The look picked to go with the words, which goes to the model in a slot of its own. */
    look: Look | null;
    onLook: (look: Look | null) => void;
    /** The user's own prompts and the ideas to start from, for the "/" menu. */
    prompts: SavedPrompt[];
    ideas: AiPreset[];
    /** Saves what's in the box under `name`, in `look`; resolves to whether it was. */
    onSavePrompt: (name: string, text: string, look: Look | null) => Promise<boolean>;
    onRemovePrompt: (prompt: SavedPrompt) => void;
    provider: AiProvider | undefined;
    model: AiModel | undefined;
    onSettings: () => void;
    onSend: () => void;
    /** The providers aren't in yet: sending waits for them. */
    loading: boolean;
    /** Sent while they weren't, and waiting for them. */
    queued: boolean;
    /** Why sending can't happen right now, when it can't though there are words to send. */
    blocked: string | null;
    /** A picture is being dragged over the window: it can be dropped here as a reference. */
    dropping: boolean;
    boxRef?: React.Ref<HTMLFormElement>;
  }
>(function PromptBox(props, ref) {
  const { idea, onIdea, placeholder, rows, refs, adding, onAddRef, onRemoveRef, shapes, shape, make, look, prompts, ideas, provider, model, onSettings, onSend, loading, queued, blocked, dropping, boxRef } = props;
  const t = useT();
  const menuId = useId().replace(/[^a-zA-Z0-9-]/g, "");
  const input = useRef<HTMLTextAreaElement>(null);
  const form = useRef<HTMLFormElement | null>(null);
  useImperativeHandle(ref, () => input.current as HTMLTextAreaElement, []);
  const setForm = useCallback(
    (el: HTMLFormElement | null) => {
      form.current = el;
      if (typeof boxRef === "function") boxRef(el);
      else if (boxRef) (boxRef as React.MutableRefObject<HTMLFormElement | null>).current = el;
    },
    [boxRef],
  );

  const limit = refLimit(model, paintsOnTemplate(shape, make));
  const ready = provider?.kind === "local" ? provider.has_key : Boolean(provider?.has_key);

  // ---------- the "@" and "/" menus ----------

  /** The trigger the caret is in, and the one Escape put away, which stays away until another is typed. */
  const [trigger, setTrigger] = useState<Trigger | null>(null);
  const [dismissed, setDismissed] = useState<number | null>(null);
  const [active, setActive] = useState(0);
  const [naming, setNaming] = useState<{ name: string; text: string; busy: boolean } | null>(null);
  const [flash, setFlash] = useState(0);

  const read = useCallback((text: string, caret: number) => {
    const next = findTrigger(text, caret);
    setTrigger((cur) => {
      if (!next) return null;
      // A new query starts at the top of the list again.
      if (!cur || cur.kind !== next.kind || cur.query !== next.query || cur.start !== next.start) setActive(0);
      return next;
    });
    setDismissed((d) => (next && d === next.start ? d : null));
  }, []);
  const readCaret = () => {
    const el = input.current;
    if (el) read(el.value, el.selectionStart ?? el.value.length);
  };
  // Words put in the box from elsewhere (a style chip, Reword it) bring the menu in step with them.
  useEffect(() => {
    const el = input.current;
    if (el && document.activeElement === el) read(el.value, el.selectionStart ?? el.value.length);
    else setTrigger(null);
  }, [idea, read]);

  const open = trigger !== null && dismissed !== trigger.start ? trigger : null;
  const textWithout = open ? removeTrigger(idea, open).text.trim() : idea.trim();

  const groups: MenuGroup[] = useMemo(() => {
    if (!open) return [];
    if (open.kind === "@") {
      return [
        {
          id: "shapes",
          title: "",
          items: filterShapes(shapes, open.query).map((s) => ({ kind: "shape" as const, id: s.id, name: shapeName(s), note: shapeNote(s), thumb: s.thumbnail, current: s.id === shape?.id })),
        },
      ].filter((g) => g.items.length > 0);
    }
    return slashMenu({
      query: open.query,
      prompts: prompts.map((p) => {
        const pl = promptLook(p);
        const text = promptText(p);
        return { id: p.id, name: p.name, note: [pl ? lookName(pl) : "", text].filter(Boolean).join(" · "), text, look: pl };
      }),
      styles: STYLE_GROUPS.map((g) => ({
        id: g.id,
        title: styleGroupName(g.id),
        items: STYLES.filter((s) => s.group === g.id).map((s) => ({ id: s.id, name: styleName(s.id), note: styleDescription(s.id), current: sameLook(look, { kind: "style", id: s.id }) })),
      })),
      ideas: ideas.map((i) => ({ id: i.id, name: ideaName(i), note: i.idea })),
      canSave: textWithout !== "",
      titles: { prompts: t("ai.slash.yours"), ideas: t("ai.slash.ideas") },
      save: { name: t("ai.slash.save"), note: t("ai.slash.saveNote") },
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open?.kind, open?.query, shapes, shape?.id, prompts, ideas, look, textWithout !== "", t]);
  const items = rowsOf(groups);

  /** Puts `next` in the box with the caret at `caret`, and the menu away. */
  const place = (next: { text: string; caret: number }) => {
    onIdea(next.text);
    setTrigger(null);
    requestAnimationFrame(() => {
      const el = input.current;
      if (!el) return;
      el.focus();
      el.setSelectionRange(next.caret, next.caret);
    });
  };

  const choose = (item: MenuItem) => {
    if (!open) return;
    switch (item.kind) {
      case "shape":
        props.onShape(item.id);
        setFlash((n) => n + 1);
        place(removeTrigger(idea, open));
        return;
      case "style":
        props.onLook({ kind: "style", id: item.id });
        place(removeTrigger(idea, open));
        return;
      case "idea": {
        const preset = ideas.find((i) => i.id === item.id);
        if (preset) place(replaceTrigger(idea, open, preset.idea));
        return;
      }
      case "prompt": {
        const saved = prompts.find((p) => p.id === item.id);
        if (!saved) return;
        // Its look comes with it; words saved without one leave the look picked as it is.
        if (item.look) props.onLook(item.look);
        place(replaceTrigger(idea, open, promptText(saved)));
        return;
      }
      case "save":
        setNaming({ name: suggestName(textWithout), text: textWithout, busy: false });
        return;
    }
  };

  const stopNaming = () => {
    setNaming(null);
    requestAnimationFrame(() => input.current?.focus());
  };

  const saveNamed = async () => {
    if (!naming || !open) return;
    setNaming({ ...naming, busy: true });
    const ok = await props.onSavePrompt(naming.name, naming.text, look);
    if (!ok) {
      setNaming((n) => (n ? { ...n, busy: false } : n));
      return;
    }
    setNaming(null);
    place(removeTrigger(idea, open));
  };

  // ---------- sending ----------

  const canSend = Boolean(idea.trim()) && (Boolean(model) || loading) && !blocked && !queued;
  const submit = (e?: FormEvent) => {
    e?.preventDefault();
    if (canSend) onSend();
  };
  const onKey = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.nativeEvent.isComposing) return;
    if (open && items.length > 0) {
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        setActive((a) => moveActive(a, e.key === "ArrowDown" ? 1 : -1, items.length));
        return;
      }
      if ((e.key === "Enter" && !e.shiftKey) || e.key === "Tab") {
        e.preventDefault();
        choose(items[Math.min(active, items.length - 1)]);
        return;
      }
    }
    if (open && e.key === "Escape") {
      // Put away, not taken out: the words stay as typed.
      e.preventDefault();
      e.stopPropagation();
      setDismissed(open.start);
      return;
    }
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      submit();
    }
  };

  const status = provider?.kind === "local" ? (ready ? t("ai.prompt.status.localReady") : t("ai.prompt.status.localNotReady")) : ready ? t("ai.prompt.status.keySaved") : t("ai.prompt.status.noKey");
  const where = `${providerName(provider?.label ?? "")} · ${model?.label ?? ""}`;
  const modelName = model?.label ?? t("ai.prompt.thisModel");
  const activeRow = open && items.length > 0 ? `${menuId}-${Math.min(active, items.length - 1)}` : undefined;
  const menuShown = open !== null && !(open.kind === "@" && shapes.length === 0);
  const lookTip = look ? (look.kind === "style" ? t("ai.prompt.styleTip", { description: styleDescription(look.id) }) : t("ai.prompt.skillTip")) : "";
  const named = naming ? namesSomeone(naming.text) : null;
  return (
    <form className={dropping ? "composer is-drop-target" : "composer"} onSubmit={submit} ref={setForm}>
      <textarea
        ref={input}
        className="composer-input"
        rows={rows}
        value={idea}
        placeholder={dropping ? t("ai.prompt.dropRef") : placeholder}
        aria-label={t("ai.prompt.label")}
        aria-autocomplete="list"
        aria-controls={menuShown && !naming ? menuId : undefined}
        aria-activedescendant={menuShown && !naming ? activeRow : undefined}
        spellCheck
        onChange={(e) => {
          onIdea(e.target.value);
          read(e.target.value, e.target.selectionStart ?? e.target.value.length);
        }}
        onKeyDown={onKey}
        onKeyUp={(e) => {
          // The caret moved without the words changing: the menu follows it.
          if (e.key.startsWith("Arrow") && !open) readCaret();
          else if (e.key === "Home" || e.key === "End") readCaret();
        }}
        onClick={readCaret}
        onBlur={() => {
          // Leaving the box puts the menu away, unless it's for naming a prompt, which is in it.
          requestAnimationFrame(() => {
            if (!document.activeElement?.closest(".pm")) setTrigger(null);
          });
        }}
      />
      {(refs.length > 0 || adding || look) && (
        <div className="composer-refs">
          {look && (
            <span className="composer-ref composer-style" data-tip={lookTip}>
              <span className="composer-style-glyph" aria-hidden="true">
                <PaletteIcon size={14} />
              </span>
              <span className="composer-ref-name">{lookName(look)}</span>
              <button type="button" className="composer-ref-x" aria-label={t("ai.prompt.removeStyleLabel", { style: lookName(look) })} data-tip={t("ai.prompt.removeRef")} onClick={() => props.onLook(null)}>
                <XIcon size={12} />
              </button>
            </span>
          )}
          {refs.map((r, i) => (
            <RefChip
              key={r.id}
              picture={r}
              unused={i >= limit}
              unusedTip={limit === 0 ? t("ai.prompt.noRefs", { model: modelName }) : t("ai.prompt.refLimit", { model: modelName, count: limit })}
              onRole={(role) => props.onRefRole(r.id, role)}
              onRemove={() => onRemoveRef(r.id)}
            />
          ))}
          {adding && (
            <span className="composer-ref is-adding">
              <LoaderIcon size={13} /> {t("ai.prompt.addingRef")}
            </span>
          )}
        </div>
      )}
      <div className="composer-bar">
        <ShapePicker
          shapes={shapes}
          shape={shape}
          make={make}
          flash={flash}
          onShape={(id) => {
            props.onShape(id);
            requestAnimationFrame(() => input.current?.focus());
          }}
          onMake={props.onMake}
        />
        <button
          type="button"
          className="icon-btn composer-attach"
          aria-label={t("ai.prompt.addRefLabel")}
          disabled={limit === 0}
          data-tip={limit === 0 ? t("ai.prompt.cantRef", { model: modelName }) : t("ai.prompt.addRef")}
          onClick={onAddRef}
        >
          <PaperclipIcon />
        </button>
        <span className="composer-spacer" />
        <button type="button" className="model-pill" onClick={onSettings} data-tip={provider ? t("ai.prompt.pillTip", { where, status: status.charAt(0).toLocaleLowerCase() + status.slice(1) }) : t("ai.prompt.choose")}>
          <span className={ready ? "model-dot is-ready" : "model-dot"} aria-hidden="true" />
          {provider?.kind === "local" ? <CpuIcon size={14} /> : provider ? <ProviderLogo id={provider.id} size={14} /> : loading && <LoaderIcon size={14} />}
          <span className="model-pill-text">{provider ? where : loading ? t("ai.prompt.loading") : t("ai.prompt.chooseProvider")}</span>
          <SlidersHorizontalIcon size={14} />
        </button>
        <button
          type="submit"
          className="send-btn"
          disabled={!canSend}
          aria-label={t("ai.prompt.generateLabel")}
          aria-busy={queued || undefined}
          data-tip={blocked ?? (queued ? t("ai.prompt.queued") : t("ai.prompt.generate"))}
          data-tip-kbd={blocked || queued ? undefined : "Enter"}
        >
          {queued ? <LoaderIcon size={16} /> : <ArrowUpIcon size={17} />}
        </button>
      </div>
      {menuShown && form.current && (
        <PromptMenu
          id={menuId}
          anchor={form.current}
          label={open.kind === "@" ? t("ai.shape.menuLabel") : t("ai.slash.label")}
          groups={groups}
          active={Math.min(active, Math.max(0, items.length - 1))}
          onActive={setActive}
          onChoose={choose}
          onRemove={(item) => {
            const saved = prompts.find((p) => p.id === item.id);
            if (saved) props.onRemovePrompt(saved);
          }}
          shapes={shapes}
          empty={open.kind === "@" ? t("ai.shape.none", { query: open.query }) : t("ai.slash.none", { query: open.query })}
          naming={
            naming
              ? {
                  name: naming.name,
                  onName: (name) => setNaming((n) => (n ? { ...n, name } : n)),
                  taken: takenBy(prompts, naming.name)?.name ?? null,
                  named,
                  look: look ? lookName(look) : null,
                  busy: naming.busy,
                  onSave: () => void saveNamed(),
                  onCancel: stopNaming,
                  onLeave: () => {
                    setNaming(null);
                    setTrigger(null);
                  },
                }
              : null
          }
        />
      )}
    </form>
  );
});
