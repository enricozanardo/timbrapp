import type { Stamp } from '../types';

/**
 * Cache of `URL.createObjectURL` results keyed by stamp id, so the same blob
 * is only converted once and previews are reused across renders.
 *
 * Components that fetch a URL for a stamp don't need to revoke it themselves;
 * `releaseStampUrl` is called when a stamp is deleted from the library.
 */
const urlByStampId = new Map<string, string>();

export function getStampUrl(stamp: Stamp): string {
	const cached = urlByStampId.get(stamp.id);
	if (cached) return cached;
	const url = URL.createObjectURL(stamp.blob);
	urlByStampId.set(stamp.id, url);
	return url;
}

export function releaseStampUrl(stampId: string): void {
	const url = urlByStampId.get(stampId);
	if (url) {
		URL.revokeObjectURL(url);
		urlByStampId.delete(stampId);
	}
}

export function releaseAllStampUrls(): void {
	for (const url of urlByStampId.values()) URL.revokeObjectURL(url);
	urlByStampId.clear();
}

/** Read the natural width/height of a PNG blob. */
export function readImageSize(blob: Blob): Promise<{ width: number; height: number }> {
	return new Promise((resolve, reject) => {
		const url = URL.createObjectURL(blob);
		const img = new Image();
		img.onload = () => {
			const size = { width: img.naturalWidth, height: img.naturalHeight };
			URL.revokeObjectURL(url);
			resolve(size);
		};
		img.onerror = (e) => {
			URL.revokeObjectURL(url);
			reject(e);
		};
		img.src = url;
	});
}
