import { PDFDocument, degrees } from 'pdf-lib';
import type { Placement, Stamp } from '../types';

export type StampedPdfInput = {
	originalBytes: Uint8Array;
	placements: Placement[];
	stamps: Stamp[];
};

/**
 * Apply the given `placements` (already in PDF-point space) to the original
 * PDF bytes and return a freshly-saved PDF as a `Uint8Array`.
 *
 * - Each unique stamp PNG is embedded once and reused across placements.
 * - `placement.rotation` is in degrees, counter-clockwise (matching pdf-lib).
 */
export async function buildStampedPdf({
	originalBytes,
	placements,
	stamps
}: StampedPdfInput): Promise<Uint8Array> {
	const doc = await PDFDocument.load(originalBytes);

	const stampById = new Map<string, Stamp>(stamps.map((s) => [s.id, s]));
	const usedStampIds = new Set(placements.map((p) => p.stampId));

	const embedded = new Map<string, Awaited<ReturnType<typeof doc.embedPng>>>();
	for (const stampId of usedStampIds) {
		const stamp = stampById.get(stampId);
		if (!stamp) continue;
		const buf = await stamp.blob.arrayBuffer();
		embedded.set(stampId, await doc.embedPng(new Uint8Array(buf)));
	}

	const pages = doc.getPages();
	for (const placement of placements) {
		const img = embedded.get(placement.stampId);
		const page = pages[placement.pageIndex];
		if (!img || !page) continue;

		page.drawImage(img, {
			x: placement.x,
			y: placement.y,
			width: placement.width,
			height: placement.height,
			rotate: placement.rotation ? degrees(placement.rotation) : undefined
		});
	}

	return doc.save();
}

/**
 * Convenience: trigger a browser download of `bytes` as `filename`.
 * Uses an `<a download>` + Blob URL; the URL is revoked after a short delay.
 */
export function downloadPdf(bytes: Uint8Array, filename: string): void {
	const blob = new Blob([bytes as BlobPart], { type: 'application/pdf' });
	const url = URL.createObjectURL(blob);
	const a = document.createElement('a');
	a.href = url;
	a.download = filename;
	document.body.appendChild(a);
	a.click();
	a.remove();
	setTimeout(() => URL.revokeObjectURL(url), 5_000);
}

/** Replace `name.pdf` with `name-stamped.pdf`, defaulting if no name was set. */
export function stampedFilename(originalName: string | null): string {
	if (!originalName) return 'stamped.pdf';
	const dot = originalName.lastIndexOf('.');
	const base = dot > 0 ? originalName.slice(0, dot) : originalName;
	return `${base}-stamped.pdf`;
}
