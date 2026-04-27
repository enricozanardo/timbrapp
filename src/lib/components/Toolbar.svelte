<script lang="ts">
	import { editor } from '$lib/stores/editor.svelte';
	import { buildStampedPdf, downloadPdf, stampedFilename } from '$lib/pdf/save';
	import ConfirmDialog from './ConfirmDialog.svelte';
	import AboutDialog from './AboutDialog.svelte';

	let saving = $state(false);
	let saveError = $state<string | null>(null);

	type PendingPrompt = 'clear' | 'close' | null;
	let pending = $state<PendingPrompt>(null);
	let aboutOpen = $state(false);

	async function onSave() {
		if (!editor.pdfBytes) return;
		saving = true;
		saveError = null;
		try {
			const bytes = await buildStampedPdf({
				originalBytes: editor.pdfBytes,
				placements: editor.placements,
				stamps: editor.stamps
			});
			downloadPdf(bytes, stampedFilename(editor.pdfFileName));
		} catch (err) {
			saveError = err instanceof Error ? err.message : String(err);
		} finally {
			saving = false;
		}
	}

	function askClose() {
		// If there's nothing to lose, just close immediately.
		if (editor.placements.length === 0) {
			editor.closePdf();
			return;
		}
		pending = 'close';
	}

	function askClear() {
		if (editor.placements.length === 0) return;
		pending = 'clear';
	}

	function onConfirm() {
		if (pending === 'clear') editor.clearPlacements();
		if (pending === 'close') editor.closePdf();
		pending = null;
	}

	function onCancel() {
		pending = null;
	}

	function zoomOut() {
		editor.scale = Math.max(0.5, +(editor.scale - 0.25).toFixed(2));
	}
	function zoomIn() {
		editor.scale = Math.min(3, +(editor.scale + 0.25).toFixed(2));
	}
	function zoomReset() {
		editor.scale = 1.5;
	}

	function stopPlacing() {
		editor.selectStamp(null);
	}
</script>

<header class="toolbar">
	<div class="left">
		<strong class="brand">timbrapp</strong>
		{#if editor.pdfFileName}
			<span class="file" title={editor.pdfFileName}>· {editor.pdfFileName}</span>
			<span class="badge">{editor.pages.length} pages · {editor.placements.length} stamps</span>
		{/if}
		{#if editor.selectedStampId && editor.hasPdf}
			<button type="button" class="placing-pill" onclick={stopPlacing} title="Press Esc to cancel">
				<span class="dot"></span>
				Placing — click to stop
			</button>
		{/if}
	</div>

	<div class="right">
		{#if editor.hasPdf}
			<div class="zoom" role="group" aria-label="Zoom">
				<button type="button" class="btn" onclick={zoomOut} aria-label="Zoom out">−</button>
				<button type="button" class="btn" onclick={zoomReset} title="Reset zoom">
					{Math.round(editor.scale * 100)}%
				</button>
				<button type="button" class="btn" onclick={zoomIn} aria-label="Zoom in">+</button>
			</div>

			<button type="button" class="btn" onclick={askClear} disabled={editor.placements.length === 0}>
				Clear stamps
			</button>
			<button type="button" class="btn" onclick={askClose}>Close PDF</button>
			<button
				type="button"
				class="btn primary"
				onclick={onSave}
				disabled={saving || editor.placements.length === 0}
			>
				{saving ? 'Saving…' : 'Download stamped PDF'}
			</button>
		{/if}

		<button
			type="button"
			class="btn ghost"
			onclick={() => (aboutOpen = true)}
			aria-label="About timbrapp"
			title="About timbrapp"
		>
			About
		</button>
	</div>

	{#if saveError}
		<div class="error" role="alert">{saveError}</div>
	{/if}
</header>

<ConfirmDialog
	open={pending === 'clear'}
	title="Clear all placed stamps?"
	message="This removes every stamp you've placed on the current PDF. The original document is untouched."
	confirmLabel="Clear stamps"
	danger
	{onConfirm}
	{onCancel}
/>

<ConfirmDialog
	open={pending === 'close'}
	title="Close the current PDF?"
	message="You have placed stamps that haven't been saved. They will be discarded."
	confirmLabel="Close without saving"
	danger
	{onConfirm}
	{onCancel}
/>

<AboutDialog open={aboutOpen} onClose={() => (aboutOpen = false)} />

<style>
	.toolbar {
		display: flex;
		flex-wrap: wrap;
		justify-content: space-between;
		align-items: center;
		gap: 0.75rem;
		padding: 0.75rem 1rem;
		background: var(--surface-1, white);
		border-bottom: 1px solid var(--border, #e2e8f0);
		min-height: 56px;
	}
	.left,
	.right {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}
	.brand {
		font-size: 1rem;
		color: #0f172a;
	}
	.file {
		color: #475569;
		font-size: 0.9rem;
		max-width: 28ch;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.badge {
		font-size: 0.75rem;
		color: #64748b;
		background: #f1f5f9;
		border-radius: 999px;
		padding: 0.15rem 0.5rem;
	}
	.placing-pill {
		appearance: none;
		display: inline-flex;
		align-items: center;
		gap: 0.4rem;
		font-size: 0.78rem;
		color: #1d4ed8;
		background: #dbeafe;
		border: 1px solid #93c5fd;
		border-radius: 999px;
		padding: 0.2rem 0.6rem;
		cursor: pointer;
	}
	.placing-pill:hover {
		background: #bfdbfe;
	}
	.placing-pill .dot {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: #2563eb;
		animation: pulse 1.2s ease-in-out infinite;
	}
	@keyframes pulse {
		0%,
		100% {
			opacity: 1;
		}
		50% {
			opacity: 0.4;
		}
	}
	.btn {
		appearance: none;
		border: 1px solid var(--border, #cbd5e1);
		background: white;
		border-radius: 6px;
		padding: 0.4rem 0.7rem;
		font-size: 0.85rem;
		cursor: pointer;
		color: #1e293b;
	}
	.btn:hover:not(:disabled) {
		background: #f1f5f9;
	}
	.btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
	.btn.primary {
		background: var(--accent, #2563eb);
		color: white;
		border-color: transparent;
	}
	.btn.primary:hover:not(:disabled) {
		background: #1d4ed8;
	}
	.btn.ghost {
		background: transparent;
		color: #475569;
		border-color: transparent;
	}
	.btn.ghost:hover:not(:disabled) {
		background: #f1f5f9;
		color: #0f172a;
	}
	.zoom {
		display: inline-flex;
		gap: 0.25rem;
	}
	.error {
		flex-basis: 100%;
		color: #b91c1c;
		font-size: 0.85rem;
	}
</style>
