<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
	import { editor } from '$lib/stores/editor.svelte';
	import { addStamp, deleteStamp } from '$lib/db/stampStore';
	import { getStampUrl, releaseStampUrl } from '$lib/stamps/objectUrl';
	import ConfirmDialog from './ConfirmDialog.svelte';

	let dragOver = $state(false);
	let showHint = $state(false);
	let uploadError = $state<string | null>(null);
	let pendingDeleteId = $state<string | null>(null);
	const pendingStamp = $derived(
		pendingDeleteId ? editor.stamps.find((s) => s.id === pendingDeleteId) ?? null : null
	);
	let unlisten: (() => void) | undefined;

	async function addPngFile(file: File) {
		uploadError = null;
		showHint = false;
		try {
			if (file.type && file.type !== 'image/png') {
				throw new Error('Only PNG images are supported');
			}
			const name = file.name.replace(/\.png$/i, '') || 'stamp';
			await addStamp(name, file);
			await editor.refreshStamps();
		} catch (err) {
			uploadError = err instanceof Error ? err.message : String(err);
		}
	}

	async function addPngFromPath(path: string) {
		try {
			type FD = { name: string; data: number[] };
			const fd = await invoke<FD>('read_file', { path });
			await addPngFile(new File([new Uint8Array(fd.data)], fd.name, { type: 'image/png' }));
		} catch (err) {
			uploadError = err instanceof Error ? err.message : String(err);
		}
	}

	// Tauri v2 intercepts cross-app file drops before they reach the JS DOM.
	onMount(async () => {
		const win = getCurrentWebviewWindow();
		unlisten = await win.onDragDropEvent((event) => {
			const payload = event.payload as { type: string; paths?: string[] };
			if (payload.type === 'enter' || payload.type === 'over') {
				if (payload.paths?.some(p => p.toLowerCase().endsWith('.png'))) {
					dragOver = true;
				}
			} else if (payload.type === 'leave') {
				dragOver = false;
			} else if (payload.type === 'drop') {
				dragOver = false;
				showHint = false;
				const pngPath = payload.paths?.find(p => p.toLowerCase().endsWith('.png'));
				if (pngPath) void addPngFromPath(pngPath);
			}
		});
	});

	onDestroy(() => unlisten?.());

	// DOM fallback for in-browser dev mode.
	function onDrop(e: DragEvent) {
		e.preventDefault();
		dragOver = false;
		const file = e.dataTransfer?.files?.[0];
		if (file) void addPngFile(file);
	}
	function onDragOver(e: DragEvent) { e.preventDefault(); dragOver = true; }
	function onDragLeave() { dragOver = false; }

	function askDelete(stampId: string) {
		pendingDeleteId = stampId;
	}

	async function confirmDelete() {
		const id = pendingDeleteId;
		pendingDeleteId = null;
		if (!id) return;
		releaseStampUrl(id);
		await deleteStamp(id);
		await editor.refreshStamps();
	}

	function cancelDelete() {
		pendingDeleteId = null;
	}

	function onSelect(stampId: string) {
		editor.selectStamp(editor.selectedStampId === stampId ? null : stampId);
	}

	/**
	 * Cross-platform file picker.
	 * - Linux: pick_file opens Thunar and returns null → show drag hint.
	 * - macOS / Windows: pick_file returns file bytes from the native dialog.
	 */
	async function onAddStamp() {
		uploadError = null;
		try {
			type Picked = { name: string; data: number[] } | null;
			const result = await invoke<Picked>('pick_file', { filterType: 'png' });
			if (!result) {
				// Linux: Thunar was opened — show the drag-and-drop hint
				showHint = true;
				return;
			}
			// macOS / Windows: file returned directly from native dialog
			const blob = new File([new Uint8Array(result.data)], result.name, { type: 'image/png' });
			const name = result.name.replace(/\.png$/i, '') || 'stamp';
			await addStamp(name, blob);
			await editor.refreshStamps();
		} catch (err) {
			uploadError = err instanceof Error ? err.message : String(err);
		}
	}
</script>

<aside
	class="library"
	class:drag-over={dragOver}
	aria-label="Stamp library"
	ondrop={onDrop}
	ondragover={onDragOver}
	ondragleave={onDragLeave}
>
	<header>
		<h2>Stamps</h2>
		<button type="button" class="btn" onclick={onAddStamp}>+ Add PNG</button>
	</header>

	{#if showHint}
		<p class="hint-drag">Thunar is open — drag a PNG here ⇓</p>
	{/if}

	{#if uploadError}
		<p class="error">{uploadError}</p>
	{/if}

	{#if editor.stamps.length === 0}
		<p class="empty">No stamps yet. Add a PNG to get started.</p>
	{:else}
		<ul class="grid">
			{#each editor.stamps as stamp (stamp.id)}
				{@const selected = editor.selectedStampId === stamp.id}
				<li class="card" class:selected>
					<button
						type="button"
						class="thumb"
						aria-pressed={selected}
						aria-label="Use stamp {stamp.name}"
						onclick={() => onSelect(stamp.id)}
					>
						<img src={getStampUrl(stamp)} alt="" loading="lazy" />
					</button>
					<div class="meta">
						<span class="name" title={stamp.name}>{stamp.name}</span>
						<button
							type="button"
							class="icon"
							aria-label="Delete stamp"
							onclick={() => askDelete(stamp.id)}
						>
							×
						</button>
					</div>
				</li>
			{/each}
		</ul>
	{/if}

	{#if editor.selectedStampId && editor.hasPdf}
		<p class="hint">Click on the PDF to place the selected stamp.</p>
	{/if}

	{#if editor.placements.length > 0}
		<p class="hint subtle">
			To remove a placed stamp, hover it and click <strong>×</strong>, or select it and press
			<kbd>Delete</kbd>.
		</p>
	{/if}
</aside>

<ConfirmDialog
	open={pendingDeleteId !== null}
	title="Delete stamp from library?"
	message={pendingStamp
		? `“${pendingStamp.name}” will be removed permanently. Any placements of this stamp on the current PDF will also be removed.`
		: ''}
	confirmLabel="Delete"
	danger
	onConfirm={confirmDelete}
	onCancel={cancelDelete}
/>

<style>
	.library {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		padding: 1rem;
		background: var(--surface-1, #ffffff);
		border-right: 1px solid var(--border, #e2e8f0);
		min-width: 240px;
		max-width: 280px;
		overflow-y: auto;
	}
	header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 0.5rem;
	}
	h2 {
		margin: 0;
		font-size: 1rem;
		font-weight: 600;
		color: #0f172a;
	}
	.btn {
		appearance: none;
		border: 1px solid var(--border, #cbd5e1);
		background: white;
		border-radius: 6px;
		padding: 0.35rem 0.6rem;
		font-size: 0.8rem;
		cursor: pointer;
		color: #1e293b;
	}
	.btn:hover {
		background: #f1f5f9;
	}
	.empty,
	.hint {
		font-size: 0.85rem;
		color: #64748b;
		margin: 0;
	}
	.hint.subtle {
		padding-top: 0.5rem;
		border-top: 1px dashed var(--border, #e2e8f0);
		line-height: 1.4;
	}
	kbd {
		font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
		font-size: 0.75rem;
		padding: 0.05rem 0.35rem;
		background: #f1f5f9;
		border: 1px solid var(--border, #cbd5e1);
		border-radius: 4px;
		color: #1e293b;
	}
	.error {
		font-size: 0.85rem;
		color: #b91c1c;
	}
	.grid {
		list-style: none;
		padding: 0;
		margin: 0;
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0.5rem;
	}
	.card {
		display: flex;
		flex-direction: column;
		border: 1px solid var(--border, #e2e8f0);
		border-radius: 8px;
		overflow: hidden;
		background: white;
	}
	.card.selected {
		border-color: var(--accent, #2563eb);
		box-shadow: 0 0 0 2px rgba(37, 99, 235, 0.18);
	}
	.thumb {
		appearance: none;
		border: 0;
		padding: 0.5rem;
		background: white;
		cursor: pointer;
		aspect-ratio: 1 / 1;
		display: grid;
		place-items: center;
	}
	.thumb img {
		max-width: 100%;
		max-height: 100%;
		object-fit: contain;
	}
	.meta {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.25rem;
		padding: 0.25rem 0.5rem;
		font-size: 0.8rem;
		background: #f8fafc;
		border-top: 1px solid var(--border, #e2e8f0);
	}
	.name {
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		color: #0f172a;
	}
	.icon {
		appearance: none;
		border: 0;
		background: transparent;
		font-size: 1.1rem;
		line-height: 1;
		color: #64748b;
		cursor: pointer;
		padding: 0 0.25rem;
	}
	.icon:hover {
		color: #b91c1c;
	}
	.library.drag-over {
		background: #eff6ff;
		border-right-color: var(--accent, #2563eb);
		outline: 2px dashed var(--accent, #2563eb);
		outline-offset: -4px;
	}
	.hint-drag {
		padding: 0.4rem 0.75rem;
		border-radius: 6px;
		background: #eff6ff;
		color: #1d4ed8;
		font-size: 0.8rem;
		font-weight: 500;
		border: 1px solid #bfdbfe;
		margin: 0;
	}
</style>
