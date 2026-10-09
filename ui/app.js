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
    pencil: stroke('<path d="M10.25 3.25 12.75 5.75 6 12.5H3.5v-2.5z"/>'),
    folder: stroke('<path d="M2.5 4.5h3.25l1.5 1.5h6.25v6.5h-11z"/>'),
    copy: stroke('<rect x="5.5" y="5.5" width="8" height="8" rx="1.5"/><path d="M3.5 10.5v-6a1 1 0 0 1 1-1h6"/>'),
    stop: '<svg viewBox="0 0 16 16" fill="currentColor"><rect x="4" y="4" width="8" height="8" rx="1.5"/></svg>',
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
    newKind: "claude",
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
    recent: $("#recent"),
    sessionDialog: $("#new-session"),
    confirmDialog: $("#confirm"),
    promptDialog: $("#prompt"),
  };

  const groups = () => state.store.groups;
  const currentGroup = () => groups()[state.selected];
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
  const icon = (name) => h("span", { class: "icon", html: ICONS[name] });
  const errorText = (err) => (typeof err === "string" ? err : (err && err.message) || String(err));

  // Restarts a one-shot CSS animation by re-adding its class (the reflow makes the browser notice).
  function replayClass(node, name) {
    node.classList.remove(name);
    void node.offsetWidth;
    node.classList.add(name);
  }

  function toast(message, kind = "error") {
    const node = h("div", { class: `toast ${kind}` }, errorText(message));
    el.toasts.append(node);
    setTimeout(() => {
      node.classList.add("leaving");
      setTimeout(() => node.remove(), 200); // matches the toast-out animation
    }, 4300);
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
    const group = currentGroup();
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
        selectionBackground: "rgba(124, 156, 255, 0.28)",
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
    if (e.metaKey && (e.key === "n" || e.key === "[" || e.key === "]" || e.key === "j" || e.key === "Enter" || /^[1-9]$/.test(e.key))) return false;
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

    const ui = buildCardDom(session);
    const card = { id: session.id, term, fit, ...ui, opened: false, live: false, sentSize: null, scroll: null, timer: 0 };
    new ResizeObserver(() => {
      clearTimeout(card.timer);
      card.timer = setTimeout(() => fitCard(card), 50);
    }).observe(card.body);
    cards.set(session.id, card);

    const queued = pendingData.get(session.id);
    if (queued) { queued.forEach((bytes) => term.write(bytes)); pendingData.delete(session.id); }
    return card;
  }

  function buildCardDom(session) {
    const dot = h("span", { class: "dot" });
    const name = h("span", { class: "card-name" });
    const meta = h("span", { class: "card-meta" });
    const status = h("span", { class: "card-status" });
    const restart = h("button", { class: "btn", title: "Restart session", onclick: () => restartWithConfirm(session.id) }, icon("restart"), "Restart");
    const expand = h("button", { class: "icon-btn", onclick: () => toggleMaximize(session.id) });
    const more = h("button", { class: "icon-btn", title: "More", "aria-label": "More", onclick: (e) => openCardMenu(session.id, e.currentTarget) }, icon("more"));
    const head = h("header", { class: "card-head", ondblclick: (e) => { if (!e.target.closest("button")) toggleMaximize(session.id); } },
      dot, logo(session.kind), name, meta, status, h("span", { class: "spacer" }), restart, expand, more);
    const body = h("div", { class: "card-body" });
    // The entrance animation is one-shot: drop its class once it ends, so moving the card later does not replay it.
    const root = h("article", {
      class: "card",
      "data-id": session.id,
      onmousedown: () => focusCard(session.id),
      onanimationend: (e) => { if (e.target === e.currentTarget) e.currentTarget.classList.remove("arrive"); },
    }, head, body);
    return { root, body, dot, name, meta, status, restart, expand };
  }

  function updateCardInfo(card, session) {
    const maximized = state.maximizedId === session.id;
    card.name.textContent = session.name;
    card.meta.textContent = `${session.kind} · ${shortPath(session.cwd)}`;
    card.meta.title = session.cwd;
    card.expand.replaceChildren(icon(maximized ? "collapse" : "expand"));
    card.expand.title = maximized ? "Restore (\u2318\u21A9)" : "Expand (\u2318\u21A9)";
    card.expand.setAttribute("aria-label", card.expand.title);
  }

  function statusText(id, st) {
    const since = state.workingSince.get(id);
    return st === "working" && since ? `${STATE_WORD[st]} · ${formatElapsed(Date.now() - since)}` : STATE_WORD[st];
  }

  function updateCardStatus(id) {
    const card = cards.get(id);
    if (!card) return;
    const st = stateOf(id);
    card.dot.dataset.state = st;
    card.status.dataset.state = st;
    card.status.textContent = statusText(id, st);
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
  }

  function updateSummary() {
    const group = currentGroup();
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

  function renderAll() {
    renderSidebar();
    renderToolbar();
    renderGrid();
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
        class: `group-row${index === state.selected ? " selected" : ""}`,
        role: "button",
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
      }, header, group.sessions.length ? h("div", { class: "session-list" }, group.sessions.map(sessionRow)) : null);
    }));

    const field = el.groupList.querySelector(".group-rename");
    if (field) {
      field.focus();
      if (state.renameFresh) { field.select(); state.renameFresh = false; }
      else field.setSelectionRange(field.value.length, field.value.length);
    }
    updateIndicators();
  }

  function sessionRow(session) {
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
    },
      logo(session.kind),
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
    el.groupList.querySelectorAll(".dragging, .drop-target").forEach((node) => node.classList.remove("dragging", "drop-target"));
  }

  function onGroupDragOver(e, index) {
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
    const id = state.dragId;
    endDrag();
    if (id) moveSession(id, index);
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
    const group = currentGroup();
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
  }

  // ---------------------------------------------------------------- groups

  function selectGroup(index) {
    if (index === state.selected || !groups()[index]) return;
    state.selected = index;
    state.maximizedId = null;
    state.focusedId = null;
    state.renaming = null;
    state.arriving = true;
    stored.set("selectedGroup", String(index));
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
    };
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
      input.focus();
      input.select();
    });
  }

  const anyDialogOpen = () => [el.sessionDialog, el.confirmDialog, el.promptDialog].some((d) => d.open);

  function setKind(kind) {
    state.newKind = kind;
    document.querySelectorAll(".segmented button").forEach((btn) =>
      btn.setAttribute("aria-checked", String(btn.dataset.kind === kind)));
  }

  function openSessionDialog(kind = state.newKind) {
    if (!currentGroup() || anyDialogOpen()) return;
    closeMenu();
    setKind(kind);
    $("#session-name").value = "";
    $("#session-cwd").value = stored.get("lastCwd") || "";
    $("#session-error").hidden = true;
    $("#session-start").disabled = false;
    renderRecent();
    el.sessionDialog.showModal();
    ($("#session-cwd").value ? $("#session-name") : $("#session-cwd")).focus();
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
    const cwd = $("#session-cwd").value.trim();
    if (!cwd) { showSessionError("Choose a folder for the session."); return; }
    const start = $("#session-start");
    start.disabled = true;
    $("#session-error").hidden = true;
    try {
      const session = await invoke("create_session", {
        groupIndex: state.selected,
        kind: state.newKind,
        name: $("#session-name").value.trim(),
        cwd,
      });
      stored.set("lastCwd", session.cwd);
      state.status[session.id] = { running: true, exited: false, working: false };
      state.focusedId = session.id;
      state.maximizedId = null;
      el.sessionDialog.close();
      applyStore(await invoke("load_store"));
      focusCard(session.id);
    } catch (err) {
      showSessionError(errorText(err));
      start.disabled = false;
    }
  }

  // ------------------------------------------------------------- dictation

  // Hold Option to talk; release to type the words into the focused session.
  // The mic opens after a short hold, so a quick Option tap or an Option+key shortcut does nothing.
  const DICTATION_HOLD_DELAY = 180; // ms of hold before the mic opens
  // macOS system sounds, played natively by the backend.
  const SOUNDS = { start: "Tink", stop: "Pop", done: "Glass" };
  const dictation = {
    state: "idle", // "idle" | "held" (recording) | "ignoring" (a shortcut was typed during the chord)
    shown: false, // the HUD is up
    recording: false, // the mic is open
    timer: 0,
    frame: 0,
    level: 0, // smoothed mic level, drives the bars
    target: 0, // last level from the helper
    audio: null,
  };
  const hud = {
    root: $("#dictation"),
    text: $("#dictation-text"),
    bars: [...document.querySelectorAll(".dictation-bars i")],
    hideTimer: 0,
  };

  const isModifierKey = (e) => e.key === "Meta" || e.key === "Alt";
  const chordDown = (e) => e.altKey && !e.metaKey && !e.ctrlKey && !e.shiftKey;
  const tail = (text) => (text.length > 72 ? `…${text.slice(-72)}` : text);

  function playSound(name) {
    invoke("play_sound", { name: SOUNDS[name] }).catch(() => {});
  }

  function setHud(mode, text) {
    hud.root.dataset.mode = mode;
    hud.root.classList.add("is-visible");
    if (text !== undefined) hud.text.textContent = text;
  }

  function hideHud(delay = 0) {
    clearTimeout(hud.hideTimer);
    hud.hideTimer = setTimeout(() => {
      if (dictation.state !== "idle") return; // a new hold has started
      dictation.shown = false;
      dictation.target = 0;
      hud.root.dataset.mode = "idle"; // back to the dim dot, without the last mode's colour
      hud.root.classList.remove("is-visible");
    }, delay);
  }

  // The bars follow the real microphone level, with a small per-bar wobble so they feel alive.
  function animateHud() {
    dictation.frame = 0;
    if (!dictation.shown) return;
    dictation.target *= 0.92; // decays between level events
    dictation.level += (dictation.target - dictation.level) * 0.3;
    const now = performance.now() / 1000;
    hud.bars.forEach((bar, i) => {
      const weight = [0.55, 0.85, 1, 0.8, 0.6][i];
      const wobble = 0.75 + 0.25 * Math.sin(now * 9 + i * 1.7);
      const height = Math.max(0.14, Math.min(1, dictation.level * weight * wobble * 1.6));
      bar.style.transform = `scaleY(${height.toFixed(3)})`;
    });
    dictation.frame = requestAnimationFrame(animateHud);
  }

  function beginDictation() {
    // Ignore the chord while typing in a text field that is not a terminal (rename, dialogs).
    const active = document.activeElement;
    if (active && active.matches("input, textarea") && !active.classList.contains("xterm-helper-textarea")) return;
    dictation.state = "held";
    dictation.timer = setTimeout(startListening, DICTATION_HOLD_DELAY);
  }

  function startListening() {
    if (dictation.state !== "held") return;
    dictation.recording = true;
    dictation.shown = true;
    setHud("listening", "Listening…");
    playSound("start");
    if (!dictation.frame) animateHud();
    attempt("dictation_start").then((result) => {
      if (!result.ok) cancelDictation();
    });
  }

  function releaseDictation() {
    clearTimeout(dictation.timer);
    dictation.state = "idle";
    dictation.target = 0;
    if (!dictation.recording) return; // released before the mic opened, so there is nothing to stop
    dictation.recording = false;
    if (dictation.shown) {
      setHud("processing", hud.text.textContent);
      playSound("stop");
    }
    attempt("dictation_stop");
  }

  function cancelDictation(nextState = "idle") {
    const wasRecording = dictation.recording;
    if (dictation.state === "idle" && !wasRecording) return;
    clearTimeout(dictation.timer);
    dictation.state = nextState;
    dictation.recording = false;
    dictation.shown = false;
    hideHud();
    if (wasRecording) attempt("dictation_cancel");
  }

  function focusedLiveCard() {
    const card = state.focusedId ? cards.get(state.focusedId) : null;
    return card && card.opened && card.live ? card : null;
  }

  function finishDictation(text) {
    const spoken = text.trim();
    if (!spoken) {
      hideHud();
      return;
    }
    const card = focusedLiveCard();
    if (card) {
      // paste() honours bracketed paste, so newlines do not submit the prompt early.
      card.term.paste(spoken);
      setHud("done", tail(spoken));
      playSound("done");
      hideHud(1200);
    } else {
      // Nothing to type into: leave the words on screen so they are not lost.
      setHud("error", `${tail(spoken)}  ·  click a running session first`);
      hideHud(4000);
    }
  }

  function onDictationEvent(event) {
    switch (event.event) {
      case "level":
        dictation.target = Math.min(1, event.value);
        break;
      case "partial":
        if (dictation.shown) hud.text.textContent = tail(event.text);
        break;
      case "final":
        finishDictation(event.text || "");
        break;
      case "state":
        if (event.state === "downloading" && dictation.shown) setHud("listening", "Downloading the speech model…");
        break;
      case "error":
        toast(event.message);
        dictation.target = 0;
        if (dictation.shown) setHud("error", event.message);
        if (dictation.state === "idle") hideHud(2500);
        break;
      case "exited":
        toast("Dictation stopped. It restarts on the next hold.");
        if (dictation.state === "idle") hideHud();
        break;
    }
  }

  // Option is the only chord. Keys typed while it is held are ordinary Option+key input, so they
  // reach the terminal, and dictation steps aside.
  function onDictationKeyDown(e) {
    if (dictation.state === "idle") {
      if (!chordDown(e) || anyDialogOpen()) return;
      beginDictation();
      return;
    }
    if (dictation.state === "held" && !isModifierKey(e)) cancelDictation("ignoring");
  }

  function onDictationKeyUp(e) {
    if (dictation.state === "idle" || e.altKey) return; // another key was released while Option is still down
    releaseDictation();
  }

  // ------------------------------------------------------- alerts and quitting

  const alertsOn = () => stored.get("alerts") !== "off";

  function syncAlerts() {
    const on = alertsOn();
    const button = $("#alerts-toggle");
    button.textContent = `Done alerts: ${on ? "On" : "Off"}`;
    button.setAttribute("aria-pressed", String(on));
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
    $("#empty-claude").addEventListener("click", () => openSessionDialog("claude"));
    $("#empty-codex").addEventListener("click", () => openSessionDialog("codex"));

    document.querySelectorAll(".segmented button").forEach((btn) =>
      btn.addEventListener("click", () => setKind(btn.dataset.kind)));
    $("#browse").addEventListener("click", browseFolder);
    $("#alerts-toggle").addEventListener("click", toggleAlerts);
    $("#session-cancel").addEventListener("click", () => el.sessionDialog.close());
    $("#new-session-form").addEventListener("submit", submitSession);
    [el.sessionDialog, el.confirmDialog, el.promptDialog].forEach((dialog) =>
      dialog.addEventListener("mousedown", (e) => { if (e.target === dialog) dialog.close(); }));

    window.addEventListener("keydown", onDictationKeyDown, true);
    window.addEventListener("keyup", onDictationKeyUp, true);
    // Losing focus means the key release will never arrive, so end the hold now.
    window.addEventListener("blur", () => cancelDictation());
    document.addEventListener("visibilitychange", () => { if (document.hidden) cancelDictation(); });
    window.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("focus", clearSeenDone);
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
    ]);
  }

  // ----------------------------------------------------------------- start

  async function start() {
    bindUi();
    await bindBackendEvents();
    syncAlerts();
    // Keeps "Working · 1m 04s" counting up. Only working cards change, so this is cheap.
    setInterval(() => state.workingSince.forEach((_, id) => updateCardStatus(id)), 1000);

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
    state.store = store;
    await refreshStatus(allSessions().map((s) => s.id));
    applyStore(store); // opening the visible cards also ensures their sessions run
  }

  start();
})();
