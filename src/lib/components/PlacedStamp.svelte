<script lang="ts">
	import { editor } from '$lib/stores/editor.svelte';
	import { getStampUrl } from '$lib/stamps/objectUrl';
	import { placementToScreenRect, screenRectToPlacement } from '$lib/pdf/coords';
	import type { Placement } from '$lib/types';

	type Props = {
		placement: Placement;
		pageWidth: number;
		pageHeight: number;
		scale: number;
	};

	let { placement, pageWidth, pageHeight, scale }: Props = $props();

	const stamp = $derived(editor.getStamp(placement.stampId));
	const stampUrl = $derived(stamp ? getStampUrl(stamp) : '');
	const rect = $derived(placementToScreenRect(placement, pageHeight, scale));
	const selected = $derived(editor.selectedPlacementId === placement.id);

	type DragKind = 'move' | 'nw' | 'ne' | 'sw' | 'se';

	type DragState = {
		kind: DragKind;
		startX: number;
		startY: number;
		startRect: { left: number; top: number; width: number; height: number };
		pointerId: number;
	};

	let drag: DragState | null = null;
	const MIN_PT = 16;

	function applyRect(left: number, top: number, width: number, height: number) {
		const next = screenRectToPlacement({ left, top, width, height }, pageHeight, scale);
		editor.updatePlacement(placement.id, next);
	}

	function onPointerDown(e: PointerEvent, kind: DragKind) {
		if (e.button !== 0) return;
		e.stopPropagation();
		(e.currentTarget as Element).setPointerCapture(e.pointerId);
		editor.selectPlacement(placement.id);
		drag = {
			kind,
			startX: e.clientX,
			startY: e.clientY,
			startRect: { ...rect },
			pointerId: e.pointerId
		};
	}

	function onPointerMove(e: PointerEvent) {
		if (!drag || drag.pointerId !== e.pointerId) return;
		const dx = e.clientX - drag.startX;
		const dy = e.clientY - drag.startY;
		const r = drag.startRect;
		const minPx = MIN_PT * scale;
		let { left, top, width, height } = r;

		switch (drag.kind) {
			case 'move':
				left = r.left + dx;
				top = r.top + dy;
				break;
			case 'se':
				width = Math.max(minPx, r.width + dx);
				height = Math.max(minPx, r.height + dy);
				break;
			case 'sw':
				width = Math.max(minPx, r.width - dx);
				height = Math.max(minPx, r.height + dy);
				left = r.left + (r.width - width);
				break;
			case 'ne':
				width = Math.max(minPx, r.width + dx);
				height = Math.max(minPx, r.height - dy);
				top = r.top + (r.height - height);
				break;
			case 'nw':
				width = Math.max(minPx, r.width - dx);
				height = Math.max(minPx, r.height - dy);
				left = r.left + (r.width - width);
				top = r.top + (r.height - height);
				break;
		}
		applyRect(left, top, width, height);
	}

	function endDrag(e: PointerEvent) {
		if (!drag || drag.pointerId !== e.pointerId) return;
		const target = e.currentTarget as Element;
		if (target.hasPointerCapture(e.pointerId)) target.releasePointerCapture(e.pointerId);
		drag = null;
	}

	function onKeydown(e: KeyboardEvent) {
		if (!selected) return;
		// Ignore deletes triggered while typing in an input / textarea / contenteditable.
		const target = e.target as HTMLElement | null;
		if (target) {
			const tag = target.tagName;
			if (tag === 'INPUT' || tag === 'TEXTAREA' || target.isContentEditable) return;
		}
		if (e.key === 'Delete' || e.key === 'Backspace') {
			e.preventDefault();
			editor.removePlacement(placement.id);
		}
	}

	function onRemove(e: MouseEvent) {
		e.stopPropagation();
		editor.removePlacement(placement.id);
	}
</script>

<svelte:window onkeydown={onKeydown} />

<div
	class="placement"
	class:selected
	style:left="{rect.left}px"
	style:top="{rect.top}px"
	style:width="{rect.width}px"
	style:height="{rect.height}px"
	role="button"
	tabindex="0"
	aria-label="Placed stamp {stamp?.name ?? ''}"
	onpointerdown={(e) => onPointerDown(e, 'move')}
	onpointermove={onPointerMove}
	onpointerup={endDrag}
	onpointercancel={endDrag}
>
	{#if stampUrl}
		<img src={stampUrl} alt="" draggable="false" />
	{/if}
	<button
		type="button"
		class="remove"
		aria-label="Remove this stamp"
		title="Remove this stamp"
		onpointerdown={(e) => e.stopPropagation()}
		onclick={onRemove}
	>
		×
	</button>
	{#if selected}
		{#each ['nw', 'ne', 'sw', 'se'] as const as corner}
			<span
				class="handle {corner}"
				role="presentation"
				onpointerdown={(e) => onPointerDown(e, corner)}
				onpointermove={onPointerMove}
				onpointerup={endDrag}
				onpointercancel={endDrag}
			></span>
		{/each}
	{/if}
</div>

<style>
	.placement {
		position: absolute;
		cursor: move;
		user-select: none;
		touch-action: none;
		outline: 1px dashed transparent;
	}
	.placement:hover {
		outline-color: rgba(37, 99, 235, 0.4);
	}
	.placement.selected {
		outline: 1.5px solid var(--accent, #2563eb);
		outline-offset: 0;
	}
	.placement img {
		width: 100%;
		height: 100%;
		display: block;
		pointer-events: none;
		-webkit-user-drag: none;
	}
	.handle {
		position: absolute;
		width: 12px;
		height: 12px;
		background: white;
		border: 1.5px solid var(--accent, #2563eb);
		border-radius: 2px;
		box-sizing: border-box;
	}
	.handle.nw {
		top: -6px;
		left: -6px;
		cursor: nwse-resize;
	}
	.handle.ne {
		top: -6px;
		right: -6px;
		cursor: nesw-resize;
	}
	.handle.sw {
		bottom: -6px;
		left: -6px;
		cursor: nesw-resize;
	}
	.handle.se {
		bottom: -6px;
		right: -6px;
		cursor: nwse-resize;
	}
	.remove {
		position: absolute;
		top: -12px;
		right: -12px;
		width: 26px;
		height: 26px;
		border-radius: 50%;
		border: 1.5px solid #b91c1c;
		background: white;
		color: #b91c1c;
		font-size: 1.1rem;
		font-weight: 600;
		line-height: 1;
		cursor: pointer;
		display: grid;
		place-items: center;
		padding: 0;
		box-shadow: 0 1px 3px rgba(15, 23, 42, 0.18);
		opacity: 0;
		transform: scale(0.85);
		transition: opacity 100ms ease, transform 100ms ease, background 100ms ease;
		pointer-events: none;
	}
	.placement:hover .remove,
	.placement.selected .remove,
	.remove:focus-visible {
		opacity: 1;
		transform: scale(1);
		pointer-events: auto;
	}
	.remove:hover {
		background: #fee2e2;
	}
	.remove:focus-visible {
		outline: 2px solid #b91c1c;
		outline-offset: 2px;
	}
</style>
