/**
 * Updater store — auto-update intentionally disabled.
 *
 * The Tauri auto-updater was removed because, on Linux, the "Install"
 * flow downloads and re-extracts the AppImage which overwrites the
 * desktop entry and reverts to the bundled (pre-fix) binary.  Users
 * should update by rebuilding from source or downloading the AppImage
 * manually from the releases page.
 */
import { APP_VERSION } from '$lib/version';

export type UpdaterCheckState = { kind: 'idle' };

export function isTauri(): boolean {
	return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

class UpdaterState {
	state = $state<UpdaterCheckState>({ kind: 'idle' });
	readonly currentVersion = APP_VERSION;
}

export const updater = new UpdaterState();
