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
    done: new Set(), // ids whose work finished while the user was not looking
    newKind: "claude",
  };
  const cards = new Map(); // id -> card entry (DOM, terminal, size bookkeeping)
  const pendingData = new Map(); // id -> bytes that arrived before the card existed
  const PENDING_LIMIT = 512 * 1024;

  const $ = (sel) => document.querySelector(sel);
  const el = {
    groupList: $("#group-list"),
    title: $("#group-title"),
    count: $("#group-count"),
    grid: $("#grid"),
    empty: $("#empty"),
    parking: $("#parking"),
    toasts: $("#toasts"),
    sessionDialog: $("#new-session"),
    confirmDialog: $("#confirm"),
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

  function toast(message, kind = "error") {
    const node = h("div", { class: `toast ${kind}` }, errorText(message));
    el.toasts.append(node);
    setTimeout(() => node.remove(), 4500);
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

  // ---------------------------------------------------------------- status

  function stateOf(id) {
    const st = state.status[id] || {};
    if (st.exited) return "exited";
    if (st.working) return "working";
    if (state.done.has(id)) return "done";
    return "idle";
  }

  const STATE_WORD = { working: "Working", done: "Done", exited: "Exited", idle: "" };

  function setStatus(id, patch) {
    const prev = state.status[id] || { running: false, exited: false, working: false };
    state.status[id] = { ...prev, ...patch };
    updateCardStatus(id);
    updateGroupDots();
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
    if (changed) { cards.forEach((_, id) => updateCardStatus(id)); updateGroupDots(); }
  }

  function onActivity({ id, working }) {
    const wasWorking = !!(state.status[id] && state.status[id].working);
    if (working) state.done.delete(id);
    else if (wasWorking && !(document.hasFocus() && isOnScreen(id))) state.done.add(id);
    setStatus(id, { working, running: true });
  }

  async function refreshStatus(ids) {
    if (!ids.length) return;
    const result = await attempt("session_status", { ids });
    if (!result.ok) return;
    Object.assign(state.status, result.value);
    ids.forEach(updateCardStatus);
    updateGroupDots();
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

  // Returns false for keys xterm must not forward to the shell.
  function handleTermKey(id, e) {
    if (e.metaKey && (e.key === "n" || /^[1-9]$/.test(e.key))) return false;
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
    const restart = h("button", { class: "btn", title: "Restart session", onclick: () => restartSession(session.id) }, icon("restart"), "Restart");
    const expand = h("button", { class: "icon-btn", onclick: () => toggleMaximize(session.id) });
    const more = h("button", { class: "icon-btn", title: "More", "aria-label": "More", onclick: (e) => openCardMenu(session.id, e.currentTarget) }, icon("more"));
    const head = h("header", { class: "card-head", ondblclick: (e) => { if (!e.target.closest("button")) toggleMaximize(session.id); } },
      dot, logo(session.kind), name, meta, status, h("span", { class: "spacer" }), restart, expand, more);
    const body = h("div", { class: "card-body" });
    const root = h("article", { class: "card", "data-id": session.id, onmousedown: () => focusCard(session.id) }, head, body);
    return { root, body, dot, name, meta, status, restart, expand };
  }

  function updateCardInfo(card, session) {
    const maximized = state.maximizedId === session.id;
    card.name.textContent = session.name;
    card.meta.textContent = `${session.kind} · ${shortPath(session.cwd)}`;
    card.meta.title = session.cwd;
    card.expand.replaceChildren(icon(maximized ? "collapse" : "expand"));
    card.expand.title = maximized ? "Restore" : "Expand";
    card.expand.setAttribute("aria-label", card.expand.title);
  }

  function updateCardStatus(id) {
    const card = cards.get(id);
    if (!card) return;
    const st = stateOf(id);
    card.dot.dataset.state = st;
    card.status.dataset.state = st;
    card.status.textContent = STATE_WORD[st];
    card.restart.hidden = st !== "exited";
  }

  function updateGroupDots() {
    [...el.groupList.children].forEach((row, i) => {
      const states = (groups()[i]?.sessions || []).map((s) => stateOf(s.id));
      const dot = row.querySelector(".dot");
      if (!dot) return;
      if (states.includes("done")) dot.dataset.state = "done";
      else if (states.includes("working")) dot.dataset.state = "working";
      else delete dot.dataset.state;
    });
  }

  function focusCard(id, { focusTerminal = true } = {}) {
    state.focusedId = id;
    cards.forEach((card) => card.root.classList.toggle("focused", card.id === id));
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

  async function stopSession(id) {
    const result = await attempt("stop_session", { id });
    if (result.ok) setStatus(id, { running: false, exited: true, working: false });
  }

  async function removeSession(id) {
    const result = await attempt("remove_session", { id });
    if (result.ok) applyStore(result.value);
  }

  function openCardMenu(id, anchor) {
    const rect = anchor.getBoundingClientRect();
    const exited = stateOf(id) === "exited";
    showMenu(rect.right, rect.bottom + 4, [
      { label: "Stop", disabled: exited, onSelect: () => stopSession(id) },
      { label: "Remove from group", danger: true, onSelect: () => removeSession(id) },
    ], { alignRight: true });
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
      return h("div", {
        class: `group-row${index === state.selected ? " selected" : ""}`,
        role: "button",
        onclick: () => selectGroup(index),
        ondblclick: () => startRename(index),
        oncontextmenu: (e) => { e.preventDefault(); openGroupMenu(e, index); },
      }, name, h("span", { class: "dot" }));
    }));

    const field = el.groupList.querySelector(".group-rename");
    if (field) {
      field.focus();
      if (state.renameFresh) { field.select(); state.renameFresh = false; }
      else field.setSelectionRange(field.value.length, field.value.length);
    }
    updateGroupDots();
  }

  function renderToolbar() {
    const group = currentGroup();
    const n = group ? group.sessions.length : 0;
    el.title.textContent = group ? group.name : "";
    el.count.textContent = n ? `${n} ${n === 1 ? "session" : "sessions"}` : "";
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
    const entries = shown.map((session, i) => {
      const card = cards.get(session.id) || createCard(session);
      updateCardInfo(card, session);
      updateCardStatus(session.id);
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
      ? `Its ${count} ${count === 1 ? "session" : "sessions"} will be stopped and removed from the deck.`
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

  function closeMenu() {
    if (closeActiveMenu) closeActiveMenu();
  }

  function showMenu(x, y, items, { alignRight = false } = {}) {
    closeMenu();
    const menu = h("div", { class: "menu", role: "menu" }, items.map((item) =>
      h("button", {
        class: `menu-item${item.danger ? " danger" : ""}`,
        role: "menuitem",
        disabled: item.disabled,
        onclick: () => { closeMenu(); item.onSelect(); },
      }, item.label)));
    document.body.append(menu);

    const { offsetWidth: w, offsetHeight: ht } = menu;
    const left = Math.max(8, Math.min(alignRight ? x - w : x, window.innerWidth - w - 8));
    const top = Math.max(8, Math.min(y, window.innerHeight - ht - 8));
    menu.style.left = `${left}px`;
    menu.style.top = `${top}px`;

    const onPointerDown = (e) => { if (!menu.contains(e.target)) closeMenu(); };
    document.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("blur", closeMenu);
    closeActiveMenu = () => {
      document.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("blur", closeMenu);
      menu.remove();
      closeActiveMenu = null;
    };
  }

  function confirmDialog({ title, text, label }) {
    const dialog = el.confirmDialog;
    $("#confirm-title").textContent = title;
    $("#confirm-text").textContent = text;
    $("#confirm-ok").textContent = label;
    return new Promise((resolve) => {
      let answer = false;
      const finish = () => { dialog.removeEventListener("close", finish); resolve(answer); };
      dialog.addEventListener("close", finish);
      $("#confirm-form").onsubmit = (e) => { e.preventDefault(); answer = true; dialog.close(); };
      $("#confirm-cancel").onclick = () => dialog.close();
      dialog.showModal();
    });
  }

  function setKind(kind) {
    state.newKind = kind;
    document.querySelectorAll(".segmented button").forEach((btn) =>
      btn.setAttribute("aria-checked", String(btn.dataset.kind === kind)));
  }

  function openSessionDialog(kind = state.newKind) {
    if (!currentGroup() || el.sessionDialog.open || el.confirmDialog.open) return;
    closeMenu();
    setKind(kind);
    $("#session-name").value = "";
    $("#session-cwd").value = stored.get("lastCwd") || "";
    $("#session-error").hidden = true;
    $("#session-start").disabled = false;
    el.sessionDialog.showModal();
    ($("#session-cwd").value ? $("#session-name") : $("#session-cwd")).focus();
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
      stored.set("lastCwd", cwd);
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

  // ---------------------------------------------------------------- events

  function onKeyDown(e) {
    const typing = document.activeElement && document.activeElement.classList.contains("xterm-helper-textarea");
    if (e.key === "Escape" && (closeActiveMenu || !typing)) {
      if (closeActiveMenu) e.preventDefault();
      closeMenu();
      return;
    }
    if (!e.metaKey || e.ctrlKey || e.altKey) return;
    closeMenu();
    if (el.sessionDialog.open || el.confirmDialog.open) return;
    if (e.key === "n") {
      e.preventDefault();
      openSessionDialog();
    } else if (/^[1-9]$/.test(e.key)) {
      e.preventDefault();
      selectGroup(Number(e.key) - 1);
    }
  }

  function bindUi() {
    document.querySelectorAll("[data-logo]").forEach((node) => { node.innerHTML = LOGOS[node.dataset.logo]; });
    document.querySelectorAll("[data-icon]").forEach((node) => { node.innerHTML = ICONS[node.dataset.icon]; });

    $("#new-group").addEventListener("click", createGroup);
    $("#new-claude").addEventListener("click", () => openSessionDialog("claude"));
    $("#new-codex").addEventListener("click", () => openSessionDialog("codex"));

    document.querySelectorAll(".segmented button").forEach((btn) =>
      btn.addEventListener("click", () => setKind(btn.dataset.kind)));
    $("#browse").addEventListener("click", browseFolder);
    $("#session-cancel").addEventListener("click", () => el.sessionDialog.close());
    $("#new-session-form").addEventListener("submit", submitSession);
    [el.sessionDialog, el.confirmDialog].forEach((dialog) =>
      dialog.addEventListener("mousedown", (e) => { if (e.target === dialog) dialog.close(); }));

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
      listen("store-changed", ({ payload }) => applyStore(payload)),
    ]);
  }

  // ----------------------------------------------------------------- start

  async function start() {
    bindUi();
    await bindBackendEvents();

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
