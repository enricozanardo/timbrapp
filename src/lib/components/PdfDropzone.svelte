<script lang="ts">
	import { editor } from '$lib/stores/editor.svelte';
	import { openPdfViaDialog } from '$lib/pdf/save';

	let dragOver = $state(false);
	let inputEl: HTMLInputElement;

	const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

	async function handleFile(file: File | null | undefined) {
		if (!file) return;
		if (file.type && file.type !== 'application/pdf') {
			editor.loadError = `Expected a PDF, got ${file.type || 'unknown'}`;
			return;
		}
		await editor.loadPdfFromFile(file);
	}

	/** Desktop: open through the native dialog so we learn the source folder
	 * (and can default saves there). Falls back to the hidden file input. */
	async function chooseFile() {
		if (isTauri) {
			try {
				const opened = await openPdfViaDialog();
				if (opened) {
					await editor.loadPdfFromBytes(opened.bytes, opened.name, opened.dir);
					return;
				}
				return; // user cancelled the native dialog
			} catch (err) {
				editor.loadError = err instanceof Error ? err.message : String(err);
				return;
			}
		}
		inputEl.click();
	}

	function onDrop(e: DragEvent) {
		e.preventDefault();
		dragOver = false;
		const file = e.dataTransfer?.files?.[0];
		void handleFile(file);
	}

	function onDragOver(e: DragEvent) {
		e.preventDefault();
		dragOver = true;
	}

	function onDragLeave() {
		dragOver = false;
	}

	function onChange(e: Event) {
		const t = e.currentTarget as HTMLInputElement;
		void handleFile(t.files?.[0]);
		t.value = '';
	}
</script>

<div
	class="dropzone"
	class:active={dragOver}
	role="region"
	aria-label="Drop a PDF to load"
	ondrop={onDrop}
	ondragover={onDragOver}
	ondragleave={onDragLeave}
>
	<div class="inner">
		<svg
			width="48"
			height="48"
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			stroke-width="1.5"
			aria-hidden="true"
		>
			<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
			<polyline points="14 2 14 8 20 8" />
			<line x1="12" y1="18" x2="12" y2="12" />
			<polyline points="9 15 12 12 15 15" />
		</svg>
		<p class="title">Drop a PDF here</p>
		<p class="subtitle">or</p>
		<button type="button" class="btn primary" onclick={chooseFile}> Choose PDF file </button>
		<input
			bind:this={inputEl}
			type="file"
			accept="application/pdf,.pdf"
			onchange={onChange}
			hidden
		/>
		{#if editor.loading}
			<p class="status">Loading PDF…</p>
		{/if}
		{#if editor.loadError}
			<p class="error">{editor.loadError}</p>
		{/if}
	</div>
</div>

<style>
	.dropzone {
		display: grid;
		place-items: center;
		min-height: 60vh;
		padding: 3rem 2rem;
		border: 2px dashed var(--border, #cbd5e1);
		border-radius: 12px;
		background: var(--surface-2, #f8fafc);
		color: #475569;
		transition: border-color 120ms, background 120ms;
	}
	.dropzone.active {
		border-color: var(--accent, #2563eb);
		background: #eff6ff;
	}
	.inner {
		text-align: center;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 0.5rem;
	}
	.title {
		font-size: 1.25rem;
		font-weight: 600;
		margin: 0.5rem 0 0;
		color: #0f172a;
	}
	.subtitle {
		margin: 0;
		color: #64748b;
	}
	.btn {
		appearance: none;
		border: 1px solid transparent;
		border-radius: 8px;
		padding: 0.5rem 1rem;
		font-size: 0.95rem;
		cursor: pointer;
	}
	.btn.primary {
		background: var(--accent, #2563eb);
		color: white;
	}
	.btn.primary:hover {
		background: #1d4ed8;
	}
	.status {
		margin-top: 1rem;
		color: #64748b;
	}
	.error {
		margin-top: 1rem;
		color: #b91c1c;
	}
</style>
