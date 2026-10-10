// The voice pill over other apps. Nesa sends what the pill shows, the same state its own pill has. The
// level bars come straight from the speech helper, so they move with the voice in this window too.
(() => {
  const { listen } = window.__TAURI__.event;
  const root = document.getElementById("hud-pill");
  const label = document.getElementById("hud-label");
  const meta = document.getElementById("hud-meta");
  const text = document.getElementById("hud-text");
  const words = document.getElementById("hud-words");
  const bars = [...document.querySelectorAll("#hud-pill .dictation-bars i")];
  let target = 0; // last level from the helper
  let level = 0; // smoothed, drives the bars
  let frame = 0;

  // The same rule as the pill in Nesa: a transcript keeps its newest words in view, and the left edge
  // fades only while text is clipped behind it. Messages keep their first words and end with an ellipsis.
  function showWords(value, kind) {
    words.textContent = value;
    text.dataset.kind = kind;
    text.scrollLeft = kind === "tail" ? text.scrollWidth : 0;
    text.toggleAttribute("data-clipped", text.scrollLeft > 0);
  }

  // The bars follow the microphone, with a small wobble per bar. They run only while listening.
  function animate() {
    frame = 0;
    if (root.dataset.mode !== "listening") return;
    target *= 0.92;
    level += (target - level) * 0.3;
    const now = performance.now() / 1000;
    bars.forEach((bar, i) => {
      const weight = [0.5, 0.85, 1, 0.85, 0.5][i];
      const wobble = 0.8 + 0.2 * Math.sin(now * 8 + i * 1.9);
      const height = Math.max(0.16, Math.min(1, Math.sqrt(level) * weight * wobble * 1.3));
      bar.style.transform = `scaleY(${height.toFixed(3)})`;
    });
    frame = requestAnimationFrame(animate);
  }

  function apply(state) {
    root.dataset.mode = state.mode;
    root.classList.toggle("is-visible", state.visible);
    root.classList.toggle("is-warn", state.warn);
    label.textContent = state.label;
    meta.textContent = state.meta;
    showWords(state.words, state.kind);
    if (!state.visible) {
      target = 0;
      level = 0;
    }
    if (!frame) animate();
  }

  listen("hud", ({ payload }) => apply(payload));
  listen("dictation", ({ payload }) => {
    if (payload.event === "level") target = Math.min(1, payload.value);
  });
})();
