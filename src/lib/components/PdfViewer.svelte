<script lang="ts">
	import { editor } from '$lib/stores/editor.svelte';
	import PdfPage from './PdfPage.svelte';

	function onBackgroundClick(e: MouseEvent) {
		// Click on empty viewer area: deselect the current placement.
		if (e.target === e.currentTarget) {
			editor.selectPlacement(null);
		}
	}
</script>

<div class="viewer" role="presentation" onclick={onBackgroundClick}>
	{#if editor.loaded}
		{#each editor.pages as page (page.pageIndex)}
			<PdfPage loaded={editor.loaded} {page} scale={editor.scale} />
		{/each}
	{/if}
</div>

<style>
	.viewer {
		flex: 1;
		min-height: 0;
		overflow: auto;
		padding: 1.5rem;
		background: var(--surface-2, #f1f5f9);
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 1rem;
	}
</style>
