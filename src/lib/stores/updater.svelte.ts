/**
 * Reactive updater state shared between the auto-check banner
 * (`UpdateChecker.svelte`) and the manual "Check for updates" button in
 * the About dialog. Tauri-only — calls into the plugin are gated behind
 * an `isTauri()` runtime check so the web build stays small.
 *
 * Backed by Svelte 5 runes (singleton class) for parity with `editor`.
 */
import { APP_VERSION } from '$lib/version';

export type UpdaterCheckState =
	| { kind: 'idle' }
	/** A manual or auto check is in flight. UI may show a spinner. */
	| { kind: 'checking' }
	/** A check completed and we're already on the latest version. */
	| { kind: 'up-to-date'; checkedAt: number }
	/** Newer release found. The banner + Install button show. */
	| { kind: 'available'; version: string; notes: string | null }
	/** Active install: bytes streaming in. `total` may be null. */
	| { kind: 'installing'; version: string; downloaded: number; total: number | null }
	/** Install finished, app is about to relaunch. */
	| { kind: 'restarting'; version: string }
	/** Last attempt failed; surface the message and let the user retry. */
	| { kind: 'error'; message: string };

const DISMISS_KEY = 'timbrapp.updater.dismissed.version';

function isTauri(): boolean {
	return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

function dismissedVersion(): string | null {
	try {
		return localStorage.getItem(DISMISS_KEY);
	} catch {
		return null;
	}
}

function rememberDismiss(version: string) {
	try {
		localStorage.setItem(DISMISS_KEY, version);
	} catch {
		// localStorage disabled — accept that we'll re-prompt on next launch.
	}
}

class UpdaterState {
	state = $state<UpdaterCheckState>({ kind: 'idle' });
	/** Current running version, surfaced by About so users always see what they have. */
	readonly currentVersion = APP_VERSION;

	/**
	 * Run a check against the configured updater endpoint.
	 *
	 * @param respectDismiss - When true (the auto-check on startup), a
	 *   release the user previously clicked "Later" on stays hidden.
	 *   The manual button passes false so a curious user sees the banner
	 *   even after dismissing it earlier.
	 */
	async check(respectDismiss = true): Promise<void> {
		if (!isTauri()) return;
		// Don't fire concurrent checks.
		if (this.state.kind === 'checking' || this.state.kind === 'installing') return;
		this.state = { kind: 'checking' };
		try {
			const { check } = await import('@tauri-apps/plugin-updater');
			const update = await check();
			if (!update) {
				this.state = { kind: 'up-to-date', checkedAt: Date.now() };
				return;
			}
			if (respectDismiss && dismissedVersion() === update.version) {
				// User asked us not to bother them about this version.
				this.state = { kind: 'idle' };
				return;
			}
			this.state = {
				kind: 'available',
				version: update.version,
				notes: update.body ?? null
			};
		} catch (err) {
			// Network outage, missing manifest, malformed signature — all
			// non-fatal. We surface them but keep the app running.
			console.warn('[updater] check failed:', err);
			this.state = {
				kind: 'error',
				message: err instanceof Error ? err.message : String(err)
			};
		}
	}

	/**
	 * Re-check (server-side) and run the plugin's download → verify → swap →
	 * relaunch flow. Updates state continuously so the modal can render a
	 * live progress bar.
	 */
	async install(): Promise<void> {
		if (this.state.kind !== 'available') return;
		const targetVersion = this.state.version;
		this.state = { kind: 'installing', version: targetVersion, downloaded: 0, total: null };
		try {
			const { check } = await import('@tauri-apps/plugin-updater');
			const { relaunch } = await import('@tauri-apps/plugin-process');
			const update = await check();
			if (!update) {
				this.state = {
					kind: 'error',
					message: 'Update disappeared between check and install.'
				};
				return;
			}
			await update.downloadAndInstall((event) => {
				if (this.state.kind !== 'installing') return;
				if (event.event === 'Started') {
					this.state = { ...this.state, total: event.data.contentLength ?? null };
				} else if (event.event === 'Progress') {
					this.state = {
						...this.state,
						downloaded: this.state.downloaded + event.data.chunkLength
					};
				}
			});
			this.state = { kind: 'restarting', version: targetVersion };
			await relaunch();
		} catch (err) {
			this.state = {
				kind: 'error',
				message: err instanceof Error ? err.message : String(err)
			};
		}
	}

	/** Persist the dismissal so we don't re-prompt for this version. */
	dismiss(): void {
		if (this.state.kind === 'available') {
			rememberDismiss(this.state.version);
		}
		this.state = { kind: 'idle' };
	}

	/** Tear-down used by error/up-to-date toasts. */
	close(): void {
		this.state = { kind: 'idle' };
	}
}

export const updater = new UpdaterState();
export { isTauri };
