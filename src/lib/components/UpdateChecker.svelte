<script lang="ts">
	/**
	 * UpdateChecker
	 *
	 * Tauri-only auto-updater renderer. Subscribes to the shared `updater`
	 * store and shows:
	 *   - a non-blocking bottom-right banner when an update is available,
	 *   - a modal with progress while the download runs,
	 *   - an error modal if anything fails.
	 *
	 * The actual check + install logic lives in `stores/updater.svelte.ts`
	 * so the About dialog can drive the same state machine from a manual
	 * "Check for updates" button.
	 *
	 * In the web build (`__TAURI_INTERNALS__` absent) the auto-check is a
	 * no-op and the store stays in `idle`, so this whole component
	 * renders nothing.
	 */
	import { onMount } from 'svelte';
	import Modal from './Modal.svelte';
	import { updater } from '$lib/stores/updater.svelte';

	const progressPct = $derived.by(() => {
		const s = updater.state;
		if (s.kind !== 'installing') return null;
		if (!s.total || s.total <= 0) return null;
		return Math.min(100, Math.round((s.downloaded / s.total) * 100));
	});

	onMount(() => {
		// Wait a beat so we don't compete with first paint or the
		// IndexedDB seed for cycles. 3s is enough for the editor to be
		// fully rendered and interactive.
		const t = setTimeout(() => updater.check(true), 3000);
		return () => clearTimeout(t);
	});
</script>

{#if updater.state.kind === 'available'}
	<aside class="banner" role="status" aria-live="polite">
		<div class="content">
			<strong>Update available</strong>
			<span class="versions">v{updater.state.version}</span>
			{#if updater.state.notes}
				<p class="notes">{updater.state.notes}</p>
			{/if}
		</div>
		<div class="actions">
			<button type="button" class="btn ghost" onclick={() => updater.dismiss()}>Later</button>
			<button type="button" class="btn primary" onclick={() => updater.install()}>Install</button>
		</div>
	</aside>
{/if}

<Modal
	open={updater.state.kind === 'installing' || updater.state.kind === 'restarting'}
	title={updater.state.kind === 'restarting' ? 'Restarting…' : 'Installing update'}
	onClose={() => {
		/* noop while installing — user can't cancel mid-flight */
	}}
	dismissOnBackdrop={false}
>
	{#snippet body()}
		{#if updater.state.kind === 'installing'}
			<p>
				Downloading <strong>v{updater.state.version}</strong>… The app will restart automatically when
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
				{(updater.state.downloaded / 1024 / 1024).toFixed(1)} MB
				{updater.state.total
					? `/ ${(updater.state.total / 1024 / 1024).toFixed(1)} MB`
					: 'downloaded'}
			</p>
		{:else if updater.state.kind === 'restarting'}
			<p>Update v{updater.state.version} installed. Relaunching timbrapp…</p>
		{/if}
	{/snippet}
</Modal>

<Modal
	open={updater.state.kind === 'error'}
	title="Update failed"
	onClose={() => updater.close()}
>
	{#snippet body()}
		{#if updater.state.kind === 'error'}
			<p>The update couldn't be installed:</p>
			<pre class="err">{updater.state.message}</pre>
			<p class="subtle">
				You can keep using this version. Try again from the About dialog, or download
				the latest installer from the
				<a
					href="https://github.com/enricozanardo/timbrapp/releases/latest"
					target="_blank"
					rel="noopener"
				>
					releases page
				</a>.
			</p>
		{/if}
	{/snippet}
	{#snippet footer()}
		<button type="button" class="btn primary" onclick={() => updater.close()}>Close</button>
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
