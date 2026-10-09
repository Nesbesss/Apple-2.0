# Agent rules for Session Deck

Session Deck is a Tauri app. The Rust backend is in `src-tauri/`, and the frontend is static HTML/CSS/JS in `ui/`.

## Workflow
- Do not edit `/Users/nesbes/nesa` directly. It is the integration folder (`main`).
- Work in your own worktree and branch:
  `git -C /Users/nesbes/nesa worktree add /Users/nesbes/nesa-wt/<name> -b <name>`
- Share the Rust build cache: `export CARGO_TARGET_DIR=/Users/nesbes/.cache/sessiondeck-target`
- Install JS deps in your worktree with `npm install`, or symlink `node_modules` from `/Users/nesbes/nesa`.
- Commit on your branch. Merge into `main` only from `/Users/nesbes/nesa`, one agent at a time, with a clean `git status`.

## Before you finish
- `cargo build` in `src-tauri/` must finish with zero warnings.
- `node --check ui/app.js` must pass.
- `cargo test` in `src-tauri/` must pass.
- Report what you verified and what you did not.

## Do not
- Run `tauri dev` in `/Users/nesbes/nesa`, or stop, restart, or kill the dev app running there.
- Kill processes you did not start.
- Edit the live data file `~/Library/Application Support/SessionDeck/groups.json`.
- Open a visible browser (Safari, Chrome) on the desktop for testing. Use a headless browser.
- Edit files another agent owns without coordinating first.
- Commit build output, `node_modules/`, or secrets.
