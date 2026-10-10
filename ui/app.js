"use strict";

(() => {
  const { invoke } = window.__TAURI__.core;
  const { listen } = window.__TAURI__.event;
  const pickFolder = (window.__TAURI__.dialog || {}).open;
  const FitAddonClass = window.FitAddon.FitAddon || window.FitAddon;

  // ---------------------------------------------------------------- assets

  const LOGOS = {
    claude: '<svg viewBox="0 0 24 24" fill="currentColor"><path d="m4.7144 15.9555 4.7174-2.6471.079-.2307-.079-.1275h-.2307l-.7893-.0486-2.6956-.0729-2.3375-.0971-2.2646-.1214-.5707-.1215-.5343-.7042.0546-.3522.4797-.3218.686.0608 1.5179.1032 2.2767.1578 1.6514.0972 2.4468.255h.3886l.0546-.1579-.1336-.0971-.1032-.0972L6.973 9.8356l-2.55-1.6879-1.3356-.9714-.7225-.4918-.3643-.4614-.1578-1.0078.6557-.7225.8803.0607.2246.0607.8925.686 1.9064 1.4754 2.4893 1.8336.3643.3035.1457-.1032.0182-.0728-.164-.2733-1.3539-2.4467-1.445-2.4893-.6435-1.032-.17-.6194c-.0607-.255-.1032-.4674-.1032-.7285L6.287.1335 6.6997 0l.9957.1336.419.3642.6192 1.4147 1.0018 2.2282 1.5543 3.0296.4553.8985.2429.8318.091.255h.1579v-.1457l.1275-1.706.2368-2.0947.2307-2.6957.0789-.7589.3764-.9107.7468-.4918.5828.2793.4797.686-.0668.4433-.2853 1.8517-.5586 2.9021-.3643 1.9429h.2125l.2429-.2429.9835-1.3053 1.6514-2.0643.7286-.8196.85-.9046.5464-.4311h1.0321l.759 1.1293-.34 1.1657-1.0625 1.3478-.8804 1.1414-1.2628 1.7-.7893 1.36.0729.1093.1882-.0183 2.8535-.607 1.5421-.2794 1.8396-.3157.8318.3886.091.3946-.3278.8075-1.967.4857-2.3072.4614-3.4364.8136-.0425.0304.0486.0607 1.5482.1457.6618.0364h1.621l3.0175.2247.7892.522.4736.6376-.079.4857-1.2142.6193-1.6393-.3886-3.825-.9107-1.3113-.3279h-.1822v.1093l1.0929 1.0686 2.0035 1.8092 2.5075 2.3314.1275.5768-.3218.4554-.34-.0486-2.2039-1.6575-.85-.7468-1.9246-1.621h-.1275v.17l.4432.6496 2.3436 3.5214.1214 1.0807-.17.3521-.6071.2125-.6679-.1214-1.3721-1.9246L14.38 17.959l-1.1414-1.9428-.1397.079-.674 7.2552-.3156.3703-.7286.2793-.6071-.4614-.3218-.7468.3218-1.4753.3886-1.9246.3157-1.53.2853-1.9004.17-.6314-.0121-.0425-.1397.0182-1.4328 1.9672-2.1796 2.9446-1.7243 1.8456-.4128.164-.7164-.3704.0667-.6618.4008-.5889 2.386-3.0357 1.4389-1.882.929-1.0868-.0062-.1579h-.0546l-6.3385 4.1164-1.1293.1457-.4857-.4554.0608-.7467.2307-.2429 1.9064-1.3114Z"/></svg>',
    openai: '<svg viewBox="0 0 24 24" fill="currentColor"><path d="M22.2819 9.8211a5.9847 5.9847 0 0 0-.5157-4.9108 6.0462 6.0462 0 0 0-6.5098-2.9A6.0651 6.0651 0 0 0 4.9807 4.1818a5.9847 5.9847 0 0 0-3.9977 2.9 6.0462 6.0462 0 0 0 .7427 7.0966 5.98 5.98 0 0 0 .511 4.9107 6.051 6.051 0 0 0 6.5146 2.9001A5.9847 5.9847 0 0 0 13.2599 24a6.0557 6.0557 0 0 0 5.7718-4.2058 5.9894 5.9894 0 0 0 3.9977-2.9001 6.0557 6.0557 0 0 0-.7475-7.0729zm-9.022 12.6081a4.4755 4.4755 0 0 1-2.8764-1.0408l.1419-.0804 4.7783-2.7582a.7948.7948 0 0 0 .3927-.6813v-6.7369l2.02 1.1686a.071.071 0 0 1 .038.052v5.5826a4.504 4.504 0 0 1-4.4945 4.4944zm-9.6607-4.1254a4.4708 4.4708 0 0 1-.5346-3.0137l.142.0852 4.783 2.7582a.7712.7712 0 0 0 .7806 0l5.8428-3.3685v2.3324a.0804.0804 0 0 1-.0332.0615L9.74 19.9502a4.4992 4.4992 0 0 1-6.1408-1.6464zM2.3408 7.8956a4.485 4.485 0 0 1 2.3655-1.9728V11.6a.7664.7664 0 0 0 .3879.6765l5.8144 3.3543-2.0201 1.1685a.0757.0757 0 0 1-.071 0l-4.8303-2.7865A4.504 4.504 0 0 1 2.3408 7.872zm16.5963 3.8558L13.1038 8.364 15.1192 7.2a.0757.0757 0 0 1 .071 0l4.8303 2.7913a4.4944 4.4944 0 0 1-.6765 8.1042v-5.6772a.79.79 0 0 0-.407-.667zm2.0107-3.0231l-.142-.0852-4.7735-2.7818a.7759.7759 0 0 0-.7854 0L9.409 9.2297V6.8974a.0662.0662 0 0 1 .0284-.0615l4.8303-2.7866a4.4992 4.4992 0 0 1 6.6802 4.66zM8.3065 12.863l-2.02-1.1638a.0804.0804 0 0 1-.038-.0567V6.0742a4.4992 4.4992 0 0 1 7.3757-3.4537l-.142.0805L8.704 5.459a.7948.7948 0 0 0-.3927.6813zm1.0976-2.3654l2.602-1.4998 2.6069 1.4998v2.9994l-2.5974 1.4997-2.6067-1.4997Z"/></svg>',
  };
  const LOGO_CLASS = { claude: "logo-claude", codex: "logo-openai" };
  const LOGO_KEY = { claude: "claude", codex: "openai" };

  const stroke = (body) =>
    `<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">${body}</svg>`;
  const ICONS = {
    plus: stroke('<path d="M8 3.5v9M3.5 8h9"/>'),
    expand: stroke('<path d="M9.5 2.5h4v4M6.5 13.5h-4v-4M13.5 2.5 9 7M2.5 13.5 7 9"/>'),
    collapse: stroke('<path d="M13 6.5H9.5V3M3 9.5h3.5V13M9.5 6.5 13.5 2.5M6.5 9.5l-4 4"/>'),
    restart: stroke('<path d="M2.75 8a5.25 5.25 0 1 0 1.6-3.75M2.75 2.75v2.5h2.5"/>'),
    more: '<svg viewBox="0 0 16 16" fill="currentColor"><circle cx="3.5" cy="8" r="1.1"/><circle cx="8" cy="8" r="1.1"/><circle cx="12.5" cy="8" r="1.1"/></svg>',
    terminal: stroke('<path d="m3.5 5 3 3-3 3M8.5 11.5h4"/>'),
    close: stroke('<path d="M4 4l8 8M12 4l-8 8"/>'),
    sparkle: stroke('<path d="M8 2.2 9.3 6.7 13.8 8 9.3 9.3 8 13.8 6.7 9.3 2.2 8 6.7 6.7z"/>'),
    alert: stroke('<circle cx="8" cy="8" r="6"/><path d="M8 5v3.6M8 10.9v.2"/>'),
    check: stroke('<path d="M3.5 8.5 6.5 11.5 12.5 4.5"/>'),
    back: stroke('<path d="M9.5 3.5 5 8l4.5 4.5"/>'),
    forward: stroke('<path d="M6.5 3.5 11 8l-4.5 4.5"/>'),
    globe: stroke('<circle cx="8" cy="8" r="5.5"/><path d="M2.5 8h11M8 2.5c1.8 1.8 1.8 9.2 0 11M8 2.5c-1.8 1.8-1.8 9.2 0 11"/>'),
    pencil: stroke('<path d="M10.25 3.25 12.75 5.75 6 12.5H3.5v-2.5z"/>'),
    folder: stroke('<path d="M2.5 4.5h3.25l1.5 1.5h6.25v6.5h-11z"/>'),
    copy: stroke('<rect x="5.5" y="5.5" width="8" height="8" rx="1.5"/><path d="M3.5 10.5v-6a1 1 0 0 1 1-1h6"/>'),
    stop: '<svg viewBox="0 0 16 16" fill="currentColor"><rect x="4" y="4" width="8" height="8" rx="1.5"/></svg>',
    home: stroke('<path d="M2.75 7.5 8 3.25l5.25 4.25V13a.75.75 0 0 1-.75.75H9.5V10h-3v3.75H3.5A.75.75 0 0 1 2.75 13z"/>'),
    gear: stroke('<circle cx="8" cy="8" r="2"/><path d="M8 1.75v1.7M8 12.55v1.7M1.75 8h1.7M12.55 8h1.7M3.55 3.55l1.2 1.2M11.25 11.25l1.2 1.2M3.55 12.45l1.2-1.2M11.25 4.75l1.2-1.2"/>'),
  };

  // ----------------------------------------------------------------- state

  const state = {
    store: { groups: [] },
    selected: 0,
    focusedId: null,
    maximizedId: null,
    renaming: null, // index of the group whose name is being edited
    renameDraft: "",
    renameFresh: false,
    status: {}, // id -> { running, exited, working }
    workingSince: new Map(), // id -> Date.now() when the current "working" stretch began
    done: new Set(), // ids whose work finished while the user was not looking
    dragId: null, // session id being dragged in the sidebar
    dragGroup: null, // group index being dragged in the sidebar
    tokens: {}, // session id -> tokens it has used, from the last count (see refreshTokens)
    newKind: "claude",
    newCount: 1, // how many sessions the New session dialog opens at once
    view: "home", // "home" | "group" | "settings"
    transcripts: null, // past sessions from disk; null until the first load finishes
    imageGen: { available: false, busy: false, error: "", stopped: false, prompt: "", images: [], selected: 0, previews: new Map(), revealed: new Set(), startedAt: 0, timer: 0 },
    resumeFilter: "all", // "all" | "claude" | "codex"
    resumeQuery: "",
    resumeShown: 8, // how many matches the list shows; "Show more" raises it
    resumeGroup: 0, // index of the group a resumed session is filed under
    arriving: false, // the next grid render plays the entrance animation on every card
  };
  const cards = new Map(); // id -> card entry (DOM, terminal, size bookkeeping)
  const pendingData = new Map(); // id -> bytes that arrived before the card existed
  const PENDING_LIMIT = 512 * 1024;

  const $ = (sel) => document.querySelector(sel);
  const el = {
    groupList: $("#group-list"),
    title: $("#group-title"),
    count: $("#group-count"),
    summary: $("#summary"),
    grid: $("#grid"),
    empty: $("#empty"),
    emptyTitle: $("#empty-title"),
    parking: $("#parking"),
    toasts: $("#toasts"),
    viewer: $("#viewer"),
    viewerName: $("#viewer-name"),
    viewerImage: $("#viewer-image"),
    viewerClose: $("#viewer-close"),
    recent: $("#recent"),
    sessionDialog: $("#new-session"),
    confirmDialog: $("#confirm"),
    promptDialog: $("#prompt"),
    toolbar: $("#toolbar"),
    stage: $("#stage"),
    home: $("#home"),
    homeSub: $("#home-sub"),
    homeHint: $("#home-hint"),
    liveBlock: $("#home-live"),
    liveList: $("#live-list"),
    workingBlock: $("#home-working"),
    workingList: $("#working-list"),
    groupCards: $("#group-cards"),
    resumeList: $("#resume-list"),
    resumeCount: $("#resume-count"),
    resumeGroup: $("#resume-group"),
    resumeSearch: $("#resume-search"),
    imagegenBlock: $("#imagegen-block"),
    imagegenForm: $("#imagegen-form"),
    imagegenPrompt: $("#imagegen-prompt"),
    imagegenGo: $("#imagegen-go"),
    imagegenCount: $("#imagegen-count"),
    imagegenIdeas: $("#imagegen-ideas"),
    imagegenState: $("#imagegen-state"),
    imagegenStage: $("#imagegen-stage"),
    resumeFilter: $("#resume-filter"),
    settings: $("#settings"),
    settingsIndex: $("#settings-index"),
    voiceSummary: $("#voice-summary"),
    hotkeyChoices: $("#hotkey-choices"),
    accessInput: $("#access-input-state"),
    accessAccessibility: $("#access-accessibility-state"),
    accentSwatches: $("#accent-swatches"),
    accentCustom: $("#accent-custom"),
    accentHex: $("#accent-hex"),
    accentReset: $("#accent-reset"),
    surfaceChoices: $("#surface-choices"),
    statusColors: $("#status-colors"),
    alertsToggle: $("#alerts-toggle"),
    pingChoices: $("#ping-choices"),
    afterTypingChoices: $("#after-typing-choices"),
    afterTypingHint: $("#after-typing-hint"),
    typedSoundToggle: $("#typed-sound-toggle"),
    builtinToggle: $("#builtin-toggle"),
    vocabForm: $("#vocab-form"),
    vocabWord: $("#vocab-word"),
    vocabHeard: $("#vocab-heard"),
    vocabError: $("#vocab-error"),
    vocabCount: $("#vocab-count"),
    vocabList: $("#vocab-list"),
    historyToggle: $("#history-toggle"),
    historyKeepChoices: $("#history-keep-choices"),
    historyClear: $("#history-clear"),
    historyList: $("#history-list"),
  };

  const groups = () => state.store.groups;
  const currentGroup = () => groups()[state.selected];
  // The group whose cards are on screen. Home and Settings show no cards, so they get no group.
  const visibleGroup = () => (state.view === "group" ? currentGroup() : undefined);
  const allSessions = () => groups().flatMap((g) => g.sessions);
  const findSession = (id) => allSessions().find((s) => s.id === id);

  const stored = {
    get(key) {
      try { return localStorage.getItem(key); } catch { return null; }
    },
    set(key, value) {
      try { localStorage.setItem(key, value); } catch { /* storage unavailable */ }
    },
  };

  // -------------------------------------------------------------- settings

  // Settings are kept in localStorage. Each read and write is guarded, so unavailable storage
  // falls back to the defaults instead of breaking the app.
  const SETTINGS_KEY = "settings";
  const HEX = /^#[0-9a-f]{6}$/i;
  // claudeLimit is tokens per five hours, 0 when not set. claudeLimitAt is when it was set (Unix ms).
  // vocabulary is a list of { word, heardAs }. historyKeep is how many dictated entries to keep.
  const DEFAULT_SETTINGS = {
    accent: "#7c9cff", surface: "default", done: "#4ac26b", fail: "#f06b6b", claude: "#d97757", hotkey: "alt", donePing: "Chime", claudeLimit: 0, claudeLimitAt: 0,
    vocabulary: [], builtinWords: true, afterTyping: "none", typedSound: true, historyOn: true, historyKeep: 200,
  };
  const SURFACES = {
    default: { label: "Default", canvas: "#121213", sidebar: "#171718", card: "#1c1c1e" },
    midnight: { label: "Midnight", canvas: "#0b0e18", sidebar: "#10141f", card: "#151a29" },
    graphite: { label: "Graphite", canvas: "#19191b", sidebar: "#1f1f22", card: "#26262a" },
  };
  const ACCENTS = [
    { label: "Blue", value: "#7c9cff" },
    { label: "Violet", value: "#b08cff" },
    { label: "Teal", value: "#4fd1c5" },
    { label: "Amber", value: "#f5b84b" },
    { label: "Rose", value: "#ff7a9c" },
    { label: "Lime", value: "#b5e061" },
  ];
  const STATUS_COLORS = [
    { key: "done", label: "Done" },
    { key: "fail", label: "Failed" },
    { key: "claude", label: "Claude logo" },
  ];
  // Voice modes, by the modifier keys that are held. Each one is a hold-to-talk chord.
  const HOTKEYS = {
    alt: { label: "Option", symbol: "⌥", keys: ["alt"], hint: "The default. Hold Option on its own." },
    ctrl: { label: "Control", symbol: "⌃", keys: ["ctrl"], hint: "Hold Control on its own." },
    "alt+meta": { label: "Option and Command", symbol: "⌥⌘", keys: ["alt", "meta"], hint: "Hold both keys together." },
    "ctrl+alt": { label: "Control and Option", symbol: "⌃⌥", keys: ["ctrl", "alt"], hint: "Hold both keys together." },
  };
  const MODIFIER_FLAG = { alt: "altKey", ctrl: "ctrlKey", meta: "metaKey", shift: "shiftKey" };
  // Sounds a finished agent can play. They are made by scripts/make-done-pings.mjs. The first one is the
  // default. The backend keeps the same list and refuses any other name.
  const DONE_PINGS = ["Chime", "Marimba", "Harp", "Droplet", "Glow", "Lift", "Knock", "Twinkle", "Pluck", "Echo", "Ripple"];
  // What happens after the words are typed. "return" presses Enter so the agent runs the prompt.
  const AFTER_TYPING = {
    none: { label: "Nothing", hint: "Leave the cursor after the words." },
    space: { label: "A space", hint: "Ready for the next thing you say." },
    return: { label: "Return", hint: "Press Enter, so the agent runs what you said." },
  };
  const HISTORY_KEEP = [50, 200, 1000];
  // The engine gets at most VOCABULARY_MAX words; the backend trims to the same limit.
  const VOCABULARY_MAX = 100;
  const VOCABULARY_WORD_MAX = 80;

  let settings = { ...DEFAULT_SETTINGS };

  function loadSettings() {
    let saved = {};
    try { saved = JSON.parse(stored.get(SETTINGS_KEY)) || {}; } catch { saved = {}; }
    const pick = (key, valid) => (valid(saved[key]) ? saved[key] : DEFAULT_SETTINGS[key]);
    const isColor = (value) => typeof value === "string" && HEX.test(value);
    const isBoolean = (value) => typeof value === "boolean";
    const isVocabulary = (list) => Array.isArray(list) && list.length <= VOCABULARY_MAX
      && list.every((entry) => entry && typeof entry.word === "string" && typeof entry.heardAs === "string");
    settings = {
      accent: pick("accent", isColor),
      surface: pick("surface", (value) => Object.keys(SURFACES).includes(value)),
      done: pick("done", isColor),
      fail: pick("fail", isColor),
      claude: pick("claude", isColor),
      hotkey: pick("hotkey", (value) => Object.keys(HOTKEYS).includes(value)),
      donePing: pick("donePing", (value) => DONE_PINGS.includes(value)),
      claudeLimit: pick("claudeLimit", (value) => Number.isFinite(value) && value >= 0),
      claudeLimitAt: pick("claudeLimitAt", (value) => Number.isFinite(value) && value >= 0),
      vocabulary: pick("vocabulary", isVocabulary),
      builtinWords: pick("builtinWords", isBoolean),
      afterTyping: pick("afterTyping", (value) => Object.keys(AFTER_TYPING).includes(value)),
      typedSound: pick("typedSound", isBoolean),
      historyOn: pick("historyOn", isBoolean),
      historyKeep: pick("historyKeep", (value) => HISTORY_KEEP.includes(value)),
    };
  }

  function saveSettings() {
    stored.set(SETTINGS_KEY, JSON.stringify(settings));
  }

  // ------------------------------------------------------------- vocabulary

  // Each entry is a word the engine should expect. "Often heard as" is optional: when the engine writes
  // that spelling instead, Nesa swaps in the word once the hold ends. Only the words go to the engine.
  const vocabularyWords = () => settings.vocabulary.map((entry) => entry.word);
  const escapeRegExp = (text) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

  // Whole words only, ignoring case, so "cooper" does not change "cooperate". Entries apply in list order.
  function applyVocabulary(text) {
    return settings.vocabulary.reduce((out, { word, heardAs }) => (heardAs
      ? out.replace(new RegExp(`(^|[^\\p{L}\\p{N}])${escapeRegExp(heardAs)}(?=$|[^\\p{L}\\p{N}])`, "giu"), (_, before) => before + word)
      : out), text);
  }

  function addVocabulary(event) {
    event.preventDefault();
    const word = el.vocabWord.value.trim();
    const heardAs = el.vocabHeard.value.trim();
    const taken = settings.vocabulary.some((entry) => entry.word.toLowerCase() === word.toLowerCase());
    const problem = !word ? "Type the word first."
      : word.length > VOCABULARY_WORD_MAX || heardAs.length > VOCABULARY_WORD_MAX ? `Keep each entry under ${VOCABULARY_WORD_MAX} characters.`
      : taken ? `"${word}" is already in your vocabulary.`
      : settings.vocabulary.length >= VOCABULARY_MAX ? `The list is full at ${VOCABULARY_MAX} words. Remove one to add another.`
      : "";
    el.vocabError.textContent = problem;
    el.vocabError.hidden = !problem;
    if (problem) return;
    // An "often heard as" that only differs by case is not a replacement, so it is dropped.
    const sameWord = heardAs.toLowerCase() === word.toLowerCase();
    setSetting("vocabulary", [...settings.vocabulary, { word, heardAs: sameWord ? "" : heardAs }]);
    el.vocabWord.value = "";
    el.vocabHeard.value = "";
    el.vocabWord.focus();
  }

  const removeVocabulary = (index) => setSetting("vocabulary", settings.vocabulary.filter((_, i) => i !== index));

  function renderVocabulary() {
    const list = settings.vocabulary;
    el.builtinToggle.setAttribute("aria-checked", String(settings.builtinWords));
    el.vocabCount.textContent = list.length ? `${list.length} of ${VOCABULARY_MAX} words.` : "No words yet.";
    el.vocabList.replaceChildren(...list.map((entry, index) => h("li", { class: "vocab-item" },
      h("span", { class: "vocab-word" }, entry.word),
      entry.heardAs ? h("small", {}, `often heard as "${entry.heardAs}"`) : null,
      h("button", {
        class: "icon-btn",
        type: "button",
        title: `Remove ${entry.word}`,
        "aria-label": `Remove ${entry.word}`,
        onclick: () => removeVocabulary(index),
        html: ICONS.close,
      }))));
  }

  // ---------------------------------------------------------------- history

  // Every dictation that produced words, newest first. It is kept on this Mac, in this app's local storage.
  const HISTORY_KEY = "dictationHistory";
  let dictationHistory = loadHistory();

  function loadHistory() {
    let saved = [];
    try { saved = JSON.parse(stored.get(HISTORY_KEY)) || []; } catch { saved = []; }
    if (!Array.isArray(saved)) return [];
    return saved.filter((item) => item && typeof item.text === "string" && Number.isFinite(item.at) && typeof item.into === "string");
  }

  function saveHistory() {
    stored.set(HISTORY_KEY, JSON.stringify(dictationHistory));
  }

  // `into` is the session the words were typed into, or "" when they were not typed.
  function recordHistory(text, into) {
    if (!settings.historyOn) return;
    dictationHistory = [{ text, at: Date.now(), into }, ...dictationHistory].slice(0, settings.historyKeep);
    saveHistory();
    if (state.view === "settings") renderHistory();
  }

  function setHistoryKeep(count) {
    dictationHistory = dictationHistory.slice(0, count);
    saveHistory();
    setSetting("historyKeep", count);
  }

  const deleteHistory = (index) => {
    dictationHistory = dictationHistory.filter((_, i) => i !== index);
    saveHistory();
    renderHistory();
  };

  async function clearHistory() {
    const count = dictationHistory.length;
    if (!count) return;
    const ok = await confirmDialog({
      title: "Clear the history?",
      text: `This removes the ${count === 1 ? "one saved entry" : `${count} saved entries`} from this Mac.`,
      label: "Clear",
    });
    if (!ok) return;
    dictationHistory = [];
    saveHistory();
    renderHistory();
  }

  // Copies through the backend, which runs pbcopy. The webview's clipboard can refuse a write that is not
  // from a direct click, so this is the reliable path.
  function copyHistory(item, button) {
    attempt("copy_text", { text: item.text }).then((result) => {
      if (!result.ok) return;
      button.textContent = "Copied";
      setTimeout(() => { button.textContent = "Copy"; }, 1400);
    });
  }

  function renderHistory() {
    el.historyToggle.setAttribute("aria-checked", String(settings.historyOn));
    el.historyClear.disabled = !dictationHistory.length;
    el.historyKeepChoices.replaceChildren(...HISTORY_KEEP.map((count) => choice(
      count === settings.historyKeep,
      () => setHistoryKeep(count),
      count.toLocaleString("en-US"),
    )));
    if (!dictationHistory.length) {
      el.historyList.replaceChildren(h("li", { class: "setting-hint history-empty" }, settings.historyOn
        ? "Nothing yet. What you say in voice mode shows up here."
        : "History is off, so new words are not saved."));
      return;
    }
    el.historyList.replaceChildren(...dictationHistory.map((item, index) => {
      const when = new Date(item.at);
      const where = item.into ? `Typed into ${item.into}` : "Not typed";
      return h("li", { class: "history-item" },
        h("div", { class: "history-meta" },
          h("time", { datetime: when.toISOString() }, when.toLocaleString([], { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" })),
          h("span", { title: where }, where)),
        h("p", { class: "history-text" }, item.text),
        h("div", { class: "history-actions" },
          h("button", { class: "btn btn-outline", type: "button", onclick: (e) => copyHistory(item, e.currentTarget) }, "Copy"),
          h("button", { class: "link", type: "button", onclick: () => deleteHistory(index) }, "Delete")));
    }));
  }

  const hexToRgb = (hex) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));

  // Dark text on light colours, light text on dark ones.
  function inkFor(hex) {
    const [r, g, b] = hexToRgb(hex);
    return 0.299 * r + 0.587 * g + 0.114 * b > 150 ? "#0f1424" : "#ffffff";
  }

  // Writes the chosen colours onto the CSS tokens. Open terminals take the new palette too.
  function applyColors() {
    const surface = SURFACES[settings.surface];
    const [r, g, b] = hexToRgb(settings.accent);
    const style = document.documentElement.style;
    style.setProperty("--accent", settings.accent);
    style.setProperty("--accent-ink", inkFor(settings.accent));
    style.setProperty("--selection", `rgba(${r}, ${g}, ${b}, 0.28)`);
    style.setProperty("--canvas", surface.canvas);
    style.setProperty("--sidebar", surface.sidebar);
    style.setProperty("--card", surface.card);
    style.setProperty("--done", settings.done);
    style.setProperty("--fail", settings.fail);
    style.setProperty("--claude", settings.claude);
    const theme = terminalOptions().theme;
    cards.forEach((card) => { card.term.options.theme = theme; });
  }

  // A colour picker fires many input events while it is open, so these do not re-render the page.
  function setColor(key, value) {
    settings[key] = value;
    saveSettings();
    applyColors();
  }

  function setSetting(key, value) {
    settings[key] = value;
    saveSettings();
    if (key === "hotkey") syncChord();
    applyColors();
    if (state.view === "settings") renderSettings();
    if (state.view === "home") renderHomeHint();
  }

  // --------------------------------------------------------------- helpers

  function h(tag, attrs = {}, ...kids) {
    const node = document.createElement(tag);
    for (const [key, value] of Object.entries(attrs)) {
      if (value == null || value === false) continue;
      if (key === "class") node.className = value;
      else if (key === "html") node.innerHTML = value; // static SVG strings only
      else if (key.startsWith("on")) node.addEventListener(key.slice(2), value);
      else node.setAttribute(key, value === true ? "" : value);
    }
    node.append(...kids.flat().filter((kid) => kid != null && kid !== false));
    return node;
  }

  const logo = (kind) => h("span", { class: `logo ${LOGO_CLASS[kind]}`, html: LOGOS[LOGO_KEY[kind]] });
  // A terminal or a browser has no agent logo, so it gets a glyph of its own.
  const kindMark = (kind) => (kind === "terminal" || kind === "browser"
    ? h("span", { class: "kind-mark", html: ICONS[kind === "terminal" ? "terminal" : "globe"] })
    : logo(kind));
  const icon = (name) => h("span", { class: "icon", html: ICONS[name] });
  const errorText = (err) => (typeof err === "string" ? err : (err && err.message) || String(err));

  // Restarts a one-shot CSS animation by re-adding its class (the reflow makes the browser notice).
  function replayClass(node, name) {
    node.classList.remove(name);
    void node.offsetWidth;
    node.classList.add(name);
  }

  // Errors stay longer, since they are the ones worth reading twice. A click dismisses a toast early.
  const TOAST_ICON = { error: "alert", ok: "check" };
  const TOAST_MS = { error: 6000 };
  const TOAST_DEFAULT_MS = 3500;

  function toast(message, kind = "error") {
    const mark = TOAST_ICON[kind] ? h("span", { class: "toast-icon", html: ICONS[TOAST_ICON[kind]] }) : null;
    const node = h("div", { class: `toast ${kind}`, role: "status" }, mark, h("span", { class: "toast-text" }, errorText(message)));
    el.toasts.append(node);
    let timer = null;
    const dismiss = () => {
      clearTimeout(timer);
      if (node.classList.contains("leaving")) return;
      node.classList.add("leaving");
      setTimeout(() => node.remove(), 200); // matches the toast-out animation
    };
    timer = setTimeout(dismiss, TOAST_MS[kind] || TOAST_DEFAULT_MS);
    node.addEventListener("click", dismiss);
  }

  // Runs a command; failures become a toast instead of an exception.
  async function attempt(name, args) {
    try {
      return { ok: true, value: await invoke(name, args) };
    } catch (err) {
      toast(err);
      return { ok: false };
    }
  }

  function shortPath(path) {
    const home = path.replace(/^\/Users\/[^/]+/, "~");
    const parts = home.split("/").filter(Boolean);
    const last = parts[parts.length - 1] || "/";
    if (home.startsWith("~")) return parts.length > 2 ? `~/…/${last}` : home;
    return parts.length > 2 ? `/…/${last}` : home;
  }

  const kindLabel = (kind) => (kind === "codex" ? "Codex" : "Claude");

  // 75s, 3m 05s, 2h 10m. Coarse on purpose: it is read at a glance.
  function formatElapsed(ms) {
    const total = Math.max(0, Math.floor(ms / 1000));
    const minutes = Math.floor(total / 60);
    const seconds = total % 60;
    if (minutes < 1) return `${seconds}s`;
    const hours = Math.floor(minutes / 60);
    if (hours < 1) return `${minutes}m ${String(seconds).padStart(2, "0")}s`;
    return `${hours}h ${String(minutes % 60).padStart(2, "0")}m`;
  }

  // ---------------------------------------------------------------- status

  function stateOf(id) {
    const st = state.status[id] || {};
    if (st.exited) return "exited";
    if (st.working) return "working";
    if (state.done.has(id)) return "done";
    return "idle";
  }

  const STATE_WORD = { working: "Working", done: "Done", exited: "Exited", idle: "Idle" };

  // Records when a "working" stretch began, so the card can show how long it has run.
  function trackWorking(id, working) {
    if (!working) state.workingSince.delete(id);
    else if (!state.workingSince.has(id)) state.workingSince.set(id, Date.now());
  }

  function isLive(id) {
    return !!(state.status[id] && state.status[id].running);
  }

  function setStatus(id, patch) {
    const prev = state.status[id] || { running: false, exited: false, working: false };
    state.status[id] = { ...prev, ...patch };
    trackWorking(id, !!state.status[id].working);
    updateCardStatus(id);
    updateIndicators();
  }

  function isOnScreen(id) {
    const group = visibleGroup();
    if (!group || !group.sessions.some((s) => s.id === id)) return false;
    return !state.maximizedId || state.maximizedId === id;
  }

  // A finished session is only "done" if nobody was looking at it.
  function clearSeenDone() {
    if (!document.hasFocus()) return;
    let changed = false;
    for (const id of [...state.done]) {
      if (isOnScreen(id)) { state.done.delete(id); changed = true; }
    }
    if (changed) { cards.forEach((_, id) => updateCardStatus(id)); updateIndicators(); }
  }

  function onActivity({ id, working }) {
    const wasWorking = !!(state.status[id] && state.status[id].working);
    if (working) state.done.delete(id);
    else if (wasWorking && !(document.hasFocus() && isOnScreen(id))) {
      state.done.add(id);
      flashRow(id);
      announceDone(id);
    }
    setStatus(id, { working, running: true });
  }

  // A finished session glows once in the sidebar, so it is easy to spot among the others.
  function flashRow(id) {
    const row = el.groupList.querySelector(`.session-row[data-id="${id}"]`);
    if (row) replayClass(row, "flash");
  }

  // A system notification, shown only while the app is in the background.
  function announceDone(id) {
    const session = findSession(id);
    if (!session || document.hasFocus() || !alertsOn()) return;
    invoke("notify", { title: "Nesa", body: `${session.name} finished` }).catch(() => {});
  }

  async function refreshStatus(ids) {
    if (!ids.length) return;
    const result = await attempt("session_status", { ids });
    if (!result.ok) return;
    Object.assign(state.status, result.value);
    ids.forEach((id) => trackWorking(id, !!(state.status[id] && state.status[id].working)));
    ids.forEach(updateCardStatus);
    updateIndicators();
  }

  // ------------------------------------------------------------- terminals

  const css = (name) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();

  function terminalOptions() {
    const card = css("--card");
    return {
      fontFamily: '"SF Mono", Menlo, monospace',
      fontSize: 12.5,
      lineHeight: 1.2,
      scrollback: 2000,
      cursorStyle: "bar",
      cursorInactiveStyle: "none",
      theme: {
        background: card,
        foreground: css("--text"),
        cursor: css("--accent"),
        cursorAccent: card,
        selectionBackground: css("--selection"),
        scrollbarSliderBackground: "rgba(139, 139, 148, 0.18)",
        scrollbarSliderHoverBackground: "rgba(139, 139, 148, 0.32)",
        scrollbarSliderActiveBackground: "rgba(139, 139, 148, 0.45)",
        black: "#3a3a40", red: "#f06b6b", green: "#4ac26b", yellow: "#e5c07b",
        blue: "#7c9cff", magenta: "#c792ea", cyan: "#56c8d8", white: "#c9c9cf",
        brightBlack: "#62626b", brightRed: "#ff8a8a", brightGreen: "#6fdc8c", brightYellow: "#f0d399",
        brightBlue: "#9db4ff", brightMagenta: "#d9aef2", brightCyan: "#7ddbe8", brightWhite: "#f2f2f4",
      },
    };
  }

  const decodeBase64 = (b64) => Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));

  // Returns false for keys xterm must not forward to the shell. Every app shortcut listed in
  // onKeyDown belongs here too, or the shell would receive it as well.
  function handleTermKey(id, e) {
    if (e.metaKey && (e.key === "n" || e.key === "[" || e.key === "]" || e.key === "j" || e.key === "Enter" || e.key === "," || /^[0-9]$/.test(e.key))) return false;
    if (e.key === "Enter" && e.shiftKey && !e.metaKey && !e.ctrlKey && !e.altKey) {
      if (e.type === "keydown") invoke("write_pty", { id, data: "\x1b\r" }).catch(() => {});
      return false; // ESC + CR makes Claude Code insert a newline
    }
    return true;
  }

  function fitCard(card, force = false) {
    if (!card.opened || !card.body.clientWidth || !card.body.clientHeight) return;
    const dims = card.fit.proposeDimensions();
    if (!dims || !(dims.cols > 0) || !(dims.rows > 0)) return;
    if (dims.cols !== card.term.cols || dims.rows !== card.term.rows) card.term.resize(dims.cols, dims.rows);
    if (!card.live) return; // the backend only learns sizes once the agent runs
    const last = card.sentSize;
    if (force || !last || last.cols !== dims.cols || last.rows !== dims.rows) {
      card.sentSize = { cols: dims.cols, rows: dims.rows };
      invoke("resize_pty", { id: card.id, cols: dims.cols, rows: dims.rows }).catch(() => {});
    }
  }

  // First display: attach xterm, size it, then start the agent.
  async function openCard(card) {
    // A browser has no terminal and no process. Its page starts once, when its card first shows, if it has an address.
    const session = findSession(card.id);
    if (session && session.kind === "browser") {
      if (!card.browserStarted) {
        card.browserStarted = true;
        if (session.url) navigateBrowser(card.id, session.url);
      }
      return;
    }
    card.opened = true;
    card.term.open(card.body);
    fitCard(card);
    const result = await attempt("ensure_session", { id: card.id });
    if (!result.ok) {
      setStatus(card.id, { running: false, exited: true, working: false });
      return;
    }
    card.live = true;
    fitCard(card, true);
  }

  function snapshotScroll(card) {
    if (!card.opened) return;
    const buf = card.term.buffer.active;
    card.scroll = { atBottom: buf.viewportY >= buf.baseY, line: buf.viewportY };
  }

  function restoreScroll(card) {
    if (!card.opened || !card.scroll) return;
    if (card.scroll.atBottom) card.term.scrollToBottom();
    else card.term.scrollToLine(card.scroll.line);
  }

  function writeToCard(id, bytes) {
    const card = cards.get(id) || (findSession(id) && createCard(findSession(id)));
    if (card) { card.term.write(bytes); return; }
    // The session is not in the store yet (just created): hold on to its output.
    const chunks = pendingData.get(id) || [];
    chunks.push(bytes);
    let total = chunks.reduce((n, c) => n + c.length, 0);
    while (total > PENDING_LIMIT && chunks.length > 1) total -= chunks.shift().length;
    pendingData.set(id, chunks);
  }

  // ----------------------------------------------------------------- cards

  function createCard(session) {
    const term = new Terminal(terminalOptions());
    const fit = new FitAddonClass();
    term.loadAddon(fit);
    term.onData((data) => invoke("write_pty", { id: session.id, data }).catch(() => {}));
    term.attachCustomKeyEventHandler((e) => handleTermKey(session.id, e));
    if (session.kind !== "browser") registerImageLinks(term, session);

    const ui = buildCardDom(session);
    const card = { id: session.id, term, fit, ...ui, opened: false, live: false, sentSize: null, scroll: null, timer: 0 };
    new ResizeObserver(() => {
      // A narrow card has no room for the elapsed time next to the token count, so it drops first.
      card.root.classList.toggle("narrow", card.root.clientWidth < NARROW_CARD_PX);
      clearTimeout(card.timer);
      card.timer = setTimeout(() => fitCard(card), 50);
    }).observe(card.body);
    if (session.kind !== "browser") bindImageMenu(term, session, card.body);
    cards.set(session.id, card);

    const queued = pendingData.get(session.id);
    if (queued) { queued.forEach((bytes) => term.write(bytes)); pendingData.delete(session.id); }
    return card;
  }

  function buildCardDom(session) {
    const dot = h("span", { class: "dot" });
    const name = h("span", { class: "card-name" });
    const meta = h("span", { class: "card-meta" });
    const browser = session.kind === "browser";
    const status = h("span", { class: "card-status", hidden: browser });
    const restart = h("button", { class: "btn", title: "Restart session", onclick: () => restartWithConfirm(session.id) }, icon("restart"), "Restart");
    const expand = h("button", { class: "icon-btn", onclick: () => toggleMaximize(session.id) });
    const more = h("button", { class: "icon-btn", title: "More", "aria-label": "More", onclick: (e) => openCardMenu(session.id, e.currentTarget) }, icon("more"));
    const head = h("header", { class: "card-head", ondblclick: (e) => { if (!e.target.closest("button")) toggleMaximize(session.id); } },
      dot, kindMark(session.kind), name, meta, status, h("span", { class: "spacer" }), restart, expand, more);
    // A browser's page is a native view that covers the body, so the body only shows before a page is open.
    const body = h("div", { class: "card-body" }, browser ? h("div", { class: "browser-empty" }, "Type an address above, or a search.") : null);
    const bar = browser ? browserBar(session.id) : null;
    // The entrance animation is one-shot: drop its class once it ends, so moving the card later does not replay it.
    const root = h("article", {
      class: "card",
      "data-id": session.id,
      onmousedown: () => focusCard(session.id),
      onanimationend: (e) => {
        if (e.target !== e.currentTarget) return;
        e.currentTarget.classList.remove("arrive");
        syncBrowsers();
      },
    }, head, bar, body);
    return { root, body, dot, name, meta, status, restart, expand, urlInput: bar ? bar.querySelector(".browser-url") : null };
  }

  // The address bar of a browser session. Its page is a native view placed under this bar.
  function browserBar(id) {
    const step = (name, label, command) => h("button", {
      class: "icon-btn", type: "button", title: label, "aria-label": label,
      onclick: () => attempt(command, { id }),
    }, icon(name));
    return h("div", { class: "browser-bar" },
      step("back", "Back", "browser_back"),
      step("forward", "Forward", "browser_forward"),
      step("restart", "Reload", "browser_reload"),
      h("input", {
        class: "input mono browser-url", type: "text", spellcheck: "false", placeholder: "Search or enter an address",
        "aria-label": "Address",
        onkeydown: (e) => { if (e.key === "Enter") { e.preventDefault(); navigateBrowser(id, e.currentTarget.value); } },
      }));
  }

  function hostOf(url) {
    try { return new URL(url).host; } catch { return ""; }
  }

  function updateCardInfo(card, session) {
    const maximized = state.maximizedId === session.id;
    card.name.textContent = session.name;
    if (session.kind === "browser") {
      card.meta.textContent = hostOf(session.url) || "no page yet";
      card.meta.title = session.url || "";
    } else {
      card.meta.textContent = shortPath(session.cwd);
      card.meta.title = session.cwd;
    }
    card.expand.replaceChildren(icon(maximized ? "collapse" : "expand"));
    card.expand.title = maximized ? "Restore (\u2318\u21A9)" : "Expand (\u2318\u21A9)";
    card.expand.setAttribute("aria-label", card.expand.title);
  }

  // Cards narrower than this drop the elapsed time from the status, so the token count still fits.
  const NARROW_CARD_PX = 380;

  // The status pill: "Working · 10s · 42k tok". Reads only cached values, so it is cheap to run every second.
  function renderStatus(card, id, st) {
    const since = state.workingSince.get(id);
    const tokens = state.tokens[id];
    // Each piece is a flex item, and flex items drop the spaces at their edges, so the separators are items too.
    const pieces = [h("span", {}, STATE_WORD[st])];
    if (st === "working" && since) {
      pieces.push(h("span", { class: "status-sep time" }, " · "), h("span", { class: "status-time" }, formatElapsed(Date.now() - since)));
    }
    if (tokens) {
      pieces.push(h("span", { class: "status-sep" }, " · "), h("span", { class: "status-tokens" }, formatTokens(tokens)), h("span", { class: "status-unit" }, " tok"));
    }
    card.status.replaceChildren(...pieces);
  }

  // Token counts for the cards on screen, read by the backend every few seconds. Only Claude and Codex
  // sessions have a transcript to count.
  const TOKEN_REFRESH_MS = 8000;
  let tokensBusy = false;

  async function refreshTokens() {
    if (tokensBusy || document.hidden) return;
    const requests = [...cards.values()]
      .filter((card) => card.root.parentNode === el.grid)
      .map((card) => findSession(card.id))
      .filter((s) => s && s.agent_id && (s.kind === "claude" || s.kind === "codex"))
      .map((s) => ({ id: s.id, kind: s.kind, agentId: s.agent_id }));
    if (!requests.length) return;
    tokensBusy = true;
    try {
      const counts = await invoke("session_tokens", { requests });
      for (const { id, tokens } of counts) state.tokens[id] = tokens;
      requests.forEach((request) => updateCardStatus(request.id));
    } catch {
      // Keep the last counts. The header shows the older number until the next read succeeds.
    } finally {
      tokensBusy = false;
    }
  }

  function updateCardStatus(id) {
    const card = cards.get(id);
    if (!card) return;
    const st = stateOf(id);
    card.dot.dataset.state = st;
    card.status.dataset.state = st;
    renderStatus(card, id, st);
    card.status.title = state.tokens[id] ? "Tokens this session has used: input and output, with cache reads left out." : "";
    card.root.classList.toggle("is-working", st === "working");
    card.restart.hidden = st !== "exited";
  }

  // Repaints every status indicator outside the cards: sidebar dots and the toolbar summary.
  function updateIndicators() {
    el.groupList.querySelectorAll("[data-session-dot]").forEach((dot) => {
      dot.dataset.state = stateOf(dot.dataset.sessionDot);
    });
    el.groupList.querySelectorAll(".group").forEach((row) => {
      const group = groups()[Number(row.dataset.index)];
      const states = (group ? group.sessions : []).map((s) => stateOf(s.id));
      const dot = row.querySelector("[data-group-dot]");
      if (states.includes("done")) dot.dataset.state = "done";
      else if (states.includes("working")) dot.dataset.state = "working";
      else delete dot.dataset.state;
    });
    updateSummary();
    if (state.view === "home") renderHomeStatus();
  }

  function updateSummary() {
    const group = visibleGroup();
    const states = (group ? group.sessions : []).map((s) => stateOf(s.id));
    const chip = (word) => {
      const n = states.filter((st) => st === word).length;
      return n ? h("span", { class: "stat" }, h("span", { class: "dot", "data-state": word }), `${n} ${word}`) : null;
    };
    // chip() returns null for an empty count; replaceChildren would print that as the text "null".
    el.summary.replaceChildren(...[chip("working"), chip("done")].filter(Boolean));
  }

  function focusCard(id, { focusTerminal = true } = {}) {
    state.focusedId = id;
    cards.forEach((card) => card.root.classList.toggle("focused", card.id === id));
    el.groupList.querySelectorAll(".session-row").forEach((row) => row.classList.toggle("focused", row.dataset.id === id));
    const card = cards.get(id);
    if (focusTerminal && card && card.opened) card.term.focus();
  }

  function toggleMaximize(id) {
    state.maximizedId = state.maximizedId === id ? null : id;
    state.focusedId = id;
    renderGrid();
    focusCard(id);
  }

  function disposeCard(card) {
    clearTimeout(card.timer);
    card.term.dispose();
    card.root.remove();
    cards.delete(card.id);
  }

  async function restartSession(id) {
    const result = await attempt("restart_session", { id });
    if (!result.ok) return;
    const card = cards.get(id);
    state.done.delete(id);
    setStatus(id, { running: true, exited: false, working: false });
    if (card) { card.live = true; fitCard(card, true); }
  }

  // A running agent loses its in-flight work on restart, so ask first. A stopped one has nothing to lose.
  async function restartWithConfirm(id) {
    const session = findSession(id);
    if (!session) return;
    if (isLive(id)) {
      const ok = await confirmDialog({
        title: `Restart "${session.name}"?`,
        text: "The running agent will be stopped and started again in the same folder.",
        label: "Restart",
        danger: false,
      });
      if (!ok) return;
    }
    restartSession(id);
  }

  async function stopSession(id) {
    const result = await attempt("stop_session", { id });
    if (result.ok) setStatus(id, { running: false, exited: true, working: false });
  }

  async function removeSession(id) {
    const session = findSession(id);
    if (!session) return;
    if (isLive(id)) {
      const ok = await confirmDialog({
        title: `Remove "${session.name}"?`,
        text: "Its running agent will be stopped and removed from this group. The transcript stays on disk.",
        label: "Remove",
      });
      if (!ok) return;
    }
    const result = await attempt("remove_session", { id });
    if (result.ok) applyStore(result.value);
  }

  async function renameSession(id) {
    const session = findSession(id);
    if (!session) return;
    const name = await promptDialog({ title: "Rename session", value: session.name, label: "Save" });
    if (!name || name === session.name) return;
    const result = await attempt("rename_session", { id, name });
    if (result.ok) applyStore(result.value);
  }

  async function moveSession(id, index) {
    const session = findSession(id);
    const target = groups()[index];
    if (!session || !target || target.sessions.some((s) => s.id === id)) return;
    const result = await attempt("move_session", { id, groupIndex: index });
    if (!result.ok) return;
    applyStore(result.value);
    toast(`Moved "${session.name}" to ${target.name}`, "ok");
  }

  async function revealFolder(id) {
    await attempt("reveal_session_folder", { id });
  }

  async function copyPath(id) {
    const session = findSession(id);
    if (!session) return;
    const result = await attempt("copy_text", { text: session.cwd });
    if (result.ok) toast("Path copied", "ok");
  }

  async function copySessionId(id) {
    const session = findSession(id);
    if (!session || !session.agent_id) return;
    const result = await attempt("copy_text", { text: session.agent_id });
    if (result.ok) toast("Session ID copied", "info");
  }

  // One list drives both the card's "more" menu and the sidebar's right-click menu.
  function sessionMenuItems(id) {
    const items = [
      { label: "Rename…", onSelect: () => renameSession(id) },
      { label: "Restart", onSelect: () => restartWithConfirm(id) },
      { label: "Stop", disabled: !isLive(id), onSelect: () => stopSession(id) },
    ];
    if (groups().length > 1) {
      const current = groups().findIndex((g) => g.sessions.some((s) => s.id === id));
      items.push({ sep: true }, { heading: "Move to" });
      groups().forEach((group, index) => items.push({
        label: group.name,
        disabled: index === current,
        onSelect: () => moveSession(id, index),
      }));
    }
    items.push(
      { sep: true },
      { label: "Reveal in Finder", onSelect: () => revealFolder(id) },
      { label: "Copy path", onSelect: () => copyPath(id) },
      { label: "Copy session ID", disabled: !findSession(id)?.agent_id, onSelect: () => copySessionId(id) },
      { sep: true },
      { label: "Remove from group", danger: true, onSelect: () => removeSession(id) },
    );
    return items;
  }

  function openCardMenu(id, anchor) {
    const rect = anchor.getBoundingClientRect();
    showMenu(rect.right, rect.bottom + 4, sessionMenuItems(id), { alignRight: true });
  }

  function openSessionMenu(id, x, y) {
    showMenu(x, y, sessionMenuItems(id));
  }

  // Selects the session's group if needed, then brings its card to the front.
  // Opens the group that holds session `id` and focuses its card. Also leaves Home and Settings.
  function focusSession(id) {
    const index = groups().findIndex((g) => g.sessions.some((s) => s.id === id));
    if (index < 0) return;
    if (index !== state.selected) {
      state.selected = index;
      state.maximizedId = null;
      state.renaming = null;
      state.arriving = true;
      stored.set("selectedGroup", String(index));
    } else if (state.maximizedId) {
      state.maximizedId = id;
    }
    if (state.view !== "group") state.arriving = true; // coming from Home or Settings
    state.view = "group";
    state.focusedId = id;
    renderAll();
    focusCard(id);
  }

  // Next or previous session in the current group, wrapping at the ends.
  function cycleFocus(step) {
    const group = currentGroup();
    if (!group || !group.sessions.length) return;
    const ids = group.sessions.map((s) => s.id);
    const at = ids.indexOf(state.focusedId);
    const from = at < 0 ? (step > 0 ? -1 : ids.length) : at;
    focusSession(ids[(from + step + ids.length) % ids.length]);
  }

  // Cmd+J: the next finished session across all groups, switching group if needed.
  function jumpToFinished() {
    const all = allSessions();
    const at = all.findIndex((s) => s.id === state.focusedId);
    for (let step = 1; step <= all.length; step += 1) {
      const session = all[(at + step + all.length) % all.length];
      if (stateOf(session.id) === "done") {
        focusSession(session.id);
        return;
      }
    }
    toast("No finished sessions right now", "info");
  }

  // ---------------------------------------------------------------- render

  // The view comes first: the grid is measured by its layout, so it must be shown before it renders.
  function renderAll() {
    renderView();
    renderSidebar();
    renderToolbar();
    renderGrid();
  }

  function renderView() {
    const view = state.view;
    el.toolbar.hidden = view !== "group";
    // The voice pill's idle dot floats in the toolbar, so it only shows where there is a toolbar.
    document.body.classList.toggle("view-group", view === "group");
    el.stage.hidden = view !== "group";
    el.home.hidden = view !== "home";
    el.settings.hidden = view !== "settings";
    document.querySelectorAll("[data-view]").forEach((node) => {
      const active = node.dataset.view === view;
      node.classList.toggle("active", active);
      if (active) node.setAttribute("aria-current", "page");
      else node.removeAttribute("aria-current");
    });
    if (view === "home") renderHome();
    if (view === "settings") renderSettings();
  }

  function renderSidebar() {
    const input = el.groupList.querySelector(".group-rename");
    if (input) input.dataset.dead = "1"; // the blur from replacing it must not commit

    el.groupList.replaceChildren(...groups().map((group, index) => {
      const renaming = state.renaming === index;
      const name = renaming
        ? h("input", {
            class: "group-rename",
            value: state.renameDraft,
            spellcheck: "false",
            "aria-label": "Group name",
            oninput: (e) => { state.renameDraft = e.target.value; },
            onkeydown: (e) => onRenameKey(e, index),
            onblur: (e) => { if (!e.target.dataset.dead) commitRename(index); },
            onclick: (e) => e.stopPropagation(),
            ondblclick: (e) => e.stopPropagation(),
          })
        : h("span", { class: "group-name" }, group.name);
      const header = h("div", {
        class: `group-row${index === state.selected && state.view === "group" ? " selected" : ""}`,
        role: "button",
        draggable: renaming ? null : "true", // a group being renamed is not dragged
        ondragstart: (e) => onGroupDragStart(e, index),
        ondragend: () => endDrag(),
        onclick: () => selectGroup(index),
        ondblclick: () => startRename(index),
        oncontextmenu: (e) => { e.preventDefault(); openGroupMenu(e, index); },
      }, name,
        group.sessions.length ? h("span", { class: "group-count" }, String(group.sessions.length)) : null,
        h("span", { class: "dot", "data-group-dot": "" }));
      // The wrapper is the drop target, so the session list under the header takes drops too.
      return h("div", {
        class: "group",
        "data-index": index,
        ondragover: (e) => onGroupDragOver(e, index),
        ondragleave: (e) => onGroupDragLeave(e),
        ondrop: (e) => onGroupDrop(e, index),
      }, header, group.sessions.length ? h("div", { class: "session-list" }, group.sessions.map((session, i) => sessionRow(session, index, i))) : null);
    }));

    const field = el.groupList.querySelector(".group-rename");
    if (field) {
      field.focus();
      if (state.renameFresh) { field.select(); state.renameFresh = false; }
      else field.setSelectionRange(field.value.length, field.value.length);
    }
    updateIndicators();
  }

  function sessionRow(session, groupIndex, sessionIndex) {
    const id = session.id;
    return h("div", {
      class: `session-row${state.focusedId === id ? " focused" : ""}`,
      role: "button",
      title: session.cwd,
      "data-id": id,
      draggable: "true",
      onclick: () => focusSession(id),
      ondblclick: () => renameSession(id),
      oncontextmenu: (e) => { e.preventDefault(); openSessionMenu(id, e.clientX, e.clientY); },
      ondragstart: (e) => onSessionDragStart(e, id),
      ondragend: () => endDrag(),
      ondragover: (e) => onRowDragOver(e),
      ondrop: (e) => onRowDrop(e, groupIndex, sessionIndex),
      ondragleave: (e) => { if (!e.currentTarget.contains(e.relatedTarget)) e.currentTarget.classList.remove("drop-before", "drop-after"); },
    },
      kindMark(session.kind),
      h("span", { class: "session-name" }, session.name),
      h("span", { class: "dot", "data-session-dot": id }));
  }

  // Drag a session row onto another group to move it there. The id lives in state because
  // drag events cannot read custom data during dragover.
  function onSessionDragStart(e, id) {
    state.dragId = id;
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", id); // WebKit will not start a drag without some data
    e.currentTarget.classList.add("dragging");
  }

  function endDrag() {
    state.dragId = null;
    state.dragGroup = null;
    el.groupList.querySelectorAll(".dragging, .drop-target, .drop-before, .drop-after").forEach((node) =>
      node.classList.remove("dragging", "drop-target", "drop-before", "drop-after"));
  }

  // Dragging a group header reorders the groups. The line shows above or below the header.
  function onGroupDragStart(e, index) {
    state.dragGroup = index;
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", groups()[index].name); // WebKit will not start a drag without some data
    e.currentTarget.closest(".group").classList.add("dragging");
  }

  // Which side of a node the pointer is on: above or below its midpoint. The drop line goes there.
  function dropSide(node, clientY) {
    const rect = node.getBoundingClientRect();
    return clientY < rect.top + rect.height / 2 ? "before" : "after";
  }

  // Shows the drop line on one node and clears it everywhere else.
  function markDrop(node, side) {
    el.groupList.querySelectorAll(".drop-before, .drop-after").forEach((other) => {
      if (other !== node) other.classList.remove("drop-before", "drop-after");
    });
    node.classList.toggle("drop-before", side === "before");
    node.classList.toggle("drop-after", side === "after");
  }

  function onGroupDragOver(e, index) {
    if (state.dragGroup !== null) {
      if (state.dragGroup === index) return; // a group cannot land on itself
      e.preventDefault();
      e.dataTransfer.dropEffect = "move";
      const header = e.currentTarget.querySelector(":scope > .group-row");
      markDrop(header, dropSide(header, e.clientY));
      return;
    }
    const id = state.dragId;
    if (!id || groups()[index].sessions.some((s) => s.id === id)) return; // not a drop onto its own group
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
    e.currentTarget.classList.add("drop-target");
  }

  function onGroupDragLeave(e) {
    if (!e.currentTarget.contains(e.relatedTarget)) e.currentTarget.classList.remove("drop-target");
  }

  function onGroupDrop(e, index) {
    e.preventDefault();
    if (state.dragGroup !== null) {
      const header = e.currentTarget.querySelector(":scope > .group-row");
      const from = state.dragGroup;
      const target = header.classList.contains("drop-after") ? index + 1 : index;
      endDrag();
      moveGroup(from, target);
      return;
    }
    const id = state.dragId;
    endDrag();
    if (id) moveSession(id, index);
  }

  // A session row takes drops of sessions and places the dragged one above or below it. Group drags pass
  // through to the group, so their drop line lands on the group header.
  function onRowDragOver(e) {
    if (state.dragId === null) return;
    e.preventDefault();
    e.stopPropagation();
    e.dataTransfer.dropEffect = "move";
    markDrop(e.currentTarget, dropSide(e.currentTarget, e.clientY));
  }

  function onRowDrop(e, groupIndex, sessionIndex) {
    if (state.dragId === null) return;
    e.preventDefault();
    e.stopPropagation();
    const after = e.currentTarget.classList.contains("drop-after");
    const id = state.dragId;
    endDrag();
    placeSession(id, groupIndex, sessionIndex + (after ? 1 : 0));
  }

  // Puts a session before the session at `index` in group `group`. A drop that changes nothing sends nothing.
  async function placeSession(id, group, index) {
    const from = groups().findIndex((g) => g.sessions.some((s) => s.id === id));
    if (from < 0) return;
    const position = groups()[from].sessions.findIndex((s) => s.id === id);
    if (from === group && (index === position || index === position + 1)) return;
    const result = await attempt("place_session", { id, group, index });
    if (result.ok) applyStore(result.value);
  }

  // Moves group `from` before the group at `index`. The selected group keeps being the same group, and the
  // stored selection follows it.
  async function moveGroup(from, index) {
    if (from === index || from + 1 === index) return;
    const result = await attempt("move_group", { from, index });
    if (!result.ok) return;
    const to = index > from ? index - 1 : index;
    state.selected = shiftedIndex(state.selected, from, to);
    state.renaming = null;
    stored.set("selectedGroup", String(state.selected));
    applyStore(result.value);
  }

  // Where group `i` ends up once group `from` has moved to `to`.
  function shiftedIndex(i, from, to) {
    if (i === from) return to;
    if (from < to && i > from && i <= to) return i - 1;
    if (to < from && i >= to && i < from) return i + 1;
    return i;
  }

  function renderToolbar() {
    const group = currentGroup();
    const n = group ? group.sessions.length : 0;
    el.title.textContent = group ? group.name : "";
    // Only a group switch slides the title in. Renames and new sessions re-render it quietly.
    if (el.title.dataset.index !== String(state.selected)) {
      el.title.dataset.index = String(state.selected);
      replayClass(el.title, "swap");
    }
    el.count.textContent = n ? `${n} ${n === 1 ? "session" : "sessions"}` : "";
    updateSummary();
  }

  function gridColumns(n) {
    return n <= 1 ? 1 : n <= 4 ? 2 : n <= 9 ? 3 : 4;
  }

  function renderGrid() {
    const group = visibleGroup();
    const sessions = group ? group.sessions : [];
    const maximized = sessions.find((s) => s.id === state.maximizedId);
    if (!maximized) state.maximizedId = null;
    const shown = maximized ? [maximized] : sessions;
    const shownIds = new Set(shown.map((s) => s.id));

    el.empty.hidden = sessions.length > 0;
    el.grid.hidden = sessions.length === 0;
    el.emptyTitle.textContent = group ? `No sessions in ${group.name}` : "No sessions";

    // Park cards that leave the screen; their terminals (and scrollback) live on.
    cards.forEach((card) => {
      if (!shownIds.has(card.id) && card.root.parentNode === el.grid) {
        // Parking hides the card, which cancels its entrance without firing animationend. Clear the class here.
        card.root.classList.remove("arrive");
        snapshotScroll(card);
        el.parking.append(card.root);
      }
    });

    const cols = gridColumns(shown.length);
    el.grid.style.setProperty("--rows", Math.ceil(shown.length / cols));
    const tail = shown.length % cols;
    const arriving = state.arriving;
    state.arriving = false;
    const entries = shown.map((session, i) => {
      const fresh = !cards.has(session.id);
      const card = cards.get(session.id) || createCard(session);
      updateCardInfo(card, session);
      updateCardStatus(session.id);
      // A new card, or every card after a group switch, arrives one after another.
      if (fresh || arriving) {
        card.root.style.setProperty("--i", i);
        card.root.classList.add("arrive");
      }
      // Twelve columns let a short last row stretch evenly across the width.
      const inTail = tail && i >= shown.length - tail;
      card.root.style.gridColumn = `span ${12 / (inTail ? tail : cols)}`;
      return card;
    });

    const placed = [...el.grid.children];
    const needsMove = placed.length !== entries.length || entries.some((card, i) => placed[i] !== card.root);
    if (needsMove) {
      entries.forEach(snapshotScroll);
      el.grid.replaceChildren(...entries.map((card) => card.root));
    }

    if (!shownIds.has(state.focusedId)) state.focusedId = shown.length ? shown[0].id : null;
    entries.forEach((card) => {
      if (!card.opened) openCard(card);
      else if (needsMove) { fitCard(card); restoreScroll(card); }
    });
    focusCard(state.focusedId, { focusTerminal: false });
    clearSeenDone();
    syncBrowsers();
  }

  // ---------------------------------------------------------------- groups

  // Shows group `index`. From Home or Settings the same group still has to open, so only the
  // group view itself is an early return.
  function selectGroup(index) {
    if (!groups()[index]) return;
    const changed = index !== state.selected;
    if (!changed && state.view === "group") return;
    // A group switch, or coming back from Home or Settings, slides the cards in one after another.
    if (changed || state.view !== "group") state.arriving = true;
    state.view = "group";
    if (changed) {
      state.selected = index;
      state.maximizedId = null;
      state.focusedId = null;
      state.renaming = null;
      stored.set("selectedGroup", String(index));
    }
    renderAll();
    focusCard(state.focusedId);
  }

  function startRename(index) {
    state.renaming = index;
    state.renameDraft = groups()[index].name;
    state.renameFresh = true;
    renderSidebar();
  }

  function onRenameKey(e, index) {
    if (e.key === "Enter") { e.preventDefault(); commitRename(index); }
    else if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); cancelRename(); }
  }

  function cancelRename() {
    state.renaming = null;
    renderSidebar();
  }

  async function commitRename(index) {
    if (state.renaming !== index) return;
    const name = state.renameDraft.trim();
    const unchanged = !name || name === groups()[index].name;
    state.renaming = null;
    if (unchanged) { renderSidebar(); return; }
    const result = await attempt("rename_group", { index, name });
    if (result.ok) applyStore(result.value);
    else renderSidebar();
  }

  async function createGroup() {
    const taken = new Set(groups().map((g) => g.name));
    let n = groups().length + 1;
    while (taken.has(`Group ${n}`)) n += 1;
    const result = await attempt("create_group", { name: `Group ${n}` });
    if (!result.ok) return;
    state.selected = result.value.groups.length - 1;
    state.view = "group";
    state.maximizedId = null;
    state.focusedId = null;
    state.renaming = state.selected;
    state.renameDraft = result.value.groups[state.selected].name;
    state.renameFresh = true;
    applyStore(result.value);
  }

  async function deleteGroup(index) {
    const group = groups()[index];
    const count = group.sessions.length;
    const detail = count
      ? `Its ${count} ${count === 1 ? "session" : "sessions"} will be stopped and removed from Nesa.`
      : "This group is empty.";
    const ok = await confirmDialog({ title: `Delete "${group.name}"?`, text: detail, label: "Delete" });
    if (!ok) return;
    const result = await attempt("delete_group", { index });
    if (!result.ok) return;
    if (index < state.selected) state.selected -= 1;
    if (index === state.selected) { state.maximizedId = null; state.focusedId = null; }
    applyStore(result.value);
  }

  function openGroupMenu(e, index) {
    const onlyOne = groups().length <= 1;
    showMenu(e.clientX, e.clientY, [
      { label: "Rename", onSelect: () => startRename(index) },
      { label: "Delete", danger: true, disabled: onlyOne, onSelect: () => deleteGroup(index) },
    ]);
  }

  // Store is the source of truth: every render starts here.
  function applyStore(store) {
    state.store = store;
    const ids = new Set(allSessions().map((s) => s.id));
    cards.forEach((card) => {
      if (ids.has(card.id)) return;
      disposeCard(card);
      state.done.delete(card.id);
      state.workingSince.delete(card.id);
      delete state.status[card.id];
    });
    if (state.selected >= store.groups.length) state.selected = Math.max(0, store.groups.length - 1);
    if (state.renaming != null && !store.groups[state.renaming]) state.renaming = null;
    if (!ids.has(state.focusedId)) state.focusedId = null;
    if (!ids.has(state.maximizedId)) state.maximizedId = null;
    renderAll();
  }

  // ------------------------------------------------------------ menu/dialog

  let closeActiveMenu = null;
  let activeMenu = null;

  function closeMenu() {
    if (closeActiveMenu) closeActiveMenu();
  }

  // Items are buttons, or { sep: true } for a divider, or { heading } for a non-clickable label.
  function showMenu(x, y, items, { alignRight = false } = {}) {
    closeMenu();
    const menu = h("div", { class: "menu", role: "menu" }, items.map((item) => {
      if (item.sep) return h("div", { class: "menu-sep", role: "separator" });
      if (item.heading) return h("div", { class: "menu-heading" }, item.heading);
      return h("button", {
        class: `menu-item${item.danger ? " danger" : ""}`,
        role: "menuitem",
        disabled: item.disabled,
        onclick: () => { closeMenu(); item.onSelect(); },
      }, item.label);
    }));
    document.body.append(menu);
    activeMenu = menu;

    const { offsetWidth: w, offsetHeight: ht } = menu;
    const left = Math.max(8, Math.min(alignRight ? x - w : x, window.innerWidth - w - 8));
    const top = Math.max(8, Math.min(y, window.innerHeight - ht - 8));
    menu.style.left = `${left}px`;
    menu.style.top = `${top}px`;
    menu.style.transformOrigin = alignRight ? "top right" : "top left"; // the menu grows from its anchor

    const onPointerDown = (e) => { if (!menu.contains(e.target)) closeMenu(); };
    document.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("blur", closeMenu);
    closeActiveMenu = () => {
      document.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("blur", closeMenu);
      menu.remove();
      activeMenu = null;
      closeActiveMenu = null;
      syncBrowsers();
    };
    syncBrowsers();
  }

  function moveMenuFocus(step) {
    if (!activeMenu) return;
    const items = [...activeMenu.querySelectorAll(".menu-item:not(:disabled)")];
    if (!items.length) return;
    const at = items.indexOf(document.activeElement);
    const next = at < 0 ? (step > 0 ? 0 : items.length - 1) : (at + step + items.length) % items.length;
    items[next].focus();
  }

  function confirmDialog({ title, text, label, danger = true }) {
    const dialog = el.confirmDialog;
    $("#confirm-title").textContent = title;
    $("#confirm-text").textContent = text;
    const ok = $("#confirm-ok");
    ok.textContent = label;
    ok.className = `btn ${danger ? "btn-danger" : "btn-primary"}`;
    return new Promise((resolve) => {
      let answer = false;
      const finish = () => { dialog.removeEventListener("close", finish); resolve(answer); };
      dialog.addEventListener("close", finish);
      $("#confirm-form").onsubmit = (e) => { e.preventDefault(); answer = true; dialog.close(); };
      $("#confirm-cancel").onclick = () => dialog.close();
      dialog.showModal();
      syncBrowsers();
    });
  }

  // Resolves with the trimmed text, or null if cancelled. Blank text keeps the dialog open.
  function promptDialog({ title, value, label }) {
    const dialog = el.promptDialog;
    const input = $("#prompt-input");
    $("#prompt-title").textContent = title;
    $("#prompt-ok").textContent = label;
    input.value = value;
    return new Promise((resolve) => {
      let answer = null;
      const finish = () => { dialog.removeEventListener("close", finish); resolve(answer); };
      dialog.addEventListener("close", finish);
      $("#prompt-form").onsubmit = (e) => {
        e.preventDefault();
        const text = input.value.trim();
        if (!text) { input.focus(); return; }
        answer = text;
        dialog.close();
      };
      $("#prompt-cancel").onclick = () => dialog.close();
      dialog.showModal();
      syncBrowsers();
      input.focus();
      input.select();
    });
  }

  const anyDialogOpen = () => [el.sessionDialog, el.confirmDialog, el.promptDialog].some((d) => d.open);

  // Kinds of the next session. The dialog's own segmented control, not the Home filter that shares the class.
  const NAME_HINT = {
    claude: "Defaults to the agent name",
    codex: "Defaults to the agent name",
    terminal: "Defaults to the folder name",
    browser: "Defaults to the site",
  };
  const MAX_BATCH = 8; // matches the backend: more than this in one request is refused

  function setKind(kind) {
    state.newKind = kind;
    document.querySelectorAll("#new-session .segmented button").forEach((btn) =>
      btn.setAttribute("aria-checked", String(btn.dataset.kind === kind)));
    const browser = kind === "browser";
    $("#session-folder-field").hidden = browser;
    $("#session-url-field").hidden = !browser;
    $("#session-name").placeholder = NAME_HINT[kind];
  }

  // How many sessions of the chosen kind to open together.
  function setCount(n) {
    state.newCount = Math.min(MAX_BATCH, Math.max(1, n));
    $("#session-count").textContent = String(state.newCount);
    $("#count-less").disabled = state.newCount === 1;
    $("#count-more").disabled = state.newCount === MAX_BATCH;
    $("#session-start").textContent = state.newCount > 1 ? `Start ${state.newCount}` : "Start";
  }

  function openSessionDialog(kind = state.newKind) {
    if (!currentGroup() || anyDialogOpen()) return;
    closeMenu();
    setKind(kind);
    setCount(1);
    $("#session-name").value = "";
    $("#session-cwd").value = stored.get("lastCwd") || "";
    $("#session-url").value = "";
    $("#session-error").hidden = true;
    $("#session-start").disabled = false;
    renderRecent();
    el.sessionDialog.showModal();
    syncBrowsers();
    if (kind === "browser") $("#session-url").focus();
    else ($("#session-cwd").value ? $("#session-name") : $("#session-cwd")).focus();
  }

  // Folders the user has started sessions in, most recent first.
  function renderRecent() {
    const paths = [];
    const candidates = [stored.get("lastCwd"), ...[...allSessions()].reverse().map((s) => s.cwd)];
    for (const path of candidates) {
      if (path && !paths.includes(path) && paths.length < 5) paths.push(path);
    }
    el.recent.replaceChildren(
      ...(paths.length ? [h("span", { class: "recent-label" }, "Recent")] : []),
      ...paths.map((path) => h("button", {
        class: "chip",
        type: "button",
        title: path,
        onclick: () => { $("#session-cwd").value = path; },
      }, shortPath(path))));
    el.recent.hidden = !paths.length;
  }

  async function browseFolder() {
    if (!pickFolder) { toast("Folder picker is unavailable"); return; }
    const field = $("#session-cwd");
    try {
      const path = await pickFolder({
        directory: true,
        multiple: false,
        defaultPath: field.value.trim() || stored.get("lastCwd") || undefined,
      });
      if (typeof path === "string") field.value = path;
    } catch (err) {
      showSessionError(errorText(err));
    }
  }

  function showSessionError(message) {
    const node = $("#session-error");
    node.textContent = message;
    node.hidden = false;
  }

  async function submitSession(e) {
    e.preventDefault();
    const kind = state.newKind;
    const browser = kind === "browser";
    const cwd = $("#session-cwd").value.trim();
    if (!browser && !cwd) { showSessionError("Choose a folder for the session."); return; }
    const start = $("#session-start");
    start.disabled = true;
    $("#session-error").hidden = true;
    try {
      const created = await invoke("create_sessions", {
        groupIndex: state.selected,
        kind,
        count: state.newCount,
        name: $("#session-name").value.trim(),
        cwd: browser ? "" : cwd, // a browser has no folder, so a folder left in the hidden field is not sent
        url: browser ? $("#session-url").value.trim() || null : null,
      });
      if (!browser) stored.set("lastCwd", created[0].cwd);
      if (!browser) created.forEach((s) => { state.status[s.id] = { running: true, exited: false, working: false }; });
      state.focusedId = created[0].id;
      state.maximizedId = null;
      state.view = "group";
      el.sessionDialog.close();
      applyStore(await invoke("load_store"));
      focusCard(created[0].id);
    } catch (err) {
      showSessionError(errorText(err));
      start.disabled = false;
      // Sessions started before the failure are saved already, so they are shown too.
      const loaded = await attempt("load_store");
      if (loaded.ok) applyStore(loaded.value);
    }
  }

  // ---------------------------------------------------------------- views

  // Home, Settings, or a group. Only one is on screen, and cards outside the group view stay parked.
  function showView(view) {
    if (state.view === view && view !== "home") return;
    state.view = view;
    renderAll();
    if (view === "home") {
      loadTranscripts();
      refreshImageGen();
    }
  }

  const plural = (n, word) => `${n} ${word}${n === 1 ? "" : "s"}`;
  const folderName = (path) => path.split("/").filter(Boolean).pop() || path;

  function timeAgo(ms) {
    if (!ms) return "";
    const seconds = Math.max(0, (Date.now() - ms) / 1000);
    if (seconds < 60) return "just now";
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
    if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`;
    if (seconds < 86400 * 14) return `${Math.floor(seconds / 86400)}d ago`;
    return new Date(ms).toLocaleDateString();
  }

  // ----------------------------------------------------------------- home

  function renderHome() {
    renderHomeStatus();
    renderHomeHint();
    renderImageGen();
    renderResumeGroups();
    renderResumeList();
  }

  function renderHomeStatus() {
    const attention = [];
    const all = allSessions();
    all.forEach((session) => {
      const st = stateOf(session.id);
      if (st === "done" || st === "working") attention.push({ session, st });
    });
    const working = attention.filter((item) => item.st === "working").length;
    const done = attention.filter((item) => item.st === "done").length;
    const parts = [`${plural(all.length, "session")} across ${plural(groups().length, "group")}`];
    if (working) parts.push(`${working} working`);
    if (done) parts.push(`${done} finished`);
    el.homeSub.textContent = parts.join(" · ");

    // "Needs you" holds only what waits for the user. Sessions still working are listed apart.
    const finished = attention.filter((item) => item.st === "done");
    const running = attention.filter((item) => item.st === "working");
    el.liveBlock.hidden = finished.length === 0;
    el.liveList.replaceChildren(...finished.map((item) => liveRow(item.session, item.st)));
    el.workingBlock.hidden = running.length === 0;
    el.workingList.replaceChildren(...running.map((item) => liveRow(item.session, item.st)));
    el.groupCards.replaceChildren(
      ...groups().map((group, index) => groupCard(group, index)),
      h("button", { class: "group-card group-card-new", type: "button", onclick: createGroup }, icon("plus"), h("span", {}, "New group")));
  }

  function renderHomeHint() {
    el.homeHint.textContent = `Hold ${hotkey().symbol} to talk to a running session. The voice-mode keys are set in Settings.`;
  }

  // "Working · 3m 10s · 42k tok" for a working session, just "Done" for a finished one.
  function liveStateText(id, st) {
    const parts = [STATE_WORD[st]];
    const since = state.workingSince.get(id);
    if (st === "working" && since) parts.push(formatElapsed(Date.now() - since));
    if (st === "working" && state.tokens[id]) parts.push(`${formatTokens(state.tokens[id])} tok`);
    return parts.join(" · ");
  }

  // Ticks the time on the working rows. Only their text changes, so Home is not rebuilt every second.
  function tickHomeWorking() {
    if (state.view !== "home") return;
    document.querySelectorAll("#home [data-live-working]").forEach((node) => {
      node.textContent = liveStateText(node.dataset.liveWorking, "working");
    });
  }

  function liveRow(session, st) {
    const index = groups().findIndex((g) => g.sessions.some((s) => s.id === session.id));
    const text = h("span", {}, liveStateText(session.id, st));
    if (st === "working") text.dataset.liveWorking = session.id;
    return h("button", { class: "row row-button", type: "button", onclick: () => focusSession(session.id) },
      kindMark(session.kind),
      h("span", { class: "row-main" },
        h("span", { class: "row-title" }, session.name),
        h("span", { class: "row-meta" }, `${groups()[index].name} · ${shortPath(session.cwd)}`)),
      h("span", { class: "row-state" }, h("span", { class: "dot", "data-state": st }), text));
  }

  // How many session marks a group card shows before it says "+N".
  const GROUP_CARD_MARKS = 6;

  function groupCard(group, index) {
    const count = (word) => group.sessions.filter((s) => stateOf(s.id) === word).length;
    const empty = group.sessions.length === 0;
    const parts = [plural(group.sessions.length, "session")];
    if (count("working")) parts.push(`${count("working")} working`);
    if (count("done")) parts.push(`${count("done")} finished`);
    // A dot on a mark only for the states worth a glance. Idle sessions stay plain.
    const mark = (session) => {
      const st = stateOf(session.id);
      return h("span", { class: "gc-mark", title: `${session.name} · ${STATE_WORD[st]}` },
        kindMark(session.kind),
        st === "idle" ? null : h("span", { class: "dot", "data-state": st }));
    };
    const marks = group.sessions.slice(0, GROUP_CARD_MARKS).map(mark);
    const more = group.sessions.length - marks.length;
    return h("button", { class: `group-card${index === state.selected ? " current" : ""}`, type: "button", onclick: () => selectGroup(index) },
      h("span", { class: "group-card-name" }, group.name),
      h("span", { class: "group-card-meta" }, empty ? "Empty" : parts.join(" · ")),
      empty ? null : h("span", { class: "gc-marks" }, ...marks, more > 0 ? h("span", { class: "gc-more" }, `+${more}`) : null));
  }

  // -------------------------------------------------------------- image gen

  // Pictures made by Codex's image tool. The block shows only when this Codex can make images. Results live in
  // state for this run, so a re-render keeps them, and a restart starts empty.
  const IMAGEGEN_IDEAS = [
    "A red fox in fresh snow, flat illustration",
    "App icon: a paper plane on indigo, minimal",
    "Isometric server room with soft evening light",
    "Watercolor map of a harbour town",
  ];
  const IMAGEGEN_LIMIT = 2000; // characters, as the backend allows

  // Asks Codex whether it can make images. Home asks again each time it shows, because the answer can change.
  async function refreshImageGen() {
    state.imageGen.available = (await invoke("imagegen_available").catch(() => false)) === true;
    renderImageGen();
  }

  // Draws the block from state only. Nothing here reads the composer's text, so a re-render keeps a prompt
  // that is being typed.
  function renderImageGen() {
    const g = state.imageGen;
    el.imagegenBlock.hidden = !g.available;
    el.imagegenGo.textContent = g.busy ? "Making…" : "Make pictures";
    renderImageGenComposer();
    renderImageGenPill();
    renderImageGenStage();
  }

  // The composer's own state: the count near the limit, the ideas until something is written, and the button.
  function renderImageGenComposer() {
    const length = el.imagegenPrompt.value.length;
    el.imagegenCount.textContent = `${length} / ${IMAGEGEN_LIMIT}`;
    el.imagegenCount.hidden = length < IMAGEGEN_LIMIT * 0.8;
    el.imagegenIdeas.hidden = length > 0;
    el.imagegenGo.disabled = state.imageGen.busy || !el.imagegenPrompt.value.trim();
    // The prompt grows with its text up to the CSS max-height. The 2px is the border, which scrollHeight leaves out.
    el.imagegenPrompt.style.height = "auto";
    el.imagegenPrompt.style.height = `${el.imagegenPrompt.scrollHeight + 2}px`;
  }

  // The status in the header. While a picture is being made, it counts the time.
  function renderImageGenPill() {
    const g = state.imageGen;
    const mode = g.busy ? "busy" : g.error ? "error" : g.images.length ? "done" : "idle";
    const text = {
      busy: `Generating · ${formatElapsed(Date.now() - g.startedAt)}`,
      error: "Didn't work",
      done: plural(g.images.length, "picture"),
      idle: "Ready",
    }[mode];
    el.imagegenState.dataset.mode = mode;
    // While a request runs, a rotating dither mark (2 by 2 dots) stands in for the idle dot.
    const mark = mode === "busy"
      ? h("span", { class: "imagegen-spinner", "aria-hidden": "true" }, h("i"), h("i"), h("i"), h("i"))
      : null;
    el.imagegenState.replaceChildren(...[mark, text].filter(Boolean));
  }

  // One of four stages: making, failed or stopped, showing results, or waiting for a first request.
  function renderImageGenStage() {
    const g = state.imageGen;
    // The empty stage is a short prompt, not a large canvas. Results keep the full height.
    el.imagegenStage.classList.toggle("is-empty", !g.busy && !g.error && !g.stopped && !g.images.length);
    if (g.busy) el.imagegenStage.replaceChildren(imagegenBusy());
    else if (g.error) el.imagegenStage.replaceChildren(imagegenMessage("error", g));
    else if (g.stopped) el.imagegenStage.replaceChildren(imagegenMessage("stopped", g));
    else if (g.images.length) el.imagegenStage.replaceChildren(imagegenResults());
    else el.imagegenStage.replaceChildren(imagegenEmpty());
  }

  // Once a second while a request runs: the header clock and the clock on the stage.
  function tickImageGen() {
    const g = state.imageGen;
    renderImageGenPill();
    const clock = el.imagegenStage.querySelector("[data-elapsed]");
    if (clock) clock.textContent = `Generating image · ${formatElapsed(Date.now() - g.startedAt)}`;
  }

  function imagegenEmpty() {
    return h("div", { class: "imagegen-empty" },
      h("span", { class: "imagegen-empty-mark", html: ICONS.sparkle }),
      h("p", { class: "imagegen-empty-title" }, "Your pictures appear here"),
      h("p", { class: "imagegen-empty-text" }, "Copy one, then press ⌘V in a session to attach it to your message."));
  }

  // While Codex draws: a dither field that stands in for the picture, and a clock with a Stop.
  function imagegenBusy() {
    const g = state.imageGen;
    return h("div", { class: "imagegen-busy" },
      ditherField(),
      h("div", { class: "imagegen-busy-line" },
        h("span", { class: "imagegen-busy-text", "data-elapsed": "", "aria-live": "polite" }, `Generating image · ${formatElapsed(Date.now() - g.startedAt)}`),
        h("button", { class: "btn btn-outline", type: "button", onclick: () => invoke("imagegen_cancel").catch(() => {}) },
          icon("stop"), "Stop")));
  }

  // A field of dots on a canvas, in the spirit of beui's dither loader: the dots drift on their own, and the pointer
  // pushes them aside. It runs only while it is on the page, so a re-render that replaces it also stops it. With
  // reduced motion it is drawn once and left still.
  const DITHER_GAP = 10;
  function ditherField() {
    const canvas = h("canvas", { class: "imagegen-dither-canvas" });
    const field = h("div", { class: "imagegen-dither", "aria-hidden": "true" }, canvas);
    const context = canvas.getContext("2d");
    const reduce = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches === true;
    const pointer = { x: 0, y: 0, tx: 0, ty: 0, inside: false };
    let width = 0;
    let height = 0;
    let color = "";
    let painted = false;

    // Keeps the canvas as sharp as the screen, and the same size as the field. False until the field has a size.
    const fit = () => {
      const rect = canvas.getBoundingClientRect();
      if (!rect.width || !rect.height) return false;
      if (rect.width !== width || rect.height !== height) {
        const dpr = Math.min(window.devicePixelRatio || 1, 2);
        width = rect.width;
        height = rect.height;
        canvas.width = Math.round(width * dpr);
        canvas.height = Math.round(height * dpr);
        context.setTransform(dpr, 0, 0, dpr, 0, 0);
        color = getComputedStyle(canvas).color;
        pointer.x = pointer.tx = width / 2;
        pointer.y = pointer.ty = height / 2;
      }
      return true;
    };

    const paint = (time) => {
      if (!pointer.inside) {
        pointer.tx = width / 2 + (reduce ? 0 : Math.sin(time / 1700) * width * 0.12);
        pointer.ty = height / 2 + (reduce ? 0 : Math.cos(time / 2100) * height * 0.1);
      }
      const follow = reduce ? 1 : pointer.inside ? 0.16 : 0.045;
      pointer.x += (pointer.tx - pointer.x) * follow;
      pointer.y += (pointer.ty - pointer.y) * follow;

      const radius = Math.min(width, height) * 0.38;
      const columns = Math.ceil(width / DITHER_GAP) + 1;
      const rows = Math.ceil(height / DITHER_GAP) + 1;
      const offsetX = (width - (columns - 1) * DITHER_GAP) / 2;
      const offsetY = (height - (rows - 1) * DITHER_GAP) / 2;

      context.clearRect(0, 0, width, height);
      context.fillStyle = color;
      for (let row = 0; row < rows; row += 1) {
        for (let column = 0; column < columns; column += 1) {
          const anchorX = offsetX + column * DITHER_GAP;
          const anchorY = offsetY + row * DITHER_GAP;
          const dx = anchorX - pointer.x;
          const dy = anchorY - pointer.y;
          const distance = Math.hypot(dx, dy);
          const proximity = Math.max(0, 1 - distance / radius);
          const influence = proximity * proximity * (3 - 2 * proximity);
          const push = influence * influence * 9;
          const x = anchorX + (distance > 0 ? (dx / distance) * push : 0);
          const y = anchorY + (distance > 0 ? (dy / distance) * push : 0);
          context.globalAlpha = 0.17 + influence * 0.72;
          context.beginPath();
          context.arc(x, y, 0.65 + influence * 0.85, 0, Math.PI * 2);
          context.fill();
        }
      }
      context.globalAlpha = 1;
      painted = true;
    };

    // One loop per field. It ends when the field leaves the page, which is how a re-render retires it.
    const tick = (time) => {
      if (!canvas.isConnected) return;
      if (fit()) paint(time);
      if (!reduce || !painted) requestAnimationFrame(tick);
    };

    field.addEventListener("pointermove", (e) => {
      const rect = canvas.getBoundingClientRect();
      pointer.inside = true;
      pointer.tx = e.clientX - rect.left;
      pointer.ty = e.clientY - rect.top;
    });
    field.addEventListener("pointerleave", () => { pointer.inside = false; });
    requestAnimationFrame(tick);
    return field;
  }

  // A request that failed or was stopped. Both offer the same description again.
  function imagegenMessage(kind, g) {
    const failed = kind === "error";
    return h("div", { class: `imagegen-message${failed ? " is-error" : ""}`, role: failed ? "alert" : "status" },
      h("span", { class: "imagegen-message-mark", html: failed ? ICONS.alert : ICONS.stop }),
      h("div", { class: "imagegen-message-body" },
        h("p", { class: "imagegen-message-title" }, failed ? "No picture this time" : "Stopped before the pictures were done"),
        failed ? h("p", { class: "imagegen-message-text" }, g.error) : null),
      h("div", { class: "imagegen-message-actions" },
        h("button", { class: "btn btn-outline", type: "button", onclick: () => { el.imagegenPrompt.value = g.prompt; renderImageGenComposer(); el.imagegenPrompt.focus(); } }, "Edit prompt"),
        h("button", { class: "btn btn-primary", type: "button", onclick: () => generateImage(g.prompt) }, icon("restart"), "Try again")));
  }

  // The picture on show, its actions, and a strip of every picture the request made. Each picture is one step
  // in the strip, and the arrow keys move between them.
  function imagegenResults() {
    const g = state.imageGen;
    const current = g.images[g.selected];
    const shown = g.previews.get(current);
    const caption = h("span", { class: "imagegen-caption" }, `Picture ${g.selected + 1} of ${g.images.length}`);
    const chip = h("span", { class: "imagegen-resolution" });
    // A picture comes out of its blur once, the first time it is shown. Choosing it again shows it as it is.
    const reveal = shown && !g.revealed.has(current);
    if (reveal) g.revealed.add(current);
    const picture = shown
      ? h("img", {
          class: `imagegen-picture${reveal ? " is-reveal" : ""}`,
          src: shown,
          alt: folderName(current),
          onload: (e) => {
            chip.textContent = `${e.currentTarget.naturalWidth} × ${e.currentTarget.naturalHeight}`;
          },
        })
      : h("span", { class: "imagegen-loading" }, "Loading…");
    return h("div", { class: "imagegen-results" },
      h("div", { class: "imagegen-viewer" }, picture, chip),
      h("p", { class: "imagegen-prompt-echo", title: g.prompt }, `“${g.prompt}”`),
      h("div", { class: "imagegen-toolbar" },
        h("button", { class: "btn btn-primary", type: "button", disabled: !shown, onclick: copyImage }, icon("copy"), "Copy image"),
        h("button", { class: "btn btn-outline", type: "button", onclick: copyImagePath }, "Copy path"),
        h("button", { class: "btn btn-outline", type: "button", onclick: revealImage }, icon("folder"), "Show in Finder"),
        h("button", { class: "btn btn-outline", type: "button", title: "Make again with the same description", onclick: () => generateImage(g.prompt) }, icon("restart"), "Again")),
      h("div", { class: "imagegen-strip-row" },
        h("div", { class: "imagegen-strip", role: "radiogroup", "aria-label": "Pictures", onkeydown: onImageStripKey },
          g.images.map((path, i) => h("button", {
          class: "imagegen-thumb",
          type: "button",
          role: "radio",
          "aria-checked": String(i === g.selected),
          "aria-label": `Picture ${i + 1}`,
          onclick: () => selectImage(i),
        }, g.previews.has(path) ? h("img", { src: g.previews.get(path), alt: "" }) : h("span", { class: "imagegen-skeleton" })))),
        caption),
      h("p", { class: "imagegen-hint" }, "Copy image, then press ⌘V in a session. Claude and Codex read it as an attachment."));
  }

  function selectImage(index) {
    state.imageGen.selected = index;
    renderImageGenStage();
  }

  // Arrow keys move through the strip, and focus follows the picture that is chosen.
  function onImageStripKey(e) {
    const g = state.imageGen;
    const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[e.key];
    if (!step) return;
    e.preventDefault();
    selectImage((g.selected + step + g.images.length) % g.images.length);
    el.imagegenStage.querySelector('.imagegen-thumb[aria-checked="true"]')?.focus();
  }

  // Starts a request for `prompt`, or for what the composer holds. The pictures, or the reason there are none,
  // replace the stage. A request already running is left alone.
  async function generateImage(prompt) {
    const g = state.imageGen;
    if (g.busy) return;
    const text = (prompt ?? el.imagegenPrompt.value).trim();
    if (!text) {
      el.imagegenPrompt.focus();
      return;
    }
    // The composer keeps whatever the user has written. Again and Try again use the description they were made from.
    Object.assign(g, { busy: true, error: "", stopped: false, prompt: text, startedAt: Date.now(), images: [], selected: 0 });
    g.timer = setInterval(tickImageGen, 1000);
    renderImageGen();
    let outcome;
    try {
      outcome = await invoke("imagegen_generate", { prompt: text });
    } catch (err) {
      outcome = { error: errorText(err) };
    }
    clearInterval(g.timer);
    g.busy = false;
    if (outcome.error) {
      g.error = outcome.error;
    } else if (outcome.stopped) {
      g.stopped = true;
    } else {
      g.images = outcome.images;
      await Promise.all(g.images.map(loadImagePreview));
    }
    renderImageGen();
  }

  // Each picture is read once through the backend, then kept for the rest of the run. The stage is drawn once,
  // after they are all read, so the picture's reveal is not cut short by a redraw.
  async function loadImagePreview(path) {
    const g = state.imageGen;
    if (g.previews.has(path)) return;
    const result = await attempt("read_image", { path });
    if (result.ok) g.previews.set(path, result.value);
  }

  // Enter makes the pictures, and Shift+Enter starts a new line. Command+Enter is left alone: the window handler uses it.
  function onImageGenKey(e) {
    if (e.key !== "Enter" || e.shiftKey || e.isComposing) return;
    e.preventDefault();
    if (!state.imageGen.busy) el.imagegenForm.dispatchEvent(new Event("submit", { cancelable: true, bubbles: true }));
  }

  function selectedImage() {
    const g = state.imageGen;
    return g.images[g.selected] || null;
  }

  // Briefly relabels a button with a check, so the result shows where the user clicked.
  function flashButton(button, text) {
    if (!button) return;
    if (!button.restore) button.restore = [...button.childNodes];
    button.replaceChildren(icon("check"), document.createTextNode(text));
    clearTimeout(button.flashTimer);
    button.flashTimer = setTimeout(() => {
      button.replaceChildren(...button.restore);
      button.restore = null;
    }, 1600);
  }

  // Copy image puts the picture on the clipboard as an image, so a paste into a session attaches it.
  async function copyImage(e) {
    const path = selectedImage();
    if (!path) return;
    const result = await attempt("imagegen_copy", { path });
    if (result.ok) flashButton(e.currentTarget, "Copied");
  }

  async function copyImagePath(e) {
    const path = selectedImage();
    if (!path) return;
    const result = await attempt("copy_text", { text: path });
    if (result.ok) flashButton(e.currentTarget, "Path copied");
  }

  function revealImage() {
    const path = selectedImage();
    if (path) attempt("imagegen_reveal", { path });
  }

  function bindImageGen() {
    el.imagegenIdeas.replaceChildren(...IMAGEGEN_IDEAS.map((text) => h("button", {
      class: "imagegen-idea",
      type: "button",
      onclick: () => {
        el.imagegenPrompt.value = text;
        renderImageGenComposer();
        el.imagegenPrompt.focus();
      },
    }, text)));
    el.imagegenForm.addEventListener("submit", (e) => {
      e.preventDefault();
      generateImage();
    });
    el.imagegenPrompt.addEventListener("keydown", onImageGenKey);
    el.imagegenPrompt.addEventListener("input", renderImageGenComposer);
  }

  // ------------------------------------------------------------- resume

  // Past sessions come from the agents' own transcripts, so they include ones started outside the deck.
  async function loadTranscripts() {
    const result = await attempt("list_transcripts");
    if (result.ok) state.transcripts = result.value;
    else if (!state.transcripts) state.transcripts = [];
    if (state.view === "home") renderResumeList();
  }

  function renderResumeGroups() {
    if (state.resumeGroup >= groups().length) state.resumeGroup = state.selected;
    el.resumeGroup.replaceChildren(...groups().map((group, index) => h("option", { value: String(index) }, group.name)));
    el.resumeGroup.value = String(state.resumeGroup);
    el.resumeFilter.querySelectorAll("button").forEach((btn) =>
      btn.setAttribute("aria-checked", String(btn.dataset.filter === state.resumeFilter)));
  }

  // The list starts short, because a real user has hundreds of transcripts. Each "Show more" adds a page.
  const RESUME_FIRST = 8;
  const RESUME_PAGE = 25;

  // Back to the short list. Called when the filter or the search changes.
  function resetResumeShown() {
    state.resumeShown = RESUME_FIRST;
  }

  function renderResumeList({ focusFrom = -1 } = {}) {
    if (!state.transcripts) {
      el.resumeCount.textContent = "";
      el.resumeList.replaceChildren(h("p", { class: "home-empty" }, "Looking for past sessions…"));
      return;
    }
    const query = state.resumeQuery.trim().toLowerCase();
    const shown = state.transcripts.filter((item) =>
      (state.resumeFilter === "all" || item.kind === state.resumeFilter) &&
      (!query || `${item.title} ${item.cwd}`.toLowerCase().includes(query)));
    const total = state.transcripts.length;
    el.resumeCount.textContent = !total ? "" : shown.length === total ? `\u00B7 ${total}` : `\u00B7 ${shown.length} of ${total}`;
    if (!shown.length) {
      el.resumeList.replaceChildren(h("p", { class: "home-empty" }, total
        ? "No sessions match that search."
        : "No past Claude or Codex sessions were found."));
      return;
    }
    const visible = shown.slice(0, state.resumeShown);
    const rows = visible.map((item) => resumeRow(item));
    const rest = shown.length - visible.length;
    if (rest > 0) {
      rows.push(h("button", {
        class: "row-more", type: "button",
        onclick: () => {
          const from = visible.length;
          state.resumeShown += RESUME_PAGE;
          renderResumeList({ focusFrom: from });
        },
      }, `Show ${Math.min(rest, RESUME_PAGE)} more`, h("span", { class: "row-more-rest" }, `${rest} left`)));
    }
    el.resumeList.replaceChildren(...rows);
    // The button that was clicked is gone, so a keyboard user would lose their place. Land on the first new row.
    if (focusFrom >= 0 && rows[focusFrom]) {
      const target = rows[focusFrom].querySelector("button:not(:disabled)");
      if (target) target.focus({ preventScroll: true });
    }
  }

  function resumeRow(item) {
    const name = item.title || folderName(item.cwd);
    let action;
    if (item.deck_session) {
      action = h("button", { class: "btn btn-outline", type: "button", onclick: () => focusSession(item.deck_session) }, "Open");
    } else if (item.running) {
      action = h("button", { class: "btn btn-outline", type: "button", disabled: true, title: "It is open in another terminal. Close it there first." }, "In use");
    } else {
      action = h("button", { class: "btn btn-outline", type: "button", onclick: (e) => resumeTranscript(item, e.currentTarget) }, "Resume");
    }
    // The whole row does what the button does. The button stays for the keyboard and as the visible cue.
    const row = h("div", { class: `row${action.disabled ? "" : " row-click"}`, title: item.cwd },
      logo(item.kind),
      h("span", { class: "row-main" },
        h("span", { class: "row-title" }, name),
        h("span", { class: "row-meta" }, `${shortPath(item.cwd)} · ${timeAgo(item.updated)}`)),
      action);
    if (!action.disabled) {
      row.addEventListener("click", (e) => {
        if (!e.target.closest("button")) action.click();
      });
    }
    return row;
  }

  // Starts the past session in the chosen group, then opens it. The backend returns an existing
  // session instead when the deck already holds this transcript.
  async function resumeTranscript(item, button) {
    button.disabled = true;
    button.textContent = "Resuming…";
    const started = await attempt("resume_session", { groupIndex: state.resumeGroup, kind: item.kind, agentId: item.id });
    if (!started.ok) {
      button.disabled = false;
      button.textContent = "Resume";
      return;
    }
    const loaded = await attempt("load_store");
    if (!loaded.ok) return;
    applyStore(loaded.value);
    await refreshStatus([started.value.id]);
    focusSession(started.value.id);
    loadTranscripts();
  }

  // --------------------------------------------------------------- settings

  function renderSettings() {
    el.hotkeyChoices.replaceChildren(...Object.keys(HOTKEYS).map((key) => {
      const mode = HOTKEYS[key];
      const card = choice(key === settings.hotkey, () => setSetting("hotkey", key),
        h("span", { class: "choice-symbol" }, mode.symbol),
        h("span", { class: "choice-text" }, h("strong", {}, mode.label), h("small", {}, mode.hint)));
      card.classList.add("choice-card");
      return card;
    }));

    el.accentSwatches.replaceChildren(...ACCENTS.map((accent) => h("button", {
      class: "swatch",
      type: "button",
      title: accent.label,
      "aria-label": `${accent.label} accent`,
      "aria-pressed": String(settings.accent === accent.value),
      style: `--swatch: ${accent.value}`,
      onclick: () => setSetting("accent", accent.value),
    })));
    el.accentCustom.value = settings.accent;
    el.accentHex.textContent = settings.accent.toUpperCase();
    el.accentReset.disabled = settings.accent === DEFAULT_SETTINGS.accent;

    el.surfaceChoices.replaceChildren(...Object.keys(SURFACES).map((key) => {
      const surface = SURFACES[key];
      const card = choice(key === settings.surface, () => setSetting("surface", key),
        h("span", { class: "surface-preview", style: `--p-canvas: ${surface.canvas}; --p-sidebar: ${surface.sidebar}; --p-card: ${surface.card}` }),
        h("span", { class: "surface-name" }, surface.label));
      card.classList.add("surface-card");
      return card;
    }));

    el.statusColors.replaceChildren(...STATUS_COLORS.map(({ key, label }) => colorRow(key, label)));
    el.alertsToggle.setAttribute("aria-checked", String(alertsOn()));
    el.pingChoices.replaceChildren(...DONE_PINGS.map((name) => h("span", { class: "ping-option" },
      choice(name === settings.donePing, () => chooseDonePing(name), name),
      h("button", { class: "ping-preview", type: "button", title: `Preview ${name}`, "aria-label": `Preview ${name}`, onclick: () => previewDonePing(name) }))));
    el.afterTypingChoices.replaceChildren(...Object.keys(AFTER_TYPING).map((key) =>
      choice(key === settings.afterTyping, () => setSetting("afterTyping", key), AFTER_TYPING[key].label)));
    el.afterTypingHint.textContent = AFTER_TYPING[settings.afterTyping].hint;
    renderVoiceSummary();
    el.typedSoundToggle.setAttribute("aria-checked", String(settings.typedSound));
    renderVocabulary();
    renderHistory();
    renderClaudeLimit();
    renderClaudeStatus();
    syncSettingsIndex();
  }

  // The voice setup in plain words, so the choices below can be checked at a glance.
  const AFTER_SUMMARY = {
    none: "Nothing is added after them.",
    space: "A space is added after them.",
    return: "Return is pressed after them, so the agent runs what you said.",
  };

  function renderVoiceSummary() {
    const mode = HOTKEYS[settings.hotkey];
    const sound = settings.typedSound ? "A sound plays once they are typed." : "No sound plays.";
    el.voiceSummary.replaceChildren(
      h("span", { class: "voice-keys", "aria-hidden": "true" }, mode.symbol),
      h("div", { class: "voice-summary-text" },
        h("p", { class: "voice-summary-title" }, `Hold ${mode.label} to talk.`),
        h("p", { class: "voice-summary-body" },
          `Let go and the words are typed into the app you are using, or the focused session in Nesa. ${AFTER_SUMMARY[settings.afterTyping]} ${sound}`)));
  }

  // The jump index at the top of Settings. It marks the section nearest the top as the page scrolls, and a
  // click scrolls to one. A click holds the mark while the scroll runs, so a section too short to reach the
  // top (the last ones) still gets the mark it was given.
  const INDEX_HOLD_MS = 800;
  let indexHoldUntil = 0;

  function markSettingsSection(id) {
    el.settingsIndex.querySelectorAll("button").forEach((btn) => {
      if (btn.dataset.target === id) btn.setAttribute("aria-current", "true");
      else btn.removeAttribute("aria-current");
    });
  }

  function syncSettingsIndex() {
    if (performance.now() < indexHoldUntil || el.settings.hidden) return;
    const pageTop = el.settings.getBoundingClientRect().top;
    const line = el.settings.clientHeight / 3; // a section counts as reached once it is in the top third
    const targets = [...el.settingsIndex.querySelectorAll("button")].map((btn) => btn.dataset.target);
    const atTop = el.settings.scrollTop <= 0; // at the top, or a page too short to scroll, Voice is the first section
    const atEnd = el.settings.scrollTop + el.settings.clientHeight >= el.settings.scrollHeight - 2;
    let current = targets[0];
    for (const id of targets) {
      if (document.getElementById(id).getBoundingClientRect().top - pageTop <= line) current = id;
    }
    if (atTop) current = targets[0];
    markSettingsSection(atEnd && !atTop ? targets[targets.length - 1] : current);
  }

  function jumpToSection(id, smooth) {
    const section = document.getElementById(id);
    if (!section) return;
    markSettingsSection(id);
    indexHoldUntil = performance.now() + INDEX_HOLD_MS;
    const calm = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    section.scrollIntoView({ behavior: smooth && !calm ? "smooth" : "auto", block: "start" });
  }

  // A radio-style option. The checked one is outlined.
  function choice(checked, onSelect, ...content) {
    return h("button", { class: "choice", type: "button", role: "radio", "aria-checked": String(checked), onclick: onSelect }, ...content);
  }

  // Dragging in the picker fires input events, so those only apply the color. The page re-renders
  // once the picker closes (change), which keeps the open picker alive while the user drags.
  function colorRow(key, label) {
    const value = settings[key];
    const fallback = DEFAULT_SETTINGS[key];
    // Dragging the picker updates this text directly. A full re-render would close the open picker.
    const hex = h("span", { class: "color-hex" }, value.toUpperCase());
    return h("div", { class: "setting-row" },
      h("span", { class: "setting-label" }, label),
      h("div", { class: "setting-control" },
        hex,
        h("input", {
          class: "color-input",
          type: "color",
          value,
          "aria-label": `${label} color`,
          oninput: (e) => { hex.textContent = e.target.value.toUpperCase(); setColor(key, e.target.value); },
          onchange: (e) => setSetting(key, e.target.value),
        }),
        h("button", { class: "link", type: "button", disabled: value === fallback, onclick: () => setSetting(key, fallback) }, "Reset")));
  }

  function resetColors() {
    for (const key of ["accent", "surface", "done", "fail", "claude"]) settings[key] = DEFAULT_SETTINGS[key];
    saveSettings();
    applyColors();
    renderSettings();
  }

  // Choosing a done ping saves it and plays it once, so the user hears the choice.
  function chooseDonePing(name) {
    setSetting("donePing", name);
    syncDonePing();
    previewDonePing(name);
  }

  // The arrow beside each ping plays it without choosing it.
  const previewDonePing = (name) => invoke("preview_done_ping", { name }).catch(() => {});

  // The backend plays the ping when an agent finishes, so it gets the saved choice on launch and after each change.
  const syncDonePing = () => invoke("set_done_ping", { name: settings.donePing }).catch(() => {});

  // ------------------------------------------------------------------- usage

  // What Claude and Codex have used on this Mac. Codex's limits are its own numbers, read from its logs.
  // Claude Code keeps its plan limit off disk, so Claude use is an estimate against a limit set in Settings.
  const USAGE_REFRESH_MS = 30 * 1000;
  const HOUR_MS = 60 * 60 * 1000;
  const IDLE_TOKENS_PER_HOUR = 10000; // below this, Claude counts as idle rather than close to its limit
  let usageSnapshot = null;
  let usageBusy = false;
  let claudeLimits = null; // Claude Code's own limit windows, once its status line is connected

  function formatTokens(n) {
    if (n >= 1e6) return `${(n / 1e6).toFixed(n >= 1e8 ? 0 : 1)}M`;
    if (n >= 1e3) return `${Math.round(n / 1e3)}k`;
    return String(Math.round(n));
  }

  // 40m, 2h 05m, 3d 4h
  function formatSpan(ms) {
    const minutes = Math.max(0, Math.round(ms / 60000));
    if (minutes < 60) return `${minutes}m`;
    const hours = Math.floor(minutes / 60);
    if (hours < 48) return `${hours}h ${String(minutes % 60).padStart(2, "0")}m`;
    return `${Math.floor(hours / 24)}d ${hours % 24}h`;
  }

  // Claude: tokens in the last five hours against the limit the user set. The share is null without a limit.
  function claudeUsage(claude) {
    const detail = `Estimated from Claude Code transcripts: ${formatTokens(claude.tokens5h)} tokens in the last five hours, from ${claude.messages5h} messages. Input, cache writes and output count. Cache reads (${formatTokens(claude.cacheRead5h)}) do not. The five hours roll forward from now, but Claude's own limit resets in blocks, so this is only an estimate.`;
    const limit = settings.claudeLimit;
    if (!limit) return { share: null, text: `${formatTokens(claude.tokens5h)} in 5h · set limit`, title: detail };
    const share = claude.tokens5h / limit;
    const remaining = Math.max(0, limit - claude.tokens5h);
    let text = `${Math.round(share * 100)}%`;
    let pace = "";
    if (remaining === 0) {
      text += " · limit reached";
    } else if (claude.tokensLastHour < IDLE_TOKENS_PER_HOUR) {
      text += " · idle";
    } else {
      const left = formatSpan((remaining / claude.tokensLastHour) * HOUR_MS);
      text += ` · ${left} left`;
      pace = ` At the last hour's pace, about ${left} remain before the limit.`;
    }
    return { share, text, title: `${detail} Against a limit of about ${formatTokens(limit)}.${pace}` };
  }

  // One Codex window, from the newest report Codex wrote. The pace is the average since the window opened.
  function codexUsage(win, now) {
    const used = win.usedPercent;
    const untilReset = win.resetsAtMs - now;
    const asOf = `Codex reported ${formatAgo(now - win.observedAtMs)}. The window resets ${formatWhen(win.resetsAtMs)}.`;
    if (used <= 0) return { share: 0, text: `unused · resets in ${formatSpan(untilReset)}`, title: `No use in this window. ${asOf}` };
    const percent = `${Math.round(used)}%`;
    const elapsed = win.observedAtMs - (win.resetsAtMs - win.windowMinutes * 60000);
    if (elapsed < 5 * 60000) return { share: used / 100, text: `${percent} · resets in ${formatSpan(untilReset)}`, title: asOf };
    const hoursLeft = (100 - used) / (used / (elapsed / HOUR_MS));
    const outlasts = hoursLeft * HOUR_MS >= untilReset;
    const text = outlasts ? `${percent} · lasts to reset` : `${percent} · ${formatSpan(hoursLeft * HOUR_MS)} left`;
    const pace = outlasts ? "At this pace the window lasts until it resets." : `At this pace it runs out in about ${formatSpan(hoursLeft * HOUR_MS)}.`;
    return { share: used / 100, text, title: `${asOf} ${pace}` };
  }

  function setUsageRow(row, share, text, title) {
    row.querySelector(".usage-text").textContent = text;
    row.dataset.note = title; // the full sentence, shown in the hover panel
    const fill = Math.min(1, Math.max(0, share ?? 0));
    row.querySelector(".usage-fill").style.width = `${(fill * 100).toFixed(1)}%`;
    row.classList.toggle("is-warm", fill >= 0.7 && fill < 0.9);
    row.classList.toggle("is-hot", fill >= 0.9);
  }

  function renderUsage() {
    const now = Date.now();
    const official = claudeLimits && claudeLimits.status === "live" ? claudeLimits : null;
    if (official) {
      // Claude Code's own windows: the same figures /usage shows.
      $("#usage-claude .usage-name").textContent = "Claude 5h";
      const five = official.fiveHour ? officialClaudeUsage(official.fiveHour, now) : null;
      if (five) setUsageRow($("#usage-claude"), five.share, five.text, five.title);
      else setUsageRow($("#usage-claude"), null, "no 5h report yet", "Claude Code has not reported its five-hour window yet.");
      const week = official.sevenDay ? officialClaudeUsage(official.sevenDay, now) : null;
      $("#usage-claude-week").hidden = !week;
      if (week) setUsageRow($("#usage-claude-week"), week.share, week.text, week.title);
    } else {
      $("#usage-claude .usage-name").innerHTML = "Claude <em>est.</em>";
      $("#usage-claude-week").hidden = true;
      if (usageSnapshot) {
        const claude = claudeUsage(usageSnapshot.claude);
        setUsageRow($("#usage-claude"), claude.share, claude.text, claude.title);
      }
    }

    if (usageSnapshot) renderCodexRows(usageSnapshot.codex, now);
    renderUsageDetail();
  }

  function renderCodexRows(codex, now) {
    const five = codex && codex.fiveHour ? codexUsage(codex.fiveHour, now) : null;
    const week = codex && codex.weekly ? codexUsage(codex.weekly, now) : null;
    if (five) setUsageRow($("#usage-codex"), five.share, five.text, five.title);
    else setUsageRow($("#usage-codex"), null, "no recent use", "Codex has no rate-limit report from the last eight days.");
    $("#usage-week").hidden = !week;
    if (week) setUsageRow($("#usage-week"), week.share, week.text, week.title);
  }

  // "just now", "5m ago", "2h 05m ago". Under a minute, "0m ago" would read as a mistake.
  const formatAgo = (ms) => (ms < 60000 ? "just now" : `${formatSpan(ms)} ago`);

  // A reset time the user can read at a glance: "14:05" today, "Sun 14:05" later.
  function formatWhen(ms) {
    const when = new Date(ms);
    const sameDay = when.toDateString() === new Date().toDateString();
    return when.toLocaleString([], sameDay
      ? { hour: "2-digit", minute: "2-digit" }
      : { weekday: "short", hour: "2-digit", minute: "2-digit" });
  }

  // The hover panel: where each number comes from, then one block per visible meter with its full note.
  function renderUsageDetail() {
    const panel = $("#usage-detail");
    const codex = usageSnapshot ? usageSnapshot.codex : null;
    const official = claudeLimits && claudeLimits.status === "live";
    const rows = [...document.querySelectorAll("#usage .usage-row")].filter((row) => !row.hidden);
    const updated = usageSnapshot
      ? `Updated ${formatAgo(Date.now() - usageSnapshot.generatedAtMs)}. Refreshes every 30 seconds while Nesa is visible.`
      : "Reading usage from this Mac…";
    panel.replaceChildren(
      h("p", { class: "usage-detail-head" }, official
        ? "Claude: Claude Code's own limits, from its status line."
        : "Claude: an estimate from your transcripts, against the limit you set."),
      h("p", { class: "usage-detail-head" }, codex
        ? `Codex: its own report${codex.plan ? `, ${codex.plan} plan` : ""}.`
        : "Codex: no report in the last eight days."),
      ...rows.map((row) => h("div", { class: "usage-detail-item" },
        h("div", { class: "usage-detail-line" },
          h("strong", {}, row.querySelector(".usage-name").textContent),
          h("span", {}, row.querySelector(".usage-text").textContent)),
        h("p", { class: "usage-detail-note" }, row.dataset.note || ""))),
      h("p", { class: "usage-detail-foot" }, updated));
  }

  async function refreshUsage() {
    if (usageBusy || document.hidden) return; // the meters are only read while the window is visible
    usageBusy = true;
    try {
      usageSnapshot = await invoke("usage_snapshot");
      claudeLimits = await invoke("claude_limits").catch(() => null);
      renderUsage();
      if (state.view === "settings") {
        renderClaudeLimit();
        renderClaudeStatus();
      }
    } catch {
      setUsageRow($("#usage-claude"), null, "unavailable", "Usage could not be read from this Mac's logs.");
      setUsageRow($("#usage-codex"), null, "unavailable", "Usage could not be read from this Mac's logs.");
    } finally {
      usageBusy = false;
    }
  }

  // One of Claude Code's own windows, from its status line. The share is what /usage shows.
  function officialClaudeUsage(win, now) {
    const used = win.usedPercent;
    const resets = `resets in ${formatSpan(win.resetsAtMs - now)}`;
    const title = `Claude Code reported ${formatAgo(now - win.observedAtMs)}. The window resets ${formatWhen(win.resetsAtMs)}. The same figure /usage shows.`;
    if (used <= 0) return { share: 0, text: `unused · ${resets}`, title };
    return { share: used / 100, text: `${Math.round(used)}% · ${resets}`, title };
  }

  const CLAUDE_STATUS_LINE_TEXT = "Nesa will write ~/.claude/nesa-statusline.py and set statusLine in ~/.claude/settings.json. Your other settings stay as they are, and the original file is kept as settings.json.nesa-backup. While the status line is on, Claude Code hides some footer hints, such as the hold-space-to-speak hint.";

  // The Settings row for Claude Code's status line. Connecting adds the script and the entry; disconnecting removes them.
  function renderClaudeStatus() {
    const status = claudeLimits ? claudeLimits.status : "off";
    $("#claude-status-row").dataset.state = status;
    const captured = claudeLimits && claudeLimits.capturedAtMs;
    $("#claude-status-text").textContent = status === "live"
      ? `Connected · updated ${captured ? formatSpan(Date.now() - captured) : "just now"} ago`
      : status === "waiting" ? "Connected. Waiting for Claude's first reply in a session." : "Not connected";
    const action = $("#claude-status-action");
    action.disabled = false;
    action.textContent = status === "off" ? "Connect…" : "Disconnect";
    action.dataset.action = status === "off" ? "connect" : "disconnect";
    // Once Claude's own numbers are in, the estimate's limit field is not needed. The estimate opens by itself
    // once a limit is set, and otherwise stays as the user left it.
    $("#claude-calibrate-row").hidden = status === "live";
    const estimate = $("#claude-estimate");
    estimate.hidden = status === "live";
    if (settings.claudeLimit) estimate.open = true;
  }

  async function toggleClaudeStatus() {
    const action = $("#claude-status-action");
    if (!action.dataset.action) return;
    const connecting = action.dataset.action === "connect";
    const ok = await confirmDialog(connecting
      ? { title: "Connect Claude usage?", text: CLAUDE_STATUS_LINE_TEXT, label: "Connect", danger: false }
      : { title: "Disconnect Claude usage?", text: "Nesa removes its entry from Claude's settings.json and deletes its script and last report. Other settings stay.", label: "Disconnect", danger: false });
    if (!ok) return;
    const result = await attempt(connecting ? "connect_claude_limits" : "disconnect_claude_limits");
    if (!result.ok) return;
    claudeLimits = result.value;
    renderClaudeStatus();
    renderUsage();
    toast(connecting ? "Connected. Claude's limits appear after its next reply." : "Disconnected.", "ok");
  }

  function renderClaudeLimit() {
    const limit = settings.claudeLimit;
    $("#claude-limit-status").textContent = limit
      ? `about ${formatTokens(limit)} tokens per 5 hours, set ${new Date(settings.claudeLimitAt).toLocaleString([], { dateStyle: "medium", timeStyle: "short" })}`
      : "Not set";
    $("#claude-limit-clear").hidden = !limit;
  }

  // The limit is calibrated from the percent /usage shows right now: limit = tokens so far / share used.
  async function calibrateClaudeLimit() {
    const error = $("#claude-limit-error");
    const input = $("#claude-calibrate-input");
    const percent = Number(input.value);
    await refreshUsage();
    const tokens = usageSnapshot ? usageSnapshot.claude.tokens5h : 0;
    let problem = "";
    if (!Number.isFinite(percent) || percent < 1 || percent > 100) problem = "Enter the percent /usage shows, from 1 to 100.";
    else if (percent < 5) problem = "Use a reading of 5% or more. Smaller readings round too much to give a useful limit.";
    else if (tokens <= 0) problem = "There are no Claude tokens in the last five hours to measure against yet. Use Claude, then set the limit.";
    error.textContent = problem;
    error.hidden = !problem;
    if (problem) return;
    settings.claudeLimit = Math.round(tokens / (percent / 100));
    settings.claudeLimitAt = Date.now();
    saveSettings();
    input.value = "";
    renderClaudeLimit();
    renderUsage();
  }

  function clearClaudeLimit() {
    settings.claudeLimit = 0;
    settings.claudeLimitAt = 0;
    saveSettings();
    renderClaudeLimit();
    renderUsage();
  }

  function startUsage() {
    refreshUsage();
    setInterval(refreshUsage, USAGE_REFRESH_MS);
    document.addEventListener("visibilitychange", () => { if (!document.hidden) refreshUsage(); });
  }

  // Navigation, the Home actions, and the Settings controls. Called once from bindUi.
  function bindHomeAndSettings() {
    document.querySelectorAll("[data-view]").forEach((node) =>
      node.addEventListener("click", () => showView(node.dataset.view)));
    $("#home-claude").addEventListener("click", () => openSessionDialog("claude"));
    $("#home-codex").addEventListener("click", () => openSessionDialog("codex"));
    $("#home-terminal").addEventListener("click", () => openSessionDialog("terminal"));
    $("#home-browser").addEventListener("click", () => openSessionDialog("browser"));
    $("#resume-refresh").addEventListener("click", loadTranscripts);
    // The usage block opens Settings at the Usage section.
    $("#usage").addEventListener("click", () => {
      showView("settings");
      requestAnimationFrame(() => jumpToSection("usage-settings", false));
    });
    el.settingsIndex.querySelectorAll("button").forEach((btn) =>
      btn.addEventListener("click", () => jumpToSection(btn.dataset.target, true)));
    el.settings.addEventListener("scroll", syncSettingsIndex, { passive: true });
    $("#claude-calibrate").addEventListener("click", calibrateClaudeLimit);
    $("#claude-calibrate-input").addEventListener("keydown", (e) => { if (e.key === "Enter") calibrateClaudeLimit(); });
    $("#claude-limit-clear").addEventListener("click", clearClaudeLimit);
    $("#claude-status-action").addEventListener("click", toggleClaudeStatus);
    el.resumeSearch.addEventListener("input", (e) => {
      state.resumeQuery = e.target.value;
      resetResumeShown();
      renderResumeList();
    });
    el.resumeGroup.addEventListener("change", (e) => { state.resumeGroup = Number(e.target.value); });
    el.resumeFilter.querySelectorAll("button").forEach((btn) => btn.addEventListener("click", () => {
      state.resumeFilter = btn.dataset.filter;
      resetResumeShown();
      renderResumeGroups();
      renderResumeList();
    }));
    el.accentCustom.addEventListener("input", (e) => {
      el.accentHex.textContent = e.target.value.toUpperCase();
      setColor("accent", e.target.value);
    });
    el.accentCustom.addEventListener("change", (e) => setSetting("accent", e.target.value));
    el.accentReset.addEventListener("click", () => setSetting("accent", DEFAULT_SETTINGS.accent));
    $("#colors-reset").addEventListener("click", resetColors);
    bindImageGen();
  }

  // ------------------------------------------------------------- browser

  // A browser page is a native view over the window, so it cannot be layered under HTML. It is placed on its
  // card's body, and it is hidden whenever a dialog, a menu, the image viewer, or another view covers the column.
  const mainCovered = () => anyDialogOpen() || closeActiveMenu !== null || !el.viewer.hidden || state.view !== "group";

  // Where a card's page belongs, in window coordinates. Parked cards and covered columns hide it.
  function browserFrame(card) {
    const rect = card.body.getBoundingClientRect();
    const visible = card.root.parentNode === el.grid && rect.width > 0 && rect.height > 0 && !mainCovered();
    return { x: rect.left, y: rect.top, width: rect.width, height: rect.height, visible };
  }

  // Positions every browser page that has an address. Requests are batched to one per frame, because a
  // single render can move several cards.
  let browserSyncQueued = false;
  function syncBrowsers() {
    if (browserSyncQueued) return;
    browserSyncQueued = true;
    requestAnimationFrame(() => {
      browserSyncQueued = false;
      const items = [];
      cards.forEach((card) => {
        const session = findSession(card.id);
        if (session && session.kind === "browser" && session.url) items.push({ id: card.id, ...browserFrame(card) });
      });
      if (items.length) invoke("browser_sync", { items }).catch(() => {});
    });
  }

  // Opens an address, or a search, in a browser session. The first address creates the page.
  async function navigateBrowser(id, text) {
    const card = cards.get(id);
    if (!card) return;
    const result = await attempt("browser_navigate", { id, url: text, frame: browserFrame(card) });
    if (!result.ok) return;
    const session = findSession(id);
    if (session) {
      session.url = result.value;
      updateCardInfo(card, session);
    }
    if (card.urlInput) card.urlInput.value = result.value;
    syncBrowsers();
  }

  // The page moved on its own (a link, a redirect, back or forward). Keep the session's address and the bar.
  function onBrowserPage({ id, url }) {
    const session = findSession(id);
    if (session) session.url = url;
    const card = cards.get(id);
    if (!card) return;
    if (card.urlInput && document.activeElement !== card.urlInput) card.urlInput.value = url;
    if (session) updateCardInfo(card, session);
  }

  // ---------------------------------------------------------- image links

  // A path to an image in a terminal becomes a link. A double click opens it in the viewer. Only paths that
  // name a real image are linked, so a word that merely ends in ".png" stays plain text.
  const IMAGE_PATH = /(?:~\/|\/|\.{1,2}\/)?[^\s"'`()<>[\]{}|,;]*?\.(?:png|jpe?g|gif|webp|bmp|svg)(?![\w.])/gi;
  const IMAGE_RECHECK_MS = 10000; // a path that was not an image is looked up again after this long
  const IMAGE_BATCH = 64; // matches the backend's limit per request
  const imageCache = new Map(); // "folder\0path" -> { path: string | null, at: number }

  // The text of the logical line that row y belongs to. A wrapped line is one line on screen, so its rows
  // are joined. Each character remembers its row and cell, so a match maps back to a place on the screen.
  function logicalLine(term, y) {
    const buffer = term.buffer.active;
    let top = y - 1;
    while (top > 0 && buffer.getLine(top)?.isWrapped) top -= 1;
    let bottom = y - 1;
    while (bottom + 1 < buffer.length && buffer.getLine(bottom + 1)?.isWrapped) bottom += 1;
    let text = "";
    const cells = [];
    for (let row = top; row <= bottom; row += 1) {
      const line = buffer.getLine(row);
      for (let x = 0; line && x < line.length; x += 1) {
        const cell = line.getCell(x);
        if (!cell || cell.getWidth() === 0) continue; // the second half of a wide character
        const chars = cell.getChars() || " ";
        text += chars;
        for (let k = 0; k < chars.length; k += 1) cells.push({ row, x });
      }
    }
    return { text, cells };
  }

  // Asks the backend which of these paths are images, in batches. Answers are kept, and a path that was
  // not an image is asked about again after a while, in case the file has since appeared.
  async function resolveImages(folder, tokens) {
    const now = Date.now();
    const unknown = [...new Set(tokens)].filter((token) => {
      const hit = imageCache.get(`${folder}\0${token}`);
      return !hit || (hit.path === null && now - hit.at > IMAGE_RECHECK_MS);
    });
    for (let i = 0; i < unknown.length; i += IMAGE_BATCH) {
      const batch = unknown.slice(i, i + IMAGE_BATCH);
      const answers = await invoke("resolve_images", { cwd: folder, candidates: batch }).catch(() => batch.map(() => null));
      batch.forEach((token, j) => imageCache.set(`${folder}\0${token}`, { path: answers[j] ?? null, at: now }));
    }
    return tokens.map((token) => imageCache.get(`${folder}\0${token}`)?.path ?? null);
  }

  // xterm asks for the links of one row at a time, as the pointer moves. Only the links on that row are returned.
  function registerImageLinks(term, session) {
    term.registerLinkProvider({
      provideLinks(y, callback) {
        const line = logicalLine(term, y);
        const matches = [...line.text.matchAll(IMAGE_PATH)];
        if (!matches.length) { callback(undefined); return; }
        resolveImages(session.cwd, matches.map((m) => m[0]))
          .then((paths) => {
            const links = [];
            matches.forEach((m, i) => {
              const path = paths[i];
              const first = line.cells[m.index];
              const last = line.cells[m.index + m[0].length - 1];
              if (!path || !first || !last) return;
              if (y - 1 < first.row || y - 1 > last.row) return;
              links.push({
                range: { start: { x: first.x + 1, y: first.row + 1 }, end: { x: last.x + 1, y: last.row + 1 } },
                text: m[0],
                // Clicking leaves the text alone, so selecting it works as usual. Right-click offers the picture
                // (see bindImageMenu); the underline on hover is what marks the path.
                activate: () => {},
              });
            });
            callback(links.length ? links : undefined);
          })
          .catch(() => callback(undefined));
      },
    });
  }

  // The terminal cell under the pointer: its absolute buffer row and its column. Null outside the grid.
  function terminalCellAt(term, event) {
    const screen = term.element?.querySelector(".xterm-screen");
    if (!screen) return null;
    const rect = screen.getBoundingClientRect();
    if (!rect.width || !rect.height) return null;
    const col = Math.floor((event.clientX - rect.left) / (rect.width / term.cols));
    const viewRow = Math.floor((event.clientY - rect.top) / (rect.height / term.rows));
    if (col < 0 || col >= term.cols || viewRow < 0 || viewRow >= term.rows) return null;
    return { row: term.buffer.active.viewportY + viewRow, col };
  }

  // The image path under a terminal cell, or null. It is found from the same logical line the link provider uses.
  function imagePathAt(term, cell) {
    if (!cell) return null;
    const line = logicalLine(term, cell.row + 1);
    return [...line.text.matchAll(IMAGE_PATH)].find((m) => {
      const first = line.cells[m.index];
      const last = line.cells[m.index + m[0].length - 1];
      if (!first || !last) return false;
      const afterStart = cell.row > first.row || (cell.row === first.row && cell.col >= first.x);
      const beforeEnd = cell.row < last.row || (cell.row === last.row && cell.col <= last.x);
      return afterStart && beforeEnd;
    }) || null;
  }

  // Right-clicking an image path opens a menu: View image, Copy image, Copy path. Anywhere else the right click
  // is left alone. A double click on a path does not select it, so the highlight does not run on into the line.
  function bindImageMenu(term, session, body) {
    // Capture phase on the card, so this runs before xterm sees the press.
    body.addEventListener("mousedown", (event) => {
      if (event.detail < 2 || !imagePathAt(term, terminalCellAt(term, event))) return;
      event.stopPropagation();
      event.preventDefault();
    }, true);
    body.addEventListener("contextmenu", (event) => {
      const match = imagePathAt(term, terminalCellAt(term, event));
      if (!match) return;
      event.preventDefault();
      const { clientX: x, clientY: y } = event;
      resolveImages(session.cwd, [match[0]]).then(([path]) => {
        if (!path) {
          toast("That picture is no longer there", "info");
          return;
        }
        showMenu(x, y, [
          { label: "View image", onSelect: () => openImage(path, match[0]) },
          { label: "Copy image", onSelect: () => attempt("imagegen_copy", { path }) },
          { label: "Copy path", onSelect: () => attempt("copy_text", { text: path }) },
        ]);
      });
    });
  }

  // ------------------------------------------------------------ image viewer

  async function openImage(path, label) {
    const result = await attempt("read_image", { path });
    if (!result.ok) return;
    el.viewerName.textContent = label.split("/").pop() || label;
    el.viewerName.title = path;
    el.viewerImage.src = result.value;
    el.viewer.hidden = false;
    el.viewerClose.focus();
    syncBrowsers();
  }

  function closeImage() {
    if (el.viewer.hidden) return;
    el.viewer.hidden = true;
    el.viewerImage.removeAttribute("src");
    syncBrowsers();
  }

  function bindImageViewer() {
    el.viewerClose.addEventListener("click", closeImage);
    el.viewerImage.addEventListener("error", () => toast("That image could not be shown"));
    el.viewer.addEventListener("mousedown", (e) => {
      if (e.target === el.viewer || e.target.classList.contains("viewer-body")) closeImage();
    });
  }

  // ------------------------------------------------------------ files and images

  const DROP_LIMIT = 25 * 1024 * 1024; // bytes; larger files are refused rather than sent over IPC

  // A dropped file is copied to a cache folder and its path is typed into the session under the
  // pointer, or the focused one. Claude and Codex read image paths as attachments.
  function cardAtPoint(x, y) {
    const node = document.elementFromPoint(x, y);
    const entry = node && node.closest(".card[data-id]") ? cards.get(node.closest(".card[data-id]").dataset.id) : null;
    return entry && entry.opened && entry.live ? entry : focusedLiveCard();
  }

  const hasFiles = (e) => Boolean(e.dataTransfer) && [...e.dataTransfer.types].includes("Files");

  function readAsBase64(file) {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result).split(",")[1] || "");
      reader.onerror = () => reject(reader.error);
      reader.readAsDataURL(file);
    });
  }

  async function attachFiles(files, card) {
    const paths = [];
    for (const file of files) {
      if (file.size > DROP_LIMIT) {
        toast(`${file.name} is larger than 25 MB`);
        continue;
      }
      const result = await attempt("save_dropped_file", { name: file.name, dataBase64: await readAsBase64(file) });
      if (result.ok) paths.push(result.value);
    }
    if (paths.length) card.term.paste(`${paths.join(" ")} `);
  }

  // Without this the webview opens the dropped file in place of the app.
  function onWindowDragOver(e) {
    if (hasFiles(e)) {
      e.preventDefault();
      e.dataTransfer.dropEffect = "copy";
    }
  }

  function onWindowDrop(e) {
    if (!hasFiles(e)) return; // the app's own drags (reordering sessions) are handled elsewhere
    e.preventDefault();
    const card = cardAtPoint(e.clientX, e.clientY);
    if (!card) {
      toast("Open a running session first, then drop the file on it");
      return;
    }
    attachFiles([...e.dataTransfer.files], card);
  }

  // Cmd+V with an image on the clipboard has no text to paste. Ctrl+V goes to the agent instead, and
  // Claude and Codex read the image from the clipboard themselves.
  function onWindowPaste(e) {
    const data = e.clipboardData;
    const card = focusedLiveCard();
    if (!data || !card || data.getData("text/plain")) return;
    if (![...data.items].some((item) => item.type.startsWith("image/"))) return;
    e.preventDefault();
    e.stopPropagation();
    invoke("write_pty", { id: card.id, data: "\x16" }).catch(() => {});
  }

  // ------------------------------------------------------------- dictation

  // Hold the voice-mode keys to talk; release to type the words into the focused session.
  // The mic opens after a short hold, so a quick tap or a shortcut that uses the keys does nothing.
  const DICTATION_HOLD_DELAY = 180; // ms of hold before the mic opens
  // macOS system sounds, played natively by the backend.
  const SOUNDS = { start: "Tink", stop: "Pop", done: "Glass" };
  const dictation = {
    state: "idle", // "idle" | "held" (the voice-mode keys are down) | "ignoring" (a shortcut was typed during the chord)
    shown: false, // the pill is up
    recording: false, // the mic is open
    timer: 0,
    frame: 0,
    level: 0, // smoothed mic level, drives the bars
    target: 0, // last level from the helper
    audio: null,
    startedAt: 0, // when the mic opened, for the elapsed clock
    clock: -1, // the last whole second written to the meta line
    destination: "", // name of the session the words will be typed into; "" when none is focused
    elsewhere: false, // the hold began while another app was in front, so the words are pasted there
    pendingPaste: "", // the words waiting for the helper to say where they went
  };
  // Shown while the mic is open and no words have arrived. It names the voice-mode key that is held.
  const placeholder = () => `Speak, then release ${hotkey().symbol}`;
  const hud = {
    root: $("#dictation"),
    label: $("#dictation-label"),
    meta: $("#dictation-meta"),
    text: $("#dictation-text"),
    words: $("#dictation-words"),
    bars: [...document.querySelectorAll(".dictation-bars i")],
    hideTimer: 0,
    watchdog: 0, // gives up when the engine answers a release with neither a final nor an error
  };

  // The voice-mode keys come from settings. Modifier keys never type anything, so pressing one
  // while the chord is held is not a shortcut.
  const hotkey = () => HOTKEYS[settings.hotkey];
  const isModifierKey = (e) => ["Alt", "Control", "Meta", "Shift"].includes(e.key);
  // Exactly the chord's keys are down: Option alone is not Option+Shift.
  const chordDown = (e) => {
    const keys = hotkey().keys;
    return Object.keys(MODIFIER_FLAG).every((name) => e[MODIFIER_FLAG[name]] === keys.includes(name));
  };
  // Every key of the chord is still held.
  const chordHeld = (e) => hotkey().keys.every((name) => e[MODIFIER_FLAG[name]]);

  // The newest words matter most while speaking, so keep the end of a long transcript and cut at a word.
  function tail(text) {
    if (text.length <= 240) return text;
    const cut = text.slice(-240);
    const space = cut.indexOf(" ");
    return space > 0 ? cut.slice(space + 1) : cut;
  }

  const clockText = (seconds) => `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
  // The app the words will go to: a Nesa session, or the app that was in front when the hold began.
  const listeningMeta = (seconds) => `${clockText(seconds)} · ${dictation.destination || (dictation.elsewhere ? "the app in front" : "no session focused")}`;

  function playSound(name) {
    invoke("play_sound", { name: SOUNDS[name] }).catch(() => {});
  }

  // A transcript is one line, scrolled to its end, so the newest words stay visible. Short text stays at
  // the start, where it reads, and the left edge fades only while text is clipped behind it. Messages
  // keep their first words and end with an ellipsis instead.
  function showWords(text, kind = "tail") {
    hud.words.textContent = text;
    hud.text.dataset.kind = kind;
    hud.text.scrollLeft = kind === "tail" ? hud.text.scrollWidth : 0;
    hud.text.toggleAttribute("data-clipped", hud.text.scrollLeft > 0);
  }

  // Changes the pill's state. Only the fields that are passed change. A state change cancels a pending hide.
  function setHud(mode, { label, words, kind = "tail", meta, warn } = {}) {
    clearTimeout(hud.hideTimer);
    hud.root.dataset.mode = mode;
    hud.root.classList.add("is-visible");
    if (warn !== undefined) hud.root.classList.toggle("is-warn", warn);
    if (label !== undefined) hud.label.textContent = label;
    if (words !== undefined) showWords(words, kind);
    if (meta !== undefined) hud.meta.textContent = meta;
    if (dictation.elsewhere) mirrorHud();
  }

  function hideHud(delay = 0) {
    clearTimeout(hud.hideTimer);
    hud.hideTimer = setTimeout(() => {
      if (dictation.state === "held") return; // a new hold has started. "ignoring" still hides the pill.
      dictation.shown = false;
      dictation.target = 0;
      hud.root.dataset.mode = "idle"; // back to the dot, without the last mode's colour
      hud.root.classList.remove("is-visible", "is-warn");
      if (dictation.elsewhere) {
        mirrorHud(); // shown is false now, so this hides the pill over the other app
        dictation.elsewhere = false;
      }
    }, delay);
  }

  // The bars follow the real microphone level, with a small per-bar wobble so they feel alive. The loop
  // runs only while listening. Processing and done are styled in CSS, so the bars are left alone there.
  function animateHud() {
    dictation.frame = 0;
    if (!dictation.shown || hud.root.dataset.mode !== "listening") return;
    dictation.target *= 0.92; // decays between level events
    dictation.level += (dictation.target - dictation.level) * 0.3;
    const now = performance.now() / 1000;
    hud.bars.forEach((bar, i) => {
      const weight = [0.5, 0.85, 1, 0.85, 0.5][i];
      const wobble = 0.8 + 0.2 * Math.sin(now * 8 + i * 1.9);
      // The square root lifts quiet speech, which is most of what a mic hears, so the meter still moves.
      const height = Math.max(0.16, Math.min(1, Math.sqrt(dictation.level) * weight * wobble * 1.3));
      bar.style.transform = `scaleY(${height.toFixed(3)})`;
    });
    // The clock is read here, in the same frame loop, and written only when the second changes.
    const seconds = Math.floor((Date.now() - dictation.startedAt) / 1000);
    if (seconds !== dictation.clock) {
      dictation.clock = seconds;
      hud.meta.textContent = listeningMeta(seconds);
      if (dictation.elsewhere) mirrorHud();
    }
    dictation.frame = requestAnimationFrame(animateHud);
  }

  // The engine should answer a release with "final" or "error". If it answers with neither, the pill
  // must not stay on "Transcribing" forever.
  function armWatchdog() {
    clearWatchdog();
    hud.watchdog = setTimeout(() => {
      hud.watchdog = 0;
      setHud("error", { label: "No answer from speech", words: `Dictation timed out. Hold ${hotkey().symbol} to try again.`, kind: "message", warn: false });
      hideHud(3000);
    }, 8000);
  }

  function clearWatchdog() {
    clearTimeout(hud.watchdog);
    hud.watchdog = 0;
  }

  function beginDictation() {
    // Ignore the chord while typing in a text field that is not a terminal (rename, dialogs).
    if (nesaTyping()) return;
    clearWatchdog(); // a new hold ends the wait for the last transcript
    dictation.state = "held";
    dictation.timer = setTimeout(startListening, DICTATION_HOLD_DELAY);
  }

  function startListening() {
    if (dictation.state !== "held") return;
    dictation.recording = true;
    dictation.shown = true;
    dictation.startedAt = Date.now();
    dictation.clock = -1;
    const card = focusedLiveCard();
    const session = card && findSession(card.id);
    dictation.destination = session ? session.name : "";
    // Nesa in front types into its own sessions. Any other app in front gets the words pasted, with the pill over it.
    dictation.elsewhere = !document.hasFocus();
    // Say where the words will go now, so a missing target is not discovered after a whole sentence.
    setHud("listening", {
      label: "Listening",
      words: placeholder(),
      kind: "message",
      meta: listeningMeta(0),
      warn: !session && !dictation.elsewhere,
    });
    playSound("start");
    if (!dictation.frame) animateHud();
    attempt("dictation_start", { words: vocabularyWords(), builtin: settings.builtinWords }).then((result) => {
      if (!result.ok) cancelDictation();
    });
  }

  function releaseDictation() {
    clearTimeout(dictation.timer);
    dictation.state = "idle";
    dictation.target = 0;
    if (!dictation.recording) {
      // Released before the mic opened, or after an error already explained the failure.
      if (dictation.shown) hideHud(2500);
      return;
    }
    dictation.recording = false;
    if (dictation.shown) {
      setHud("processing", { label: "Transcribing", words: hud.words.textContent, kind: hud.text.dataset.kind, meta: "" });
      playSound("stop");
      armWatchdog();
    }
    attempt("dictation_stop");
  }

  function cancelDictation(nextState = "idle") {
    const wasRecording = dictation.recording;
    if (dictation.state === "idle" && !wasRecording) return;
    clearTimeout(dictation.timer);
    clearWatchdog();
    dictation.state = nextState;
    dictation.recording = false;
    dictation.shown = false;
    hideHud();
    if (wasRecording) attempt("dictation_cancel");
  }

  // ------------------------------------------- dictation in every app

  // While the helper's key watch runs, it reports the voice-mode chord in every app, and the window's
  // key handlers step aside. Without it (no Input Monitoring yet), the window still handles Nesa's own keys.
  let keysWatched = false;
  const access = { input: null, accessibility: null }; // null until the helper has said
  const PASTE_REASONS = {
    target: "No app was in front when you started",
    secure: "A secure field is active, so nothing was typed",
    accessibility: "Allow Nesa in Accessibility to type into other apps",
    moved: "The app changed while you spoke",
  };

  // Typing into a text field of Nesa itself. Dictation steps aside then, so a rename is not interrupted.
  const nesaTyping = () => {
    const active = document.activeElement;
    return document.hasFocus() && !!active && active.matches("input, textarea") && !active.classList.contains("xterm-helper-textarea");
  };

  // The pill over another app. Nesa sends the state; the level bars come from the helper's events.
  function mirrorHud() {
    invoke("dictation_hud", {
      state: {
        visible: dictation.elsewhere && dictation.shown,
        mode: hud.root.dataset.mode,
        warn: hud.root.classList.contains("is-warn"),
        label: hud.label.textContent,
        meta: hud.meta.textContent,
        words: hud.words.textContent,
        kind: hud.text.dataset.kind,
      },
    }).catch(() => {});
  }

  // The chord the helper watches. It follows the voice-mode key setting.
  const syncChord = () => invoke("dictation_keys", { keys: hotkey().keys }).catch(() => {});
  // Starts the key watch if it is not running, and reports the permissions. Runs at launch and on focus.
  const watchKeys = () => invoke("dictation_watch").catch(() => {});

  function onChordEvent(chordState) {
    if (chordState === "held") {
      // A dialog in Nesa only blocks the hold while Nesa is in front. Behind another app it cannot be in the way.
      if (dictation.state !== "idle" || nesaTyping() || (document.hasFocus() && anyDialogOpen())) return;
      clearWatchdog(); // a new hold ends the wait for the last transcript
      dictation.state = "held";
      startListening();
    } else if (chordState === "other") {
      if (dictation.state === "held") cancelDictation("ignoring");
    } else if (chordState === "up") {
      releaseDictation();
    }
  }

  // The words go to the app that was in front when the hold began. The helper answers where they went.
  function sendToApp(spoken) {
    dictation.pendingPaste = spoken;
    armWatchdog();
    const text = settings.afterTyping === "space" ? `${spoken} ` : spoken;
    invoke("dictation_paste", { text, pressReturn: settings.afterTyping === "return" }).catch((err) => {
      onPasteResult({ ok: false, error: String(err) });
    });
  }

  function onPasteResult(event) {
    clearWatchdog();
    const spoken = dictation.pendingPaste;
    dictation.pendingPaste = "";
    if (!spoken) return;
    if (event.ok) {
      recordHistory(spoken, event.app || "");
      setHud("done", { label: `Typed into ${event.app || "the app"}`, words: tail(spoken), kind: "tail", meta: "", warn: false });
      if (settings.typedSound) playSound("done");
      hideHud(1600);
    } else {
      recordHistory(spoken, "");
      setHud("error", { label: "Not typed", words: tail(spoken), kind: "tail", meta: PASTE_REASONS[event.reason] || event.error || "Nothing was typed", warn: true });
      hideHud(4500);
    }
  }

  // Input Monitoring and Accessibility, as the helper reports them.
  function onAccess(event) {
    access.input = event.input;
    access.accessibility = event.accessibility;
    el.accessInput.textContent = event.input ? "Allowed" : "Not allowed";
    el.accessInput.dataset.granted = String(event.input);
    el.accessAccessibility.textContent = event.accessibility ? "Allowed" : "Not allowed";
    el.accessAccessibility.dataset.granted = String(event.accessibility);
  }

  // Opens the System Settings pane for one permission, after asking macOS for both.
  function openAccess(pane) {
    attempt("dictation_request_access");
    attempt("dictation_open_privacy", { pane });
  }

  // Dictation only types into a card the user can see, so Home and Settings never receive text. A card
  // only counts while Nesa is in front, so words meant for another app never land in a Nesa session.
  function focusedLiveCard() {
    if (state.view !== "group" || !document.hasFocus()) return null;
    const card = state.focusedId ? cards.get(state.focusedId) : null;
    return card && card.opened && card.live ? card : null;
  }

  function finishDictation(text) {
    clearWatchdog();
    const spoken = applyVocabulary(text.trim());
    if (!spoken) {
      setHud("error", { label: "Nothing heard", words: `Hold ${hotkey().symbol} and speak a little longer`, kind: "message", meta: "", warn: true });
      hideHud(1800);
      return;
    }
    const card = focusedLiveCard();
    if (card) {
      // paste() honours bracketed paste, so newlines do not submit the prompt early.
      card.term.paste(settings.afterTyping === "space" ? `${spoken} ` : spoken);
      // Return goes to the session after a short pause, so it reads as a keypress and not as more pasted text.
      if (settings.afterTyping === "return") setTimeout(() => invoke("write_pty", { id: card.id, data: "\r" }).catch(() => {}), 50);
      const session = findSession(card.id);
      recordHistory(spoken, session ? session.name : "");
      setHud("done", { label: `Typed into ${session ? session.name : "the session"}`, words: tail(spoken), kind: "tail", meta: "", warn: false });
      if (settings.typedSound) playSound("done");
      hideHud(1600);
    } else if (dictation.elsewhere) {
      sendToApp(spoken);
    } else {
      // Nothing to type into: keep the words on screen so they are not lost, and in the history.
      recordHistory(spoken, "");
      setHud("error", { label: "Not typed", words: tail(spoken), kind: "tail", meta: "click a running session first", warn: true });
      hideHud(4500);
    }
  }

  function onDictationEvent(event) {
    switch (event.event) {
      case "level":
        dictation.target = Math.min(1, event.value);
        break;
      case "partial":
        if (dictation.shown) {
          showWords(event.text ? tail(event.text) : placeholder(), event.text ? "tail" : "message");
          if (dictation.elsewhere) mirrorHud();
        }
        break;
      case "keys":
        onChordEvent(event.state);
        break;
      case "tap":
        keysWatched = event.state === "on";
        break;
      case "access":
        onAccess(event);
        break;
      case "target":
        // The app the words will go to, named by the helper when the hold began.
        if (dictation.elsewhere && dictation.recording && event.app) {
          dictation.destination = event.app;
          hud.meta.textContent = listeningMeta(Math.max(0, Math.floor((Date.now() - dictation.startedAt) / 1000)));
          mirrorHud();
        }
        break;
      case "paste":
        onPasteResult(event);
        break;
      case "final":
        finishDictation(event.text || "");
        break;
      case "state":
        if (event.state === "downloading" && dictation.shown) {
          setHud("listening", { label: "Preparing speech", words: "Downloading the speech model. This happens once.", kind: "message" });
        }
        break;
      case "error":
        clearWatchdog();
        dictation.recording = false; // the engine has closed the mic, so a release must not wait for a final
        dictation.target = 0;
        if (dictation.shown) setHud("error", { label: "Dictation unavailable", words: event.message, kind: "message", warn: false });
        else toast(event.message);
        if (dictation.state === "idle") hideHud(4000);
        break;
      case "exited":
        toast("Dictation stopped. It restarts on the next hold.");
        if (dictation.state === "idle") hideHud();
        break;
    }
  }

  // The chord is the only trigger. Keys typed while it is held are ordinary input, so they reach the
  // terminal, and dictation steps aside.
  function onDictationKeyDown(e) {
    if (keysWatched) return; // the helper reports the chord in every app
    if (dictation.state === "idle") {
      if (!chordDown(e) || anyDialogOpen()) return;
      beginDictation();
      return;
    }
    if (dictation.state === "held" && !isModifierKey(e)) cancelDictation("ignoring");
  }

  function onDictationKeyUp(e) {
    if (keysWatched || dictation.state === "idle" || chordHeld(e)) return; // a key of the chord is still down
    releaseDictation();
  }

  // ------------------------------------------------------- alerts and quitting

  const alertsOn = () => stored.get("alerts") !== "off";

  function syncAlerts() {
    const on = alertsOn();
    el.alertsToggle.setAttribute("aria-checked", String(on));
    invoke("set_alerts", { enabled: on }).catch(() => {});
  }

  function toggleAlerts() {
    stored.set("alerts", alertsOn() ? "off" : "on");
    syncAlerts();
  }

  // Quitting stops every agent, so the user confirms when any of them is still working.
  async function confirmQuit(working) {
    if (el.confirmDialog.open) return;
    const ok = await confirmDialog({
      title: "Quit Nesa?",
      text: `${working === 1 ? "A session is" : `${working} sessions are`} still working. Quitting stops ${working === 1 ? "it" : "them"} mid-task. Sessions reopen with their transcripts the next time you open them.`,
      label: "Quit",
    });
    if (ok) invoke("quit_app").catch(() => {});
  }

  // ---------------------------------------------------------------- events

  // Shortcuts that reach the terminal are also listed in handleTermKey, so the shell never sees them.
  function onKeyDown(e) {
    if (e.key === "Escape" && !el.viewer.hidden) { e.preventDefault(); closeImage(); return; }
    if (activeMenu && (e.key === "ArrowDown" || e.key === "ArrowUp")) {
      e.preventDefault();
      moveMenuFocus(e.key === "ArrowDown" ? 1 : -1);
      return;
    }
    const typing = document.activeElement && document.activeElement.classList.contains("xterm-helper-textarea");
    if (e.key === "Escape" && (closeActiveMenu || !typing)) {
      if (closeActiveMenu) { e.preventDefault(); closeMenu(); return; }
      if (!anyDialogOpen() && state.maximizedId) toggleMaximize(state.maximizedId);
      return;
    }
    if (!e.metaKey || e.ctrlKey || e.altKey) return;
    closeMenu();
    if (anyDialogOpen()) return;
    if (e.key === "n") {
      e.preventDefault();
      openSessionDialog();
    } else if (/^[1-9]$/.test(e.key)) {
      e.preventDefault();
      selectGroup(Number(e.key) - 1);
    } else if (e.key === "]" || e.key === "[") {
      e.preventDefault();
      cycleFocus(e.key === "]" ? 1 : -1);
    } else if (e.key === "0") {
      e.preventDefault();
      showView("home");
    } else if (e.key === ",") {
      e.preventDefault();
      showView("settings");
    } else if (e.key === "j") {
      e.preventDefault();
      jumpToFinished();
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (state.focusedId) toggleMaximize(state.focusedId);
    }
  }

  function bindUi() {
    document.querySelectorAll("[data-logo]").forEach((node) => { node.innerHTML = LOGOS[node.dataset.logo]; });
    document.querySelectorAll("[data-icon]").forEach((node) => { node.innerHTML = ICONS[node.dataset.icon]; });

    $("#new-group").addEventListener("click", createGroup);
    $("#new-claude").addEventListener("click", () => openSessionDialog("claude"));
    $("#new-codex").addEventListener("click", () => openSessionDialog("codex"));
    $("#new-terminal").addEventListener("click", () => openSessionDialog("terminal"));
    $("#new-browser").addEventListener("click", () => openSessionDialog("browser"));
    $("#empty-terminal").addEventListener("click", () => openSessionDialog("terminal"));
    $("#empty-browser").addEventListener("click", () => openSessionDialog("browser"));
    $("#empty-claude").addEventListener("click", () => openSessionDialog("claude"));
    $("#empty-codex").addEventListener("click", () => openSessionDialog("codex"));

    document.querySelectorAll("#new-session .segmented button").forEach((btn) =>
      btn.addEventListener("click", () => setKind(btn.dataset.kind)));
    $("#count-less").addEventListener("click", () => setCount(state.newCount - 1));
    $("#count-more").addEventListener("click", () => setCount(state.newCount + 1));
    bindImageViewer();
    window.addEventListener("resize", syncBrowsers);
    [el.sessionDialog, el.confirmDialog, el.promptDialog].forEach((dialog) => dialog.addEventListener("close", syncBrowsers));
    $("#browse").addEventListener("click", browseFolder);
    el.alertsToggle.addEventListener("click", toggleAlerts);
    el.typedSoundToggle.addEventListener("click", () => setSetting("typedSound", !settings.typedSound));
    document.querySelectorAll("[data-access]").forEach((button) => button.addEventListener("click", () => openAccess(button.dataset.access)));
    el.builtinToggle.addEventListener("click", () => setSetting("builtinWords", !settings.builtinWords));
    el.vocabForm.addEventListener("submit", addVocabulary);
    el.historyToggle.addEventListener("click", () => setSetting("historyOn", !settings.historyOn));
    el.historyClear.addEventListener("click", clearHistory);
    bindHomeAndSettings();
    $("#session-cancel").addEventListener("click", () => el.sessionDialog.close());
    $("#new-session-form").addEventListener("submit", submitSession);
    [el.sessionDialog, el.confirmDialog, el.promptDialog].forEach((dialog) =>
      dialog.addEventListener("mousedown", (e) => { if (e.target === dialog) dialog.close(); }));

    window.addEventListener("keydown", onDictationKeyDown, true);
    window.addEventListener("keyup", onDictationKeyUp, true);
    window.addEventListener("dragover", onWindowDragOver, true);
    window.addEventListener("drop", onWindowDrop, true);
    window.addEventListener("paste", onWindowPaste, true);
    // Losing focus means the key release will never arrive, so end the hold now.
    window.addEventListener("blur", () => cancelDictation());
    // A hold that is listening to another app is not Nesa's to end when Nesa is hidden.
    document.addEventListener("visibilitychange", () => { if (document.hidden && !dictation.elsewhere) cancelDictation(); });
    window.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("focus", clearSeenDone);
    // Coming back to Nesa is when a permission granted in System Settings can take effect.
    window.addEventListener("focus", watchKeys);
    window.addEventListener("contextmenu", (e) => { if (!e.target.closest(".group-row, .input, .group-rename, .xterm")) e.preventDefault(); });
  }

  async function bindBackendEvents() {
    await Promise.all([
      listen("pty-data", ({ payload }) => writeToCard(payload.id, decodeBase64(payload.data))),
      listen("pty-exit", ({ payload }) => {
        const card = cards.get(payload.id);
        if (card) {
          card.live = false;
          card.term.write("\r\n\x1b[2m[process exited]\x1b[0m\r\n");
        }
        state.done.delete(payload.id);
        setStatus(payload.id, { running: false, exited: true, working: false });
      }),
      listen("activity", ({ payload }) => onActivity(payload)),
      listen("dictation", ({ payload }) => onDictationEvent(payload)),
      listen("store-changed", ({ payload }) => applyStore(payload)),
      listen("quit-requested", ({ payload }) => confirmQuit(payload)),
      listen("browser-page", ({ payload }) => onBrowserPage(payload)),
    ]);
  }

  // ----------------------------------------------------------------- start

  async function start() {
    loadSettings();
    applyColors();
    bindUi();
    await bindBackendEvents();
    // The helper watches the voice-mode chord in every app once it knows which keys they are.
    syncChord();
    watchKeys();
    syncAlerts();
    syncDonePing();
    startUsage();
    // Keeps "Working · 1m 04s" counting up. Only working cards change, so this is cheap.
    setInterval(() => {
      state.workingSince.forEach((_, id) => updateCardStatus(id));
      tickHomeWorking();
    }, 1000);
    setInterval(refreshTokens, TOKEN_REFRESH_MS);
    refreshTokens();

    let loaded = await attempt("load_store");
    if (!loaded.ok) return;
    let store = loaded.value;
    if (!store.groups.length) {
      const created = await attempt("create_group", { name: "Group 1" });
      if (!created.ok) return;
      store = created.value;
    }

    const saved = Number(stored.get("selectedGroup"));
    state.selected = Number.isInteger(saved) && saved >= 0 && saved < store.groups.length ? saved : 0;
    state.resumeGroup = state.selected;
    state.store = store;
    await refreshStatus(allSessions().map((s) => s.id));
    // Home is the landing view. A group's sessions start when that group is opened.
    applyStore(store);
    loadTranscripts();
    refreshImageGen();
  }

  start();
})();
