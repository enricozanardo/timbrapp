# timbrapp

Apply PNG stamps and signatures to PDF documents — entirely on your machine.

A small client-side SvelteKit app: load a PDF, drop reusable stamps from your
library onto any page, drag/resize them to taste, and download the stamped
PDF. Nothing leaves your device — there's no backend.

Ships as both a **web app** and a **native desktop app** (Windows, macOS,
Linux) packaged with [Tauri 2](https://v2.tauri.app/).

## Download & install

Pre-built installers are published on the
[**Releases page**](https://github.com/enricozanardo/timbrapp/releases/latest).

| OS / family | File to download | Install |
|---|---|---|
| **Windows 10/11** (x86\_64) | `timbrapp_<version>_x64-setup.exe` *or* `timbrapp_<version>_x64_en-US.msi` | Double-click; Windows SmartScreen will warn ("Windows protected your PC") because the binary is unsigned — click **More info → Run anyway**. |
| **macOS — Apple Silicon** (M1/M2/M3/M4) | `timbrapp_<version>_aarch64.dmg` | Open the `.dmg`, drag **timbrapp** to *Applications*. The first launch is blocked by Gatekeeper (unsigned); **right-click the app → Open → Open**. |
| **macOS — Intel** | `timbrapp_<version>_x64.dmg` | Same as above. |
| **Debian / Ubuntu / Mint / Pop!\_OS** (x86\_64) | `timbrapp_<version>_amd64.deb` | `sudo apt install ./timbrapp_<version>_amd64.deb` |
| **Fedora / RHEL / openSUSE** (x86\_64) | `timbrapp-<version>-1.x86_64.rpm` | `sudo dnf install ./timbrapp-<version>-1.x86_64.rpm` (or `zypper`) |
| **Arch / Manjaro / Gentoo / NixOS / any other Linux** (x86\_64) | `timbrapp_<version>_amd64.AppImage` | `chmod +x timbrapp_<version>_amd64.AppImage && ./timbrapp_<version>_amd64.AppImage` |

> **Note on macOS / Windows warnings.** The bundled binaries are not yet code
> signed. They are perfectly safe — they're built by GitHub Actions directly
> from this repository — but unsigned binaries trigger Gatekeeper / SmartScreen
> warnings on first launch. The "Run anyway" / "right-click → Open" flow only
> needs to be done once.

### Linux specifics (including Gentoo)

The `.AppImage` is the most portable Linux artifact and runs on essentially
any glibc-based distro shipped after ~2020. It needs three things on the host:

- **WebKitGTK 4.1** (the embedded browser).
- **GTK 3**.
- **FUSE 2** (so the AppImage can mount itself).

Per-distro install of the runtime dependencies:

```bash
# Debian / Ubuntu (also picked up automatically when you install the .deb)
sudo apt install libwebkit2gtk-4.1-0 libgtk-3-0 fuse

# Fedora
sudo dnf install webkit2gtk4.1 gtk3 fuse

# Arch / Manjaro
sudo pacman -S webkit2gtk-4.1 gtk3 fuse2

# Gentoo (Portage)
sudo emerge --ask net-libs/webkit-gtk:4.1 x11-libs/gtk+:3 sys-fs/fuse:0

# NixOS — easiest path is to wrap the AppImage with appimage-run:
nix-shell -p appimage-run --run "appimage-run ./timbrapp_<version>_amd64.AppImage"
```

> On **Gentoo**, make sure `net-libs/webkit-gtk` is built with the
> `introspection` USE flag (default in most profiles). If `./timbrapp.AppImage`
> errors with `dlopen ... libwebkit2gtk-4.1.so.0: cannot open shared object
> file`, that's the package you're missing.

If you'd rather not deal with FUSE at all on a strange distro, you can extract
the AppImage to a folder and run the binary inside it directly:

```bash
./timbrapp_<version>_amd64.AppImage --appimage-extract
./squashfs-root/AppRun
```

## Run from source (developers)

If you want to hack on the app instead of installing a release binary:

```bash
npm install
npm run dev          # web app at http://localhost:5173
# — or —
npm run tauri:dev    # native window with hot-reload (needs Rust + system deps)
```

On first launch the default `Libera Università Gorgia` stamp from
`static/stamps/` is seeded into your browser's IndexedDB so you have something
to play with.

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

## Becoming a "trusted" publisher

This is what's required to ship installers that don't trigger SmartScreen or
Gatekeeper warnings (Windows / macOS), and what's available on Linux. The
GitHub Actions workflow is already wired up to pick up signing secrets when
present — adding signing is a matter of obtaining credentials and pasting
them into repository secrets.

### macOS — Apple Developer ID + notarization

What "trusted" means: the app launches with a single click on any modern
Mac without the right-click → Open dance, and Gatekeeper shows the
publisher name.

Steps:

1. **Enroll in the Apple Developer Program** — $99/year, individual or
   organization. Sign up at <https://developer.apple.com/programs/>.
2. **Create a Developer ID Application certificate** in the
   *Certificates, Identifiers & Profiles* section of the developer portal.
   Download the `.p12` (export from Keychain Access with a password).
3. **Generate an app-specific password** at <https://appleid.apple.com>
   (Sign-In and Security → App-Specific Passwords). This is what
   notarization uses, *not* your Apple ID password.
4. **Add these GitHub repository secrets** (Settings → Secrets and variables
   → Actions):

   | Secret | Value |
   |---|---|
   | `APPLE_CERTIFICATE` | base64 of the `.p12` (`base64 -i Cert.p12 \| pbcopy`) |
   | `APPLE_CERTIFICATE_PASSWORD` | password used when exporting the `.p12` |
   | `APPLE_SIGNING_IDENTITY` | the certificate's common name, e.g. `Developer ID Application: Your Name (TEAMID)` |
   | `APPLE_ID` | your Apple ID email |
   | `APPLE_TEAM_ID` | 10-char Team ID, visible on the developer portal |
   | `APPLE_PASSWORD` | the app-specific password from step 3 |

5. Push a new tag. `tauri-action` detects the secrets, signs the `.app`,
   submits to Apple's notary service, and staples the resulting ticket to
   the `.dmg`. The whole flow adds 2–5 minutes to the CI build.

Reference: [Tauri code-signing docs (macOS)](https://v2.tauri.app/distribute/sign/macos/)
· [tauri-action signing inputs](https://github.com/tauri-apps/tauri-action#code-signing).

### Windows — Code-signing certificate

What "trusted" means: SmartScreen no longer shows a warning on download
and the publisher name appears instead of "unknown publisher" in the UAC
prompt.

There are two flavors of certificate you can buy:

| Type | Cost (≈) | SmartScreen reputation |
|---|---|---|
| **Standard / OV** (Organization Validated) | $100–250 / year | Built up over time as users download and run the binary; new releases are flagged for the first few days/weeks until reputation accumulates |
| **EV** (Extended Validation) | $300–700 / year | Instantly trusted by SmartScreen — no warm-up period |

EV is the only way to get *immediate* SmartScreen trust. Both kinds require
identity verification of you / your company by the issuing CA (DigiCert,
SSL.com, Sectigo, GlobalSign, etc.).

Recent regulatory change (June 2023): Windows code-signing private keys
**must** live on a hardware token (USB HSM / cloud HSM). You can't just
hand a `.pfx` to GitHub Actions any more for new certificates. Two
practical options:

- **Cloud-HSM-backed signing service** (e.g. SSL.com eSigner, DigiCert
  KeyLocker, Azure Trusted Signing). You configure the signer in
  `tauri.conf.json` and pass an API token to CI.
- **Local self-hosted runner** with the USB token plugged in (only viable
  for low release frequency).

For Tauri 2 the relevant `tauri.conf.json` block lives under
`bundle.windows.signCommand` (custom signer) or `bundle.windows.certificateThumbprint`
(traditional `.pfx` flow, only legal for legacy certs). See
[Tauri code-signing docs (Windows)](https://v2.tauri.app/distribute/sign/windows/).

If you go the cloud-HSM route with Azure Trusted Signing (cheapest as of
2026, ~$120/year), the GH Actions secrets you need are roughly:

```
AZURE_CLIENT_ID
AZURE_CLIENT_SECRET
AZURE_TENANT_ID
AZURE_CODE_SIGNING_NAME
```

…and a `signCommand` invoking `azuresigntool.exe` in
`tauri.conf.json#bundle.windows`.

### Linux — there is no central "trusted developer"

Linux has no Gatekeeper / SmartScreen equivalent. Trust is established
distro-by-distro and through reputation. The realistic options, ranked
by reach:

1. **Publish to [Flathub](https://flathub.org/)** — the most universal
   path. One Flatpak manifest, available in GNOME Software / KDE Discover
   / `flatpak install` on virtually every desktop Linux distro,
   sandboxed. Tauri provides a Flatpak target. You go through a one-time
   review when submitting the manifest.
2. **Publish to the [Snap Store](https://snapcraft.io/)** — same idea, run
   by Canonical, default on Ubuntu. Tauri can produce `.snap` bundles via
   `snapcraft`.
3. **Sign your `.deb` / `.rpm` artifacts with GPG** and publish the public
   key on a keyserver and on this repo. `apt-key` / `rpm --import` then
   verify checksums automatically. This is what you'd do for a
   self-hosted apt/yum repo.
4. **Submit packages to specific distros**:
   - Arch: AUR (`PKGBUILD`) or community repo via maintainers.
   - Gentoo: ebuild in [::guru overlay](https://wiki.gentoo.org/wiki/Project:GURU)
     first, then potentially ::gentoo.
   - Debian/Ubuntu: ITP (Intent to Package) → maintainer review → main
     repo. Slow but produces "real" trust (`apt install timbrapp`).
   - Fedora: Fedora Package Sources, similar review process.

For most projects, Flathub is the right answer: one place, one review,
all distros. The downside is the sandbox can complicate IndexedDB / file
access — for timbrapp this isn't an issue (we don't touch the filesystem
outside of save dialogs).

### Quick reference: which secrets unlock signed CI builds

| Platform | Repo secrets required |
|---|---|
| macOS | `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_TEAM_ID`, `APPLE_PASSWORD` |
| Windows (Azure Trusted Signing) | `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_TENANT_ID`, `AZURE_CODE_SIGNING_NAME` + a `signCommand` in `tauri.conf.json` |
| Windows (legacy `.pfx`, valid only for pre-2023 certs) | `WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_PASSWORD` |
| Linux (Flathub) | a Flathub repo + manifest, no GitHub secrets needed |

The release workflow already wires `GITHUB_TOKEN`; the signing secrets
above are picked up automatically by `tauri-action` and the relevant
Tauri bundlers when present.
