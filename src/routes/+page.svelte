<script lang="ts">
	import { onMount } from 'svelte';
	import { editor } from '$lib/stores/editor.svelte';
	import { seedDefaultStamps } from '$lib/db/seed';
	import StampLibrary from '$lib/components/StampLibrary.svelte';
	import PdfViewer from '$lib/components/PdfViewer.svelte';
	import PdfDropzone from '$lib/components/PdfDropzone.svelte';
	import Toolbar from '$lib/components/Toolbar.svelte';

	let booted = $state(false);

	onMount(async () => {
		await seedDefaultStamps();
		await editor.refreshStamps();
		booted = true;
	});
</script>

<svelte:head>
	<title>timbrapp — PDF stamping</title>
</svelte:head>

<div class="app">
	<Toolbar />
	<div class="body">
		{#if booted}
			<StampLibrary />
			<main class="main">
				{#if editor.hasPdf}
					<PdfViewer />
				{:else}
					<div class="empty-pane">
						<PdfDropzone />
					</div>
				{/if}
			</main>
		{:else}
			<div class="boot">Loading…</div>
		{/if}
	</div>
</div>

<style>
	.app {
		display: grid;
		grid-template-rows: auto 1fr;
		height: 100vh;
		min-height: 0;
	}
	.body {
		display: grid;
		grid-template-columns: auto 1fr;
		min-height: 0;
		overflow: hidden;
	}
	.main {
		display: flex;
		flex-direction: column;
		min-width: 0;
		min-height: 0;
	}
	.empty-pane {
		flex: 1;
		display: grid;
		place-items: center;
		padding: 2rem;
		overflow: auto;
	}
	.boot {
		display: grid;
		place-items: center;
		width: 100%;
		color: #64748b;
	}
</style>
