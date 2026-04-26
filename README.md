# timbrapp

Apply PNG stamps and signatures to PDF documents — entirely on your machine.

A small client-side SvelteKit app: load a PDF, drop reusable stamps from your
library onto any page, drag/resize them to taste, and download the stamped
PDF. Nothing leaves your device — there's no backend.

Ships as both a **web app** and a **native desktop app** (Windows, macOS,
Linux) packaged with [Tauri 2](https://v2.tauri.app/).

## Quick start

```bash
npm install
npm run dev
```

Open [http://localhost:5173](http://localhost:5173). On first launch the
default `Libera Università Gorgia` stamp from `static/stamps/` is seeded into
your browser's IndexedDB so you have something to play with.

## How to use

1. Pick (or drop) a PDF in the dropzone, or click **Choose PDF file**.
2. Click a stamp in the left **Stamps** panel to select it (cursor turns into a
   crosshair).
3. Click anywhere on a page to drop the stamp at that position.
4. Drag the stamp to move it; drag a corner handle to resize it; click the
   small **×** to remove it. Press `Delete` while a stamp is selected to remove
   it. Use the toolbar's `−` / `+` to zoom.
5. Click **Download stamped PDF** to save the result.

### Managing your stamp library

- **+ Add PNG** uploads a transparent PNG into your library (stored in
  IndexedDB). It will be available next time you open the app.
- The **×** next to a stamp deletes it. Any placements still on the current PDF
  that referenced that stamp are removed too.

## Architecture

| Concern | Library |
|---|---|
| PDF rendering (canvas) | [`pdfjs-dist`](https://github.com/mozilla/pdf.js) |
| PDF mutation/save | [`pdf-lib`](https://pdf-lib.js.org/) |
| Stamp library storage | IndexedDB via [`idb`](https://github.com/jakearchibald/idb) |
| UI | SvelteKit (Svelte 5 runes) + TypeScript |

```
src/
├── lib/
│   ├── components/        # PdfViewer, StampLibrary, PlacedStamp, Toolbar, ...
│   ├── db/                # IndexedDB stamp store + first-run seed
│   ├── pdf/               # render.ts (pdfjs), save.ts (pdf-lib), coords.ts
│   ├── stamps/            # ObjectURL cache helpers
│   ├── stores/editor.svelte.ts  # Reactive editor singleton (runes)
│   └── types.ts
├── routes/
│   ├── +layout.ts         # ssr: false, prerender: true (SPA)
│   ├── +layout.svelte
│   └── +page.svelte
└── app.css
```

Coordinates are stored in **PDF point space** (origin bottom-left, the same
space `pdf-lib` uses), so the save step is a direct `drawImage` per
placement. The `lib/pdf/coords.ts` module converts to and from screen space
(CSS pixels, origin top-left) for the overlay UI.

## Testing

```bash
npm run check                # type-check (svelte-check)
node scripts/test-save.mjs   # end-to-end save pipeline test
```

`scripts/generate-sample-pdf.mjs` produces `static/sample.pdf` (a 3-page
document) which is handy when manually testing the editor.

## Build

### Web app

```bash
npm run build      # output in build/
npm run preview    # serve the production build
```

The app uses `@sveltejs/adapter-static` and renders as a fully client-side
SPA, so the `build/` directory can be served from any static host.

### Desktop app (Tauri)

The desktop bundle wraps the same SvelteKit build in a small Rust shell
([`src-tauri/`](src-tauri/)) that embeds the OS's native webview — WebView2 on
Windows, WKWebView on macOS, WebKitGTK on Linux. Resulting installers are
~5–15 MB and run fully offline.

#### Prerequisites

| OS      | Required tooling |
|---------|-------------------|
| All     | [Rust stable](https://www.rust-lang.org/tools/install) (1.77+), Node.js 20+ |
| Linux   | `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev libxdo-dev libssl-dev libgtk-3-dev patchelf` (Debian/Ubuntu) |
| macOS   | Xcode Command Line Tools (`xcode-select --install`) |
| Windows | [Microsoft Edge WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (already present on Windows 10 21H2+ / Windows 11) |

On Ubuntu/Debian:

```bash
sudo apt-get update
sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev \
  librsvg2-dev libxdo-dev libssl-dev libgtk-3-dev patchelf
```

#### Run as a native window (development)

```bash
npm run tauri:dev
```

This starts Vite, then opens timbrapp in a native window with hot-reload. The
SvelteKit dev server runs on port 5174.

#### Build platform-specific installers locally

```bash
npm run tauri:build
```

The output lands in `src-tauri/target/release/bundle/`:

- **Linux** → `bundle/deb/*.deb`, `bundle/rpm/*.rpm`, `bundle/appimage/*.AppImage`
- **macOS** → `bundle/dmg/*.dmg`, `bundle/macos/*.app`
- **Windows** → `bundle/msi/*.msi`, `bundle/nsis/*-setup.exe`

Each platform can only build for itself locally; for cross-platform releases
use the GitHub Actions workflow described below.

## Releasing across all platforms (GitHub Actions)

Pushing a version tag triggers
[`.github/workflows/release.yml`](.github/workflows/release.yml), which builds
on `macos-latest` (both Apple Silicon and Intel), `ubuntu-22.04`, and
`windows-latest`, then attaches the resulting installers to a draft GitHub
Release for that tag.

```bash
# Bump version in package.json + src-tauri/tauri.conf.json + src-tauri/Cargo.toml,
# commit, then:
git tag v0.1.0
git push origin v0.1.0
```

Watch the workflow in the **Actions** tab; once green, the **Releases** page
will show a draft release with all installers attached. Edit the notes and
publish.

[`.github/workflows/ci.yml`](.github/workflows/ci.yml) runs on every push/PR
to `main` to verify the web build typechecks and the Tauri shell still
`cargo check`s.

### Code signing (optional)

The unsigned macOS `.dmg` and Windows `.msi`/`.exe` will trigger SmartScreen
or Gatekeeper warnings — fine for personal/internal distribution, awkward for
public releases. To sign:

- **macOS**: set `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`,
  `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_TEAM_ID`,
  `APPLE_PASSWORD` repository secrets (the
  [tauri-action docs](https://github.com/tauri-apps/tauri-action#code-signing)
  list what each one does).
- **Windows**: set `WINDOWS_CERTIFICATE` (base64-encoded `.pfx`) and
  `WINDOWS_CERTIFICATE_PASSWORD`.

The release workflow already wires `GITHUB_TOKEN`; signing secrets are
picked up automatically by `tauri-action` when present.
