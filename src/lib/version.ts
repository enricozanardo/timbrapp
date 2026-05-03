/**
 * App version surfaced in the UI (About dialog, etc.).
 *
 * Bumped together with `package.json`, `src-tauri/tauri.conf.json` and
 * `src-tauri/Cargo.toml` when cutting a release. Vite inlines it at build
 * time, so updating it here is enough — no env-var plumbing required.
 */
export const APP_VERSION = '0.1.9';
