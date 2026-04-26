<script lang="ts">
	import type { LoadedPdf } from '$lib/pdf/render';
	import type { PageMeta } from '$lib/types';
	import PageOverlay from './PageOverlay.svelte';

	type Props = {
		loaded: LoadedPdf;
		page: PageMeta;
		scale: number;
	};

	let { loaded, page, scale }: Props = $props();

	let canvas: HTMLCanvasElement;
	let renderToken = 0;

	$effect(() => {
		const myToken = ++renderToken;
		// Touch reactive deps so $effect re-runs when scale changes.
		void scale;
		void page.pageIndex;

		if (!canvas) return;

		let cancelled = false;
		let task: Awaited<ReturnType<typeof loaded.renderPage>> | null = null;

		(async () => {
			try {
				const t = await loaded.renderPage(page.pageIndex, canvas, scale);
				if (cancelled || myToken !== renderToken) {
					t.cancel();
					return;
				}
				task = t;
				await t.promise;
			} catch (err: unknown) {
				const isCancel = err instanceof Error && /cancel/i.test(err.message);
				if (!cancelled && !isCancel) {
					console.warn(`Render error on page ${page.pageIndex + 1}:`, err);
				}
			}
		})();

		return () => {
			cancelled = true;
			if (task) task.cancel();
		};
	});
</script>

<div
	class="page"
	style:width="{page.width * scale}px"
	style:height="{page.height * scale}px"
>
	<canvas bind:this={canvas}></canvas>
	<PageOverlay
		pageIndex={page.pageIndex}
		pageWidth={page.width}
		pageHeight={page.height}
		{scale}
	/>
</div>

<style>
	.page {
		position: relative;
		background: white;
		box-shadow: 0 1px 3px rgba(15, 23, 42, 0.12), 0 4px 16px rgba(15, 23, 42, 0.08);
		margin: 0 auto;
	}
	canvas {
		display: block;
	}
</style>
