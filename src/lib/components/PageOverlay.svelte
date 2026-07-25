<script lang="ts">
	import { editor } from '$lib/stores/editor.svelte';
	import { buildPlacementAtClick } from '$lib/pdf/coords';
	import { readImageSize } from '$lib/stamps/objectUrl';
	import PlacedStamp from './PlacedStamp.svelte';

	type Props = {
		pageIndex: number;
		pageWidth: number;
		pageHeight: number;
		scale: number;
	};

	let { pageIndex, pageWidth, pageHeight, scale }: Props = $props();

	const placements = $derived(editor.placementsForPage(pageIndex));

	async function onClick(e: MouseEvent) {
		// Ignore clicks on placements / handles (they stop propagation, but still).
		if (e.target !== e.currentTarget) return;

		const overlay = e.currentTarget as HTMLDivElement;
		const r = overlay.getBoundingClientRect();
		const cssX = e.clientX - r.left;
		const cssY = e.clientY - r.top;

		// Placing the visible signature box takes priority over stamp placement.
		if (editor.placingSignature) {
			const widthPt = Math.min(240, pageWidth * 0.45);
			const rect = buildPlacementAtClick({
				clickCssX: cssX,
				clickCssY: cssY,
				pageHeight,
				pageWidth,
				scale,
				defaultWidthPt: widthPt,
				aspectRatio: widthPt / 70
			});
			editor.addSignaturePlacement({ pageIndex, ...rect });
			return;
		}

		const stamp = editor.selectedStamp;
		if (!stamp) {
			editor.selectPlacement(null);
			return;
		}

		const size = await readImageSize(stamp.blob).catch(() => ({ width: 1, height: 1 }));
		const aspectRatio = size.width / Math.max(1, size.height);

		const rect = buildPlacementAtClick({
			clickCssX: cssX,
			clickCssY: cssY,
			pageHeight,
			pageWidth,
			scale,
			defaultWidthPt: Math.min(140, pageWidth * 0.25),
			aspectRatio
		});

		editor.addPlacement({
			stampId: stamp.id,
			pageIndex,
			...rect
		});
	}
</script>

<div
	class="overlay"
	class:placing={editor.selectedStampId !== null || editor.placingSignature}
	style:width="{pageWidth * scale}px"
	style:height="{pageHeight * scale}px"
	role="presentation"
	onclick={onClick}
>
	{#each placements as placement (placement.id)}
		<PlacedStamp {placement} {pageWidth} {pageHeight} {scale} />
	{/each}
</div>

<style>
	.overlay {
		position: absolute;
		inset: 0;
		cursor: default;
	}
	.overlay.placing {
		cursor: crosshair;
	}
</style>
