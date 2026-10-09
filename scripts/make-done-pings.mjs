// Makes the sounds an agent plays when it finishes. Every sound is synthesized here from sine and
// triangle tones, so the repo holds no recordings. Run `node scripts/make-done-pings.mjs` to rewrite
// the WAV files in src-tauri/assets/sounds/. The output is the same bytes on every run.
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const RATE = 44100;
const out = join(dirname(fileURLToPath(import.meta.url)), "..", "src-tauri", "assets", "sounds");

// Every sound is set to the same loudness over its first 150 ms, then capped so nothing clips.
const LOUD_SECONDS = 0.15;
const TARGET_RMS = 0.1; // about -20 dBFS
const PEAK_CAP = 0.708; // -3 dBFS
const FADE_OUT_SECONDS = 0.012;

// Adds one tone to `buf`, starting at `at` seconds. It glides from `freq` to `to` over `sweep` seconds.
// `overtones` are [ratio, level, decayScale] triples that ring alongside it and fade faster or slower.
function tone(buf, { at = 0, freq, to = freq, sweep = 0.1, amp = 1, decay = 0.3, attack = 0.004, wave = "sine", overtones = [] }) {
  const start = Math.round(at * RATE);
  let phase = 0;
  for (let i = start; i < buf.length; i++) {
    const t = (i - start) / RATE;
    const f = freq + (to - freq) * Math.min(1, t / sweep);
    phase += (2 * Math.PI * f) / RATE;
    const body = wave === "triangle" ? (2 / Math.PI) * Math.asin(Math.sin(phase)) : Math.sin(phase);
    let sum = body;
    for (const [ratio, level, scale] of overtones) {
      sum += level * Math.sin(ratio * phase) * Math.exp(-t / (decay * scale));
    }
    buf[i] += amp * Math.min(1, t / attack) * Math.exp(-t / decay) * sum;
  }
}

// A short burst of seeded noise, for the click of a knock. The seed keeps the output the same every run.
function click(buf, { at = 0, amp = 0.5, decay = 0.002, length = 0.012, rand }) {
  const start = Math.round(at * RATE);
  for (let i = 0; i < length * RATE && start + i < buf.length; i++) {
    buf[start + i] += amp * (rand() * 2 - 1) * Math.exp(-i / (decay * RATE));
  }
}

// Mulberry32: a small seeded generator that returns numbers in [0, 1).
function seeded(seed) {
  let a = seed;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

// Removes any offset, sets the loudness, caps the peak, and fades the end to exact silence.
function finish(buf) {
  const mean = buf.reduce((sum, x) => sum + x, 0) / buf.length;
  for (let i = 0; i < buf.length; i++) buf[i] -= mean;

  const fade = Math.round(FADE_OUT_SECONDS * RATE);
  for (let i = 0; i < fade; i++) buf[buf.length - 1 - i] *= i / fade;
  buf[buf.length - 1] = 0;

  const loud = buf.slice(0, Math.round(LOUD_SECONDS * RATE));
  const loudRms = Math.sqrt(loud.reduce((sum, x) => sum + x * x, 0) / loud.length);
  const gain = TARGET_RMS / loudRms;
  for (let i = 0; i < buf.length; i++) buf[i] *= gain;

  const peak = Math.max(...buf.map(Math.abs));
  if (peak > PEAK_CAP) for (let i = 0; i < buf.length; i++) buf[i] *= PEAK_CAP / peak;
  return buf;
}

// 16-bit mono PCM WAV.
function wav(samples) {
  const data = Buffer.alloc(44 + samples.length * 2);
  data.write("RIFF", 0);
  data.writeUInt32LE(36 + samples.length * 2, 4);
  data.write("WAVE", 8);
  data.write("fmt ", 12);
  data.writeUInt32LE(16, 16);
  data.writeUInt16LE(1, 20); // PCM
  data.writeUInt16LE(1, 22); // mono
  data.writeUInt32LE(RATE, 24);
  data.writeUInt32LE(RATE * 2, 28);
  data.writeUInt16LE(2, 32);
  data.writeUInt16LE(16, 34);
  data.write("data", 36);
  data.writeUInt32LE(samples.length * 2, 40);
  samples.forEach((x, i) => data.writeInt16LE(Math.round(Math.max(-1, Math.min(1, x)) * 32767), 44 + i * 2));
  return data;
}

// The first entry is the default. Names must not match any macOS system sound.
const SOUNDS = [
  {
    name: "Chime", seconds: 0.9, make(buf) {
      const bell = [[2.0, 0.25, 0.5], [2.76, 0.2, 0.3], [5.4, 0.1, 0.12]];
      tone(buf, { freq: 1046.5, decay: 0.3, overtones: bell });
      tone(buf, { at: 0.12, freq: 1568, amp: 0.8, decay: 0.3, overtones: bell });
    },
  },
  {
    name: "Marimba", seconds: 0.4, make(buf) {
      tone(buf, { freq: 523.25, decay: 0.12, overtones: [[4.0, 0.35, 0.3]] });
    },
  },
  {
    name: "Harp", seconds: 0.7, make(buf) {
      [[784, 0], [987.8, 0.11], [1174.7, 0.22]].forEach(([freq, at]) =>
        tone(buf, { at, freq, amp: 0.8, decay: 0.2, overtones: [[2, 0.3, 0.4]] }));
    },
  },
  {
    name: "Droplet", seconds: 0.35, make(buf) {
      tone(buf, { freq: 1500, to: 450, sweep: 0.09, decay: 0.05 });
      tone(buf, { at: 0.12, freq: 900, to: 1200, sweep: 0.05, amp: 0.5, decay: 0.04 });
    },
  },
  {
    name: "Glow", seconds: 0.9, make(buf) {
      [261.6, 329.6, 392].forEach((freq) => tone(buf, { freq, amp: 0.5, attack: 0.08, decay: 0.3 }));
    },
  },
  {
    name: "Lift", seconds: 0.45, make(buf) {
      tone(buf, { freq: 660, to: 990, sweep: 0.2, amp: 0.8, decay: 0.15, wave: "triangle" });
    },
  },
  {
    name: "Knock", seconds: 0.25, make(buf) {
      tone(buf, { freq: 220, to: 140, sweep: 0.06, decay: 0.05 });
      click(buf, { amp: 0.5, rand: seeded(7) });
    },
  },
  {
    name: "Twinkle", seconds: 0.45, make(buf) {
      [[2093, 0], [2637, 0.06], [3136, 0.12]].forEach(([freq, at]) =>
        tone(buf, { at, freq, amp: 0.5, decay: 0.12, overtones: [[2.0, 0.1, 0.5]] }));
    },
  },
  {
    name: "Pluck", seconds: 0.6, make(buf) {
      tone(buf, { freq: 330, decay: 0.25, overtones: [[2, 0.5, 0.5], [3, 0.3, 0.3], [4, 0.15, 0.2]] });
    },
  },
  {
    name: "Echo", seconds: 0.5, make(buf) {
      tone(buf, { freq: 880, decay: 0.1 });
      tone(buf, { at: 0.22, freq: 880, amp: 0.5, decay: 0.1 });
    },
  },
  {
    name: "Ripple", seconds: 0.6, make(buf) {
      tone(buf, { freq: 1100, to: 700, sweep: 0.25, decay: 0.2, overtones: [[2, 0.2, 0.5]] });
    },
  },
];

mkdirSync(out, { recursive: true });
console.log("name       seconds  peak dBFS  loud RMS dBFS  bytes");
for (const { name, seconds, make } of SOUNDS) {
  const buf = new Float64Array(Math.ceil(seconds * RATE));
  make(buf);
  const samples = finish(buf);
  const peak = Math.max(...samples.map(Math.abs));
  const loud = samples.slice(0, Math.round(LOUD_SECONDS * RATE));
  const loudRms = Math.sqrt(loud.reduce((sum, x) => sum + x * x, 0) / loud.length);
  const bytes = wav(samples);
  writeFileSync(join(out, `${name.toLowerCase()}.wav`), bytes);
  console.log(`${name.padEnd(10)} ${seconds.toFixed(2).padStart(7)}  ${(20 * Math.log10(peak)).toFixed(1).padStart(9)}  ${(20 * Math.log10(loudRms)).toFixed(1).padStart(13)}  ${String(bytes.length).padStart(6)}`);
}
