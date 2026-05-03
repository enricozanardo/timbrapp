/**
 * Reactive updater state — active on macOS and Windows, disabled on Linux.
 *
 * On Linux the Tauri auto-updater re-extracts the AppImage and overwrites the
 * desktop entry, reverting to the bundled (pre-fix) binary.  Linux users
 * update by rebuilding from source instead.
 */
import { APP_VERSION } from '$lib/version';

export type UpdaterCheckState =
	| { kind: 'idle' }
	| { kind: 'checking' }
	| { kind: 'up-to-date'; checkedAt: number }
	| { kind: 'available'; version: string; notes: string | null }
	| { kind: 'installing'; version: string; downloaded: number; total: number | null }
	| { kind: 'restarting'; version: string }
	| { kind: 'error'; message: string };

export function isTauri(): boolean {
	return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/** True when running on Linux — updater is intentionally disabled here. */
export function isLinux(): boolean {
	return typeof navigator !== 'undefined' &&
		navigator.userAgent.includes('Linux') &&
		!navigator.userAgent.includes('Android');
}

const DISMISS_KEY = 'timbrapp.updater.dismissed.version';
function dismissedVersion(): string | null {
	try { return localStorage.getItem(DISMISS_KEY); } catch { return null; }
}
function rememberDismiss(version: string) {
	try { localStorage.setItem(DISMISS_KEY, version); } catch { /* ignore */ }
}

class UpdaterState {
	state = $state<UpdaterCheckState>({ kind: 'idle' });
	readonly currentVersion = APP_VERSION;

	async check(respectDismiss = true): Promise<void> {
		if (!isTauri() || isLinux()) return;
		if (this.state.kind === 'checking' || this.state.kind === 'installing') return;
		this.state = { kind: 'checking' };
		try {
			const { check } = await import('@tauri-apps/plugin-updater');
			const update = await check();
			if (!update) { this.state = { kind: 'up-to-date', checkedAt: Date.now() }; return; }
			if (respectDismiss && dismissedVersion() === update.version) {
				this.state = { kind: 'idle' }; return;
			}
			this.state = { kind: 'available', version: update.version, notes: update.body ?? null };
		} catch (err) {
			console.warn('[updater] check failed:', err);
			this.state = { kind: 'error', message: err instanceof Error ? err.message : String(err) };
		}
	}

	async install(): Promise<void> {
		if (this.state.kind !== 'available') return;
		const targetVersion = this.state.version;
		this.state = { kind: 'installing', version: targetVersion, downloaded: 0, total: null };
		try {
			const { check } = await import('@tauri-apps/plugin-updater');
			const { relaunch } = await import('@tauri-apps/plugin-process');
			const update = await check();
			if (!update) { this.state = { kind: 'error', message: 'Update disappeared.' }; return; }
			await update.downloadAndInstall((event) => {
				if (this.state.kind !== 'installing') return;
				if (event.event === 'Started') {
					this.state = { ...this.state, total: event.data.contentLength ?? null };
				} else if (event.event === 'Progress') {
					this.state = { ...this.state, downloaded: this.state.downloaded + event.data.chunkLength };
				}
			});
			this.state = { kind: 'restarting', version: targetVersion };
			await relaunch();
		} catch (err) {
			this.state = { kind: 'error', message: err instanceof Error ? err.message : String(err) };
		}
	}

	dismiss(): void {
		if (this.state.kind === 'available') rememberDismiss(this.state.version);
		this.state = { kind: 'idle' };
	}

	close(): void { this.state = { kind: 'idle' }; }
}

export const updater = new UpdaterState();
