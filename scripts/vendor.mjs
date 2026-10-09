// Copies the browser builds of xterm.js into ui/vendor so the frontend needs no bundler.
import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const out = join(root, "ui", "vendor");
mkdirSync(out, { recursive: true });

const files = [
  ["node_modules/@xterm/xterm/lib/xterm.js", "xterm.js"],
  ["node_modules/@xterm/xterm/css/xterm.css", "xterm.css"],
  ["node_modules/@xterm/addon-fit/lib/addon-fit.js", "addon-fit.js"],
];
for (const [from, to] of files) {
  copyFileSync(join(root, from), join(out, to));
  console.log(`vendored ${to}`);
}
