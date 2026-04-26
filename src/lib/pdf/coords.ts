/**
 * Coordinate conversion between screen (CSS pixels, origin top-left) and the
 * PDF point space (origin bottom-left) used by `pdf-lib`.
 *
 * All `Placement` records are stored in PDF point space so the save step is
 * trivial; conversion happens here whenever we touch the DOM.
 */

import type { Placement } from '../types';

export type ScreenRect = {
	/** CSS pixels from the page overlay's left edge. */
	left: number;
	/** CSS pixels from the page overlay's top edge. */
	top: number;
	width: number;
	height: number;
};

/** Convert a placement (PDF points) into a screen-space rect at `scale`. */
export function placementToScreenRect(
	placement: Pick<Placement, 'x' | 'y' | 'width' | 'height'>,
	pageHeight: number,
	scale: number
): ScreenRect {
	return {
		left: placement.x * scale,
		// y in PDF is the *bottom* of the box; convert to top-left CSS origin.
		top: (pageHeight - placement.y - placement.height) * scale,
		width: placement.width * scale,
		height: placement.height * scale
	};
}

/** Convert a screen-space rect (CSS px) into PDF point coords for a placement. */
export function screenRectToPlacement(
	rect: ScreenRect,
	pageHeight: number,
	scale: number
): Pick<Placement, 'x' | 'y' | 'width' | 'height'> {
	const width = rect.width / scale;
	const height = rect.height / scale;
	const x = rect.left / scale;
	const y = pageHeight - rect.top / scale - height;
	return { x, y, width, height };
}

/**
 * Build a centered placement at a click point.
 *
 * `clickCss` is in CSS pixels relative to the page overlay; `aspectRatio` is
 * the natural width/height ratio of the stamp PNG, used to derive the
 * placement's height from a default width.
 */
export function buildPlacementAtClick(opts: {
	clickCssX: number;
	clickCssY: number;
	pageHeight: number;
	pageWidth: number;
	scale: number;
	defaultWidthPt: number;
	aspectRatio: number;
}): Pick<Placement, 'x' | 'y' | 'width' | 'height'> {
	const widthPt = opts.defaultWidthPt;
	const heightPt = widthPt / opts.aspectRatio;

	const widthCss = widthPt * opts.scale;
	const heightCss = heightPt * opts.scale;

	const left = opts.clickCssX - widthCss / 2;
	const top = opts.clickCssY - heightCss / 2;

	return screenRectToPlacement(
		{ left, top, width: widthCss, height: heightCss },
		opts.pageHeight,
		opts.scale
	);
}

/** Clamp a placement so it stays fully within the page boundaries. */
export function clampPlacement(
	p: Pick<Placement, 'x' | 'y' | 'width' | 'height'>,
	pageWidth: number,
	pageHeight: number
): Pick<Placement, 'x' | 'y' | 'width' | 'height'> {
	const width = Math.min(p.width, pageWidth);
	const height = Math.min(p.height, pageHeight);
	const x = Math.min(Math.max(0, p.x), pageWidth - width);
	const y = Math.min(Math.max(0, p.y), pageHeight - height);
	return { x, y, width, height };
}
