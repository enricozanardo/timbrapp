<script lang="ts">
	/**
	 * UpdateChecker
	 *
	 * Tauri-only auto-updater for the desktop builds. Hits the GitHub
	 * Releases-hosted `latest.json` on startup, and if a newer version is
	 * available shows a non-blocking banner at the bottom-right with
	 * "Install" and "Later" actions. Pressing Install runs the
	 * download-verify-replace flow, then relaunches the app.
	 *
	 * The component is a no-op in the web build: when the runtime isn't
	 * Tauri (no `__TAURI_INTERNALS__` global) we skip the check entirely
	 * and never import the plugins, so the resulting JS chunk is small.
	 *
	 * Per-version dismissal is persisted in localStorage so users who
	 * click "Later" aren't nagged for the same release.
	 */
	import { onMount } from 'svelte';
	import Modal from './Modal.svelte';

	type CheckState =
		| { kind: 'idle' }
		| { kind: 'available'; version: string; notes: string | null }
		| {
				kind: 'installing';
				version: string;
				downloaded: number;
				total: number | null;
		  }
		| { kind: 'restarting'; version: string }
		| { kind: 'error'; message: string };

	let state = $state<CheckState>({ kind: 'idle' });

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
			// localStorage disabled — silently accept that we'll re-prompt next launch.
		}
	}

	async function checkForUpdate() {
		if (!isTauri()) return;
		try {
			const { check } = await import('@tauri-apps/plugin-updater');
			const update = await check();
			if (!update) return;
			if (dismissedVersion() === update.version) return;
			state = {
				kind: 'available',
				version: update.version,
				notes: update.body ?? null
			};
		} catch (err) {
			// Network outage, missing manifest, malformed signature — all
			// non-critical. We log and stay silent so the user isn't pestered.
			console.warn('[updater] check failed:', err);
		}
	}

	async function install() {
		if (state.kind !== 'available') return;
		const targetVersion = state.version;
		state = { kind: 'installing', version: targetVersion, downloaded: 0, total: null };
		try {
			const { check } = await import('@tauri-apps/plugin-updater');
			const { relaunch } = await import('@tauri-apps/plugin-process');
			const update = await check();
			if (!update) {
				state = { kind: 'error', message: 'Update disappeared between check and install.' };
				return;
			}
			await update.downloadAndInstall((event) => {
				if (state.kind !== 'installing') return;
				if (event.event === 'Started') {
					state = { ...state, total: event.data.contentLength ?? null };
				} else if (event.event === 'Progress') {
					state = { ...state, downloaded: state.downloaded + event.data.chunkLength };
				}
			});
			state = { kind: 'restarting', version: targetVersion };
			await relaunch();
		} catch (err) {
			state = {
				kind: 'error',
				message: err instanceof Error ? err.message : String(err)
			};
		}
	}

	function later() {
		if (state.kind === 'available') {
			rememberDismiss(state.version);
		}
		state = { kind: 'idle' };
	}

	function closeError() {
		state = { kind: 'idle' };
	}

	const progressPct = $derived.by(() => {
		if (state.kind !== 'installing') return null;
		if (!state.total || state.total <= 0) return null;
		return Math.min(100, Math.round((state.downloaded / state.total) * 100));
	});

	onMount(() => {
		// Wait a beat so we don't compete with first paint or the
		// IndexedDB seed for cycles. 3s is enough for the editor to be
		// fully rendered and interactive.
		const t = setTimeout(checkForUpdate, 3000);
		return () => clearTimeout(t);
	});
</script>

{#if state.kind === 'available'}
	<aside class="banner" role="status" aria-live="polite">
		<div class="content">
			<strong>Update available</strong>
			<span class="versions">v{state.version}</span>
			{#if state.notes}
				<p class="notes">{state.notes}</p>
			{/if}
		</div>
		<div class="actions">
			<button type="button" class="btn ghost" onclick={later}>Later</button>
			<button type="button" class="btn primary" onclick={install}>Install</button>
		</div>
	</aside>
{/if}

<Modal
	open={state.kind === 'installing' || state.kind === 'restarting'}
	title={state.kind === 'restarting' ? 'Restarting…' : 'Installing update'}
	onClose={() => {
		/* noop while installing — user can't cancel mid-flight */
	}}
	dismissOnBackdrop={false}
>
	{#snippet body()}
		{#if state.kind === 'installing'}
			<p>
				Downloading <strong>v{state.version}</strong>… The app will restart automatically when
				the install finishes.
			</p>
			<div class="bar" aria-label="Download progress">
				<div
					class="fill"
					style="width: {progressPct !== null ? progressPct + '%' : '40%'}; {progressPct ===
					null
						? 'animation: indeterminate 1.4s ease-in-out infinite;'
						: ''}"
				></div>
			</div>
			<p class="bytes">
				{(state.downloaded / 1024 / 1024).toFixed(1)} MB
				{state.total ? `/ ${(state.total / 1024 / 1024).toFixed(1)} MB` : 'downloaded'}
			</p>
		{:else if state.kind === 'restarting'}
			<p>Update v{state.version} installed. Relaunching timbrapp…</p>
		{/if}
	{/snippet}
</Modal>

<Modal
	open={state.kind === 'error'}
	title="Update failed"
	onClose={closeError}
>
	{#snippet body()}
		{#if state.kind === 'error'}
			<p>The update couldn't be installed:</p>
			<pre class="err">{state.message}</pre>
			<p class="subtle">
				You can keep using this version. Try again from the next app launch, or
				download the latest installer from the
				<a href="https://github.com/enricozanardo/timbrapp/releases/latest" target="_blank" rel="noopener">
					releases page
				</a>.
			</p>
		{/if}
	{/snippet}
	{#snippet footer()}
		<button type="button" class="btn primary" onclick={closeError}>Close</button>
	{/snippet}
</Modal>

<style>
	.banner {
		position: fixed;
		bottom: 1rem;
		right: 1rem;
		max-width: 380px;
		background: white;
		border: 1px solid #e2e8f0;
		border-radius: 10px;
		box-shadow: 0 12px 30px rgba(15, 23, 42, 0.18);
		padding: 0.9rem 1rem;
		z-index: 900;
		animation: slide 200ms ease-out;
	}
	.content {
		display: grid;
		gap: 0.25rem;
		margin-bottom: 0.7rem;
	}
	.content strong {
		color: #0f172a;
		font-size: 0.95rem;
	}
	.versions {
		font-size: 0.8rem;
		color: #2563eb;
		font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
	}
	.notes {
		margin: 0.25rem 0 0;
		font-size: 0.8rem;
		color: #64748b;
		max-height: 4.5em;
		overflow: hidden;
		text-overflow: ellipsis;
		display: -webkit-box;
		-webkit-line-clamp: 3;
		line-clamp: 3;
		-webkit-box-orient: vertical;
	}
	.actions {
		display: flex;
		justify-content: flex-end;
		gap: 0.5rem;
	}
	.btn {
		appearance: none;
		border: 1px solid #cbd5e1;
		background: white;
		border-radius: 6px;
		padding: 0.4rem 0.9rem;
		font-size: 0.85rem;
		cursor: pointer;
		color: #1e293b;
	}
	.btn.primary {
		background: #2563eb;
		color: white;
		border-color: transparent;
	}
	.btn.primary:hover {
		background: #1d4ed8;
	}
	.btn.ghost {
		background: transparent;
		color: #64748b;
		border-color: transparent;
	}
	.btn.ghost:hover {
		color: #0f172a;
		background: #f1f5f9;
	}
	.bar {
		height: 8px;
		background: #e2e8f0;
		border-radius: 999px;
		overflow: hidden;
		margin: 0.6rem 0 0.4rem;
	}
	.fill {
		height: 100%;
		background: linear-gradient(90deg, #3b82f6, #2563eb);
		transition: width 200ms ease-out;
	}
	.bytes {
		margin: 0;
		font-size: 0.8rem;
		color: #64748b;
		font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
	}
	.err {
		background: #fef2f2;
		border: 1px solid #fecaca;
		border-radius: 6px;
		padding: 0.5rem 0.7rem;
		font-size: 0.8rem;
		color: #991b1b;
		white-space: pre-wrap;
		word-break: break-word;
	}
	.subtle {
		margin: 0.7rem 0 0;
		font-size: 0.85rem;
		color: #64748b;
	}
	.subtle a {
		color: #2563eb;
		text-decoration: none;
	}
	.subtle a:hover {
		text-decoration: underline;
	}
	@keyframes slide {
		from {
			opacity: 0;
			transform: translateY(12px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}
	@keyframes indeterminate {
		0% {
			transform: translateX(-60%);
		}
		100% {
			transform: translateX(160%);
		}
	}
</style>
