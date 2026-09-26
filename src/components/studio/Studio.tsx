import { forwardRef, useCallback, useEffect, useImperativeHandle, useLayoutEffect, useMemo, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, errorMessage, type AiCatalogue, type SavedPrompt, type Skin } from "../../lib/tauri";
import { explain } from "../../lib/sentences";
import { isTauri } from "../../lib/devMock";
import { IMAGE_EXTENSIONS } from "../../lib/files";
import { CHIP_STYLES, styleTags, suggestion, surprise as surprisePick } from "../../lib/prompts";
import { lookName, sameLook, styleById, styleDescription, styleName, type Look } from "../../lib/styles";
import { shapeName, shapeOf, type ShapeInfo } from "../../lib/shapes";
import { clip } from "../../lib/names";
import type { ToastTone } from "../../hooks/useToasts";
import { ask, deleteChat, dismissProblem, keepReference, openChat, renameChatTo, setChatBase, setChatFolder, startChats, startNewChat, stop, useChats } from "../../state/chatStore";
import { loadPrompts, removePrompt, restorePrompt, savePrompt, useSavedPrompts } from "../../state/savedPrompts";
import { useLook } from "../../state/look";
import type { ChatFolder, ChatRef, ChatSummary, Shape, Turn } from "../../state/chats";
import type { ApplyOutcome } from "../composer/Composer";
import { Confirm } from "../Confirm";
import { StudioSettings } from "../StudioSettings";
import { ChatHelper } from "../ChatHelper";
import { HistoryIcon, SquarePenIcon } from "../icons/composer";
import { SparklesIcon } from "../icons/sparkles";
import { ChatDrawer } from "./ChatDrawer";
import { FolderTarget } from "./FolderTarget";
import { paintsOnTemplate, PromptBox, refLimit } from "./PromptBox";
import { TurnCard, type TurnActions } from "./TurnCard";
import { branded } from "../Brand";
import { t as tNow, useT } from "../../i18n";
import { chatTitle } from "../../state/chats";

const CHOICE_KEY = "folderskin.ai.choice";

/** The provider, model and what to make (the whole shape or just the art) last used, so the next visit starts where this one left off. */
function loadChoice(): { provider: string; model: string; shape: Shape } {
  try {
    const v = JSON.parse(localStorage.getItem(CHOICE_KEY) ?? "{}") as Partial<Record<"provider" | "model" | "shape", unknown>>;
    return { provider: typeof v.provider === "string" ? v.provider : "", model: typeof v.model === "string" ? v.model : "", shape: v.shape === "skin" ? "skin" : "folder" };
  } catch {
    return { provider: "", model: "", shape: "folder" };
  }
}

export type StudioHandle = {
  /** A picture dropped on the AI view: it becomes a reference for the next request. */
  addReference: (path: string) => void;
};

/**
 * The AI assistant, as a chat. Each request is a card that develops into its picture while it
 * runs, showing what's happening and how far it is, with the log a click away; the result can be
 * previewed on the chosen folder and applied from the card. Chats are kept (chatStore.ts) and
 * listed in a drawer; a request carries on in its own chat while the user looks elsewhere.
 * Provider, model and key, or setting up this computer, live in a dialog behind the model pill.
 *
 * The chat on screen lasts as long as the app is open: this view stays mounted while another is
 * on show, so the words in the box, the pictures and style with them, where the chat was scrolled
 * to and whatever is still being made are all as they were on coming back. Only the next launch
 * opens on a fresh chat.
 */
export const Studio = forwardRef<
  StudioHandle,
  {
    active: boolean;
    os: string;
    /** The folder the pictures are for: the one on show on the right. */
    folder: ChatFolder | null;
    /** The skin tried on that folder now, and the one it wears. */
    shownId: string | null;
    appliedId: string | null;
    panelShown: boolean;
    onTogglePanel: () => void;
    onChooseFolder: () => void;
    /** Makes `path` the folder on show (a chat's own folder, when it's opened), or none. */
    onUseFolder: (path: string | null) => void;
    onPreview: (skinId: string) => void;
    onApply: (skin: Skin) => Promise<ApplyOutcome>;
    onGenerated: (skin: Skin) => void;
    onImport: () => void;
    skinOf: (skinId: string) => Skin | undefined;
    onMenu: (skin: Skin, anchor: HTMLElement) => void;
    /** Changes when a key is saved or removed in Settings, so the providers are read again. */
    keysVersion: number;
    /** A picture is being dragged over the window. */
    dragImage: boolean;
    toast: (text: string, opts?: { tone?: ToastTone; action?: { label: string; run: () => void } }) => void;
  }
>(function Studio(props, ref) {
  const { active, folder, shownId, appliedId, panelShown, skinOf, toast } = props;
  const t = useT();
  const chats = useChats();
  const chat = chats.active;
  const folderLook = useLook();
  const saved = useSavedPrompts();
  const [catalogue, setCatalogue] = useState<AiCatalogue | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [shapes, setShapes] = useState<ShapeInfo[]>([]);
  const initial = useMemo(loadChoice, []);
  const [providerId, setProviderId] = useState(initial.provider);
  const [modelId, setModelId] = useState(initial.model);
  const [make, setMake] = useState<Shape>(initial.shape);
  const [idea, setIdea] = useState("");
  /** The style chip whose idea is in the box, and which of its ideas (clicking again cycles). */
  const [pick, setPick] = useState<{ styleId: string; index: number } | null>(null);
  /** The look picked (a style from a chip or the "/" menu, or a saved prompt's), which goes to the model in a slot of its own. */
  const [look, setLook] = useState<Look | null>(null);
  const [refs, setRefs] = useState<ChatRef[]>([]);
  const [adding, setAdding] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [helperOpen, setHelperOpen] = useState(false);
  const [drawer, setDrawer] = useState(false);
  const [deleting, setDeleting] = useState<ChatSummary | null>(null);
  const input = useRef<HTMLTextAreaElement>(null);
  const box = useRef<HTMLFormElement>(null);
  const thread = useRef<HTMLDivElement>(null);

  // The first time the view is shown this session, a new chat opens with the folder chosen in
  // the library, the earlier chats are read into the history, and so are the saved prompts. After
  // that the chat on screen is the one to come back to.
  useEffect(() => {
    if (!active) return;
    startChats(folder);
    void loadPrompts();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active]);

  // Hidden, the thread loses where it was scrolled to (a hidden element has no layout); it's put
  // back before the view is painted again, or taken to the newest card when one arrived or
  // finished meanwhile.
  const scrolled = useRef(0);
  const arrivedAway = useRef(false);
  useLayoutEffect(() => {
    const el = thread.current;
    if (!active || !el) return;
    el.scrollTop = arrivedAway.current ? el.scrollHeight : scrolled.current;
    arrivedAway.current = false;
  }, [active]);

  // The shapes the chat can paint on, drawn once.
  useEffect(() => {
    api
      .aiShapes()
      .then(setShapes)
      .catch(() => setShapes([]));
  }, []);

  const load = useCallback(() => {
    setLoadError(null);
    api
      .aiCatalogue()
      .then((c) => {
        setCatalogue(c);
        setProviderId((p) => (c.providers.some((x) => x.id === p) ? p : (c.providers.find((x) => x.has_key)?.id ?? c.providers[0]?.id ?? "")));
      })
      .catch((e) => setLoadError(errorMessage(e)));
  }, []);
  useEffect(() => load(), [load, props.keysVersion]);

  const provider = catalogue?.providers.find((p) => p.id === providerId);
  const model = provider?.models.find((m) => m.id === modelId) ?? provider?.models[0];
  useEffect(() => {
    if (provider && !provider.models.some((m) => m.id === modelId)) setModelId(provider.models[0]?.id ?? "");
  }, [provider, modelId]);
  useEffect(() => {
    try {
      localStorage.setItem(CHOICE_KEY, JSON.stringify({ provider: providerId, model: modelId, shape: make }));
    } catch {
      // Only a preference.
    }
  }, [providerId, modelId, make]);

  // The shape is part of the chat: the one it was left on, or for a new chat, the folder the app
  // puts skins on. Until the shapes are in, that folder by name.
  const shape: ShapeInfo | undefined =
    shapeOf(shapes, chat?.base, folderLook) ??
    (shapes.length === 0 ? { id: `${folderLook}-folder`, label: "", family: "folder", system: folderLook, whole: true, thumbnail: null } : undefined);
  // The chat opened shows its own folder; a folder chosen while it's open becomes its folder.
  const chatId = chat?.id ?? null;
  const chatFolder = chat?.folder?.path ?? null;
  const shownChat = useRef<string | null>(null);
  useEffect(() => {
    // Only when another chat is opened, not on coming back to this view: a folder chosen in the
    // library meanwhile is the one to keep, and becomes the chat's.
    if (!active || chatId === shownChat.current) return;
    shownChat.current = chatId;
    if (chatFolder && chatFolder !== folder?.path) props.onUseFolder(chatFolder);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [chatId, active]);
  useEffect(() => {
    if (active && folder) setChatFolder(folder);
  }, [active, folder]);

  // A chat's reference pictures are copied into its own folder (chats.rs), so they're for that
  // chat alone: another one starts with none, and without the style picked for this one.
  const openChatId = useRef(chatId);
  useEffect(() => {
    openChatId.current = chatId;
    setRefs([]);
    setLook(null);
  }, [chatId]);

  // The newest card in view as it arrives and as it grows into its result.
  const last = chat?.turns.at(-1);
  useEffect(() => {
    if (!active) {
      arrivedAway.current = true;
      return;
    }
    thread.current?.scrollTo({ top: thread.current.scrollHeight, behavior: "smooth" });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [chat?.id, chat?.turns.length, last?.status]);

  const addReference = useCallback(
    async (path: string) => {
      setAdding(true);
      const at = openChatId.current;
      try {
        const kept = await keepReference(path);
        // Another chat was opened while it was copied: it was kept for the one it was added to.
        if (openChatId.current !== at) return;
        setRefs((rs) => (rs.some((r) => r.id === kept.id) ? rs : [...rs, kept]));
      } catch (e) {
        toast(tNow("ai.studio.refFailed", { reason: errorMessage(e) }), { tone: "danger" });
      } finally {
        setAdding(false);
      }
    },
    [toast],
  );
  useImperativeHandle(ref, () => ({ addReference: (path) => void addReference(path) }), [addReference]);

  const pickReference = useCallback(async () => {
    if (!isTauri()) return addReference("/Users/you/Pictures/Reference.jpg");
    const picked = await open({ multiple: false, title: tNow("ai.studio.chooseRef"), filters: [{ name: tNow("common.dialog.pictures"), extensions: IMAGE_EXTENSIONS }] }).catch(() => null);
    if (typeof picked === "string") await addReference(picked);
  }, [addReference]);

  const openSettings = useCallback((at?: string) => {
    if (at) setProviderId(at);
    setSettingsOpen(true);
  }, []);

  // This computer paints one picture at a time, whichever chat asked for the one it's on.
  const blocked = provider?.kind === "local" && chats.localRunning ? t("ai.studio.localBusy") : null;

  /** Sent before the providers were in (the first list of a session takes a moment): it goes when they are. */
  const [queued, setQueued] = useState(false);
  const send = () => {
    const text = idea.trim();
    if (!text) return;
    if (!catalogue) {
      setQueued(true);
      return;
    }
    if (!provider || !model || !shape) return;
    // Nothing set up for it yet: keep the words and open what fixes that.
    if (!provider.has_key) {
      openSettings(provider.id);
      return;
    }
    const sent = ask(
      {
        idea: text,
        shape: make,
        base: shape.id,
        look,
        provider: provider.id,
        model: model.id,
        where: `${provider.label} · ${model.label}`,
        local: provider.kind === "local",
        refs: refs.slice(0, refLimit(model, paintsOnTemplate(shape, make))),
        tags: styleTags(text),
        size: model.sizes[0] ?? null,
      },
      props.onGenerated,
    );
    if (!sent) return;
    setIdea("");
    setPick(null);
    setRefs([]);
  };

  useEffect(() => {
    if (!queued || !catalogue) return;
    setQueued(false);
    send();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [queued, catalogue]);

  const act: TurnActions = {
    again: (turn: Turn) => {
      const p = catalogue?.providers.find((x) => x.id === turn.provider);
      const m = p?.models.find((x) => x.id === turn.model);
      if (p && !p.has_key) return openSettings(p.id);
      const local = p?.kind === "local";
      const sent = ask(
        {
          idea: turn.idea,
          shape: turn.shape,
          // Made again on the shape it was made for, or, from before shapes, the folder it was.
          base: turn.base ?? shapeOf(shapes, undefined, folderLook)?.id ?? `${folderLook}-folder`,
          look: turn.skill ? { kind: "skill", ...turn.skill } : turn.style ? { kind: "style", id: turn.style } : null,
          provider: turn.provider,
          model: turn.model,
          where: p && m ? `${p.label} · ${m.label}` : turn.where,
          local,
          refs: turn.refs,
          tags: styleTags(turn.idea),
          size: m?.sizes[0] ?? null,
        },
        props.onGenerated,
      );
      if (!sent && local) toast(tNow("ai.studio.localBusyToast"));
    },
    reword: (turn) => {
      setIdea(turn.idea);
      requestAnimationFrame(() => {
        input.current?.focus();
        input.current?.setSelectionRange(turn.idea.length, turn.idea.length);
      });
    },
    stop: (turn) => stop(turn.id),
    settings: (at) => openSettings(at),
    preview: (skin) => {
      props.onPreview(skin.id);
      if (!panelShown) props.onTogglePanel();
    },
    apply: async (skin) => {
      const r = await props.onApply(skin);
      if (r.ok || r.message)
        toast(r.message ?? tNow("ai.studio.nowWears", { folder: folder ? clip(folder.name) : tNow("ai.studio.theFolder"), skin: clip(skin.name) }), { tone: r.tone ?? "ok", action: r.action });
    },
    chooseFolder: props.onChooseFolder,
    menu: props.onMenu,
    copy: (text) => {
      navigator.clipboard
        .writeText(text)
        .then(() => toast(tNow("ai.studio.questionCopied"), { tone: "ok" }))
        .catch(() => toast(tNow("ai.studio.copyFailed"), { tone: "danger" }));
    },
  };

  /** Keeps the words in the box as a prompt, with the look picked; resolves to whether they were. */
  const keepPrompt = async (name: string, text: string, withLook: Look | null) => {
    try {
      const { saved: kept, replaced } = await savePrompt(name, text, withLook);
      toast(tNow(replaced ? "ai.slash.replaced" : "ai.slash.saved", { name: clip(kept.name) }), { tone: "ok" });
      return true;
    } catch (e) {
      toast(tNow("ai.slash.notSaved", { reason: explain(errorMessage(e)) }), { tone: "danger" });
      return false;
    }
  };
  /** Removes a saved prompt, with the chance to put it back where it was, as it was. */
  const dropPrompt = (prompt: SavedPrompt) => {
    removePrompt(prompt)
      .then((at) =>
        toast(tNow("ai.slash.removed", { name: clip(prompt.name) }), {
          tone: "ok",
          action: {
            label: tNow("ai.slash.undo"),
            run: () => void restorePrompt(prompt, at).catch((e) => toast(tNow("ai.slash.notSaved", { reason: explain(errorMessage(e)) }), { tone: "danger" })),
          },
        }),
      )
      .catch((e) => toast(tNow("ai.slash.notRemoved", { reason: explain(errorMessage(e)) }), { tone: "danger" }));
  };
  // The list that couldn't be read is said once, when the menu that shows it is first wanted.
  const [savedProblemSaid, setSavedProblemSaid] = useState(false);
  useEffect(() => {
    if (!saved.problem || savedProblemSaid || !active) return;
    setSavedProblemSaid(true);
    toast(tNow("ai.slash.notRead", { reason: saved.problem }), { tone: "danger" });
  }, [saved.problem, savedProblemSaid, active, toast]);

  /** Puts a brief in the box, flashes the box so the change is seen, and parks the caret at the end. */
  const fill = (text: string) => {
    setIdea(text);
    requestAnimationFrame(() => {
      const el = input.current;
      if (el) {
        el.focus();
        el.setSelectionRange(text.length, text.length);
        el.scrollTop = el.scrollHeight;
      }
      const root = getComputedStyle(document.documentElement);
      box.current?.animate(
        [
          { backgroundColor: root.getPropertyValue("--accent-wash").trim(), borderColor: root.getPropertyValue("--accent").trim() },
          { backgroundColor: root.getPropertyValue("--island").trim(), borderColor: root.getPropertyValue("--line").trim() },
        ],
        { duration: 700, easing: "cubic-bezier(0.2, 0.8, 0.2, 1)" },
      );
    });
  };
  /** A chip puts its style in the look's slot and one of its ideas in the box; clicking it again brings the next idea. */
  const choose = (chipId: string) => {
    const index = pick?.styleId === chipId && sameLook(look, { kind: "style", id: chipId }) ? pick.index + 1 : 0;
    setPick({ styleId: chipId, index });
    setLook({ kind: "style", id: chipId });
    fill(suggestion(chipId, index));
  };
  const surprise = () => {
    const next = surprisePick();
    setPick({ styleId: next.styleId, index: next.index });
    setLook({ kind: "style", id: next.styleId });
    fill(next.text);
  };
  /** A chip reads as chosen while its style is the one picked. */
  const chosen = look?.kind === "style" ? (styleById(look.id)?.id ?? null) : null;

  if (loadError) {
    return (
      <section className="studio" hidden={!active}>
        <div className="empty">
          <span className="empty-glyph">
            <SparklesIcon size={22} />
          </span>
          <p className="empty-title">{t("ai.studio.noAssistant")}</p>
          <p className="empty-text">{branded(t("ai.studio.noAssistantText", { reason: loadError }))}</p>
          <button type="button" className="btn btn-secondary" onClick={load}>
            {t("community.tryAgain")}
          </button>
        </div>
      </section>
    );
  }

  const turns = chat?.turns ?? [];
  const hasThread = turns.length > 0;
  const local = provider?.kind === "local";
  // The box asks for what the shape is: a folder, a drive, or an icon of its own.
  const family = shape?.family ?? "folder";
  const placeholderFor =
    family === "free"
      ? folder
        ? t("ai.studio.placeholderIconFor", { name: clip(folder.name) })
        : t("ai.studio.placeholderIcon")
      : family === "drive"
        ? folder
          ? t("ai.studio.placeholderDriveFor", { name: clip(folder.name) })
          : t("ai.studio.placeholderDrive")
        : folder
          ? t("ai.studio.placeholderFor", { name: clip(folder.name) })
          : t("ai.studio.placeholder");
  // Before the chat starts, the box says "/" is there too.
  const placeholder = hasThread ? placeholderFor : t("ai.studio.placeholderHint", { placeholder: placeholderFor });
  const foot = !provider
    ? null
    : local
      ? provider.has_key
        ? null
        : t("ai.studio.foot.localNotReady")
      : provider.has_key
        ? t("ai.studio.foot.billed", { price: model?.price_hint ? explain(model.price_hint) : t("ai.studio.foot.priced"), provider: provider.label })
        : t("ai.studio.foot.addKey", { provider: provider.label });
  // Once a chat has started the box sits at the bottom with nothing under it, unless the
  // provider can't paint yet: then the line says why, and offers the way without a key.
  const showFoot = !hasThread || !provider?.has_key;

  return (
    <section className={hasThread ? "studio has-thread" : "studio"} hidden={!active} aria-label={t("ai.studio.label")}>
      <header className="studio-head" data-tauri-drag-region>
        <button
          type="button"
          className={drawer ? "icon-btn studio-chats-btn is-on" : "icon-btn studio-chats-btn"}
          aria-label={t("ai.chats.label")}
          aria-expanded={drawer}
          data-tip={t("ai.chats.title")}
          data-tip-side="bottom"
          onClick={() => setDrawer((d) => !d)}
        >
          <HistoryIcon size={17} />
        </button>
        <p className="studio-chat-title" data-tip={chat ? chatTitle(chat.title) : undefined} data-tip-overflow>
          {chat ? chatTitle(chat.title) : t("ai.chats.newChat")}
        </p>
        <span className="studio-head-space" data-tauri-drag-region />
        <FolderTarget
          folder={folder}
          panelShown={panelShown}
          onChoose={props.onChooseFolder}
          onTogglePanel={props.onTogglePanel}
          onClear={() => {
            setChatFolder(null);
            props.onUseFolder(null);
          }}
        />
        <button type="button" className="icon-btn" aria-label={t("ai.chats.newChatLabel")} data-tip={t("ai.chats.newChat")} data-tip-side="bottom" onClick={() => startNewChat(folder)}>
          <SquarePenIcon size={16} />
        </button>
      </header>

      {chats.problem && (
        <p className="studio-problem" role="status">
          {chats.problem}
          <button type="button" className="link-btn" onClick={dismissProblem}>
            {t("ai.studio.ok")}
          </button>
        </p>
      )}

      <div
        className="studio-thread"
        ref={thread}
        onScroll={(e) => {
          if (active) scrolled.current = e.currentTarget.scrollTop;
        }}
      >
        {turns.map((turn) => (
          <TurnCard
            key={turn.id}
            turn={turn}
            live={turn.skinId ? skinOf(turn.skinId) : undefined}
            shape={shapeOf(shapes, turn.base, folderLook)}
            shapeLabel={turnShape(turn, shapes)}
            folderName={folder?.name ?? null}
            onFolder={turn.skinId !== undefined && turn.skinId === shownId && panelShown}
            applied={turn.skinId !== undefined && turn.skinId === appliedId}
            act={act}
          />
        ))}
      </div>

      <div className="studio-dock">
        {!hasThread && (
          <div className="studio-hero">
            <span className="studio-hero-glyph">
              <SparklesIcon size={22} playOnMount />
            </span>
            <h2 className="studio-title">{t("ai.studio.heroTitle")}</h2>
            <p className="studio-sub">
              {/* Where it goes once the providers are in: until then, nothing that might not be so. */}
              {!catalogue
                ? t("ai.studio.heroSub")
                : local
                  ? t("ai.studio.heroSubLocal")
                  : t("ai.studio.heroSubKey", { provider: provider?.label ?? t("ai.studio.theProvider") })}
            </p>
          </div>
        )}

        <PromptBox
          ref={input}
          boxRef={box}
          idea={idea}
          onIdea={setIdea}
          placeholder={placeholder}
          rows={hasThread ? 1 : 2}
          refs={refs}
          adding={adding}
          onAddRef={() => void pickReference()}
          onRemoveRef={(id) => setRefs((rs) => rs.filter((r) => r.id !== id))}
          onRefRole={(id, role) => setRefs((rs) => rs.map((r) => (r.id === id ? { ...r, role } : r)))}
          shapes={shapes}
          shape={shape}
          onShape={setChatBase}
          make={make}
          onMake={setMake}
          look={look}
          onLook={(next) => {
            setLook(next);
            if (!next) setPick(null);
          }}
          prompts={saved.list}
          ideas={catalogue?.presets ?? []}
          onSavePrompt={keepPrompt}
          onRemovePrompt={dropPrompt}
          provider={provider}
          model={model}
          onSettings={() => openSettings()}
          onSend={send}
          loading={!catalogue}
          queued={queued}
          blocked={blocked}
          dropping={props.dragImage}
        />

        {!hasThread && (
          <div className="style-chips" aria-label={t("ai.studio.stylesLabel")}>
            {CHIP_STYLES.map((s) => (
              <button
                key={s.id}
                type="button"
                aria-pressed={chosen === s.id}
                className={chosen === s.id ? "style-chip is-active" : "style-chip"}
                data-tip={chosen === s.id ? t("ai.studio.styleAgain") : styleDescription(s.id)}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => choose(s.id)}
              >
                {styleName(s.id)}
              </button>
            ))}
            <button type="button" className="style-chip is-surprise" onMouseDown={(e) => e.preventDefault()} onClick={surprise}>
              <SparklesIcon size={13} /> {t("ai.studio.surprise")}
            </button>
          </div>
        )}
        {showFoot && (foot || (catalogue && !local)) && (
          <p className="studio-foot">
            {foot && <span>{foot}</span>}
            {catalogue && !local && (
              <button type="button" className="link-btn" onClick={() => setHelperOpen(true)}>
                {t("ai.studio.noKey")}
              </button>
            )}
          </p>
        )}
      </div>

      <ChatDrawer
        open={drawer}
        list={chats.list}
        activeId={chat?.turns.length ? chat.id : null}
        skinOf={skinOf}
        onOpen={(id) => {
          void openChat(id);
          setDrawer(false);
        }}
        onNew={() => {
          startNewChat(folder);
          setDrawer(false);
          requestAnimationFrame(() => input.current?.focus());
        }}
        onRename={renameChatTo}
        onDelete={setDeleting}
        onClose={() => setDrawer(false)}
      />

      {deleting && (
        <Confirm
          title={t("ai.chats.deleteTitle", { name: clip(chatTitle(deleting.title)) })}
          text={t("ai.chats.deleteText")}
          action={t("library.delete.action")}
          onCancel={() => setDeleting(null)}
          onConfirm={() => {
            const d = deleting;
            setDeleting(null);
            void deleteChat(d.id);
          }}
        />
      )}
      {settingsOpen && catalogue && (
        <StudioSettings
          catalogue={catalogue}
          providerId={providerId}
          modelId={model?.id ?? ""}
          onProvider={setProviderId}
          onModel={setModelId}
          onChanged={load}
          onClose={() => setSettingsOpen(false)}
          toast={toast}
        />
      )}
      {helperOpen && (
        <ChatHelper
          scene={idea}
          styleId={look?.kind === "style" ? look.id : null}
          onImport={() => {
            setHelperOpen(false);
            props.onImport();
          }}
          onClose={() => setHelperOpen(false)}
          toast={toast}
        />
      )}
    </section>
  );
});

/**
 * What a card says the picture was made for: its shape by name, with the look picked for it. A
 * card from before shapes names none, as it was always a folder's.
 */
function turnShape(turn: Turn, shapes: ShapeInfo[]): string | null {
  const shape = turn.base ? shapes.find((s) => s.id === turn.base) : undefined;
  const look = turn.skill ? lookName({ kind: "skill", ...turn.skill }) : turn.style && styleById(turn.style) ? styleName(turn.style) : null;
  const parts = [shape ? shapeName(shape) : null, look].filter(Boolean);
  return parts.length ? parts.join(" · ") : null;
}
