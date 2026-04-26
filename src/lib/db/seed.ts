import { base } from '$app/paths';
import { addStamp, countStamps } from './stampStore';

const DEFAULT_STAMPS: Array<{ name: string; url: string }> = [
	{ name: 'Libera Università Gorgia', url: `${base}/stamps/timbro_gorgia.png` }
];

/**
 * On first run (empty IndexedDB), download the bundled default stamps from
 * `static/stamps/` and persist them so the user has a starter library.
 *
 * Safe to call on every app load — it only seeds when the store is empty.
 */
export async function seedDefaultStamps(): Promise<void> {
	if ((await countStamps()) > 0) return;

	for (const { name, url } of DEFAULT_STAMPS) {
		try {
			const res = await fetch(url);
			if (!res.ok) {
				console.warn(`[seed] Could not fetch default stamp ${url}: ${res.status}`);
				continue;
			}
			const blob = await res.blob();
			// Some servers return application/octet-stream; force the type.
			const png = blob.type === 'image/png' ? blob : new Blob([blob], { type: 'image/png' });
			await addStamp(name, png);
		} catch (err) {
			console.warn(`[seed] Failed to seed ${name}:`, err);
		}
	}
}
