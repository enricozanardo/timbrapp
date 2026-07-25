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

function isTauri(): boolean {
	return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

const LAST_DIR_KEY = 'timbrapp:lastSaveDir';

function rememberDir(path: string): void {
	const sep = path.includes('\\') ? '\\' : '/';
	const idx = path.lastIndexOf(sep);
	if (idx > 0) {
		try {
			localStorage.setItem(LAST_DIR_KEY, path.slice(0, idx));
		} catch {
			/* ignore storage errors */
		}
	}
}

function lastSaveDir(): string | null {
	try {
		return localStorage.getItem(LAST_DIR_KEY);
	} catch {
		return null;
	}
}

function toBase64(bytes: Uint8Array): string {
	let binary = '';
	const chunk = 0x8000;
	for (let i = 0; i < bytes.length; i += chunk) {
		binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
	}
	return btoa(binary);
}

function fromBase64(b64: string): Uint8Array {
	const binary = atob(b64);
	const out = new Uint8Array(binary.length);
	for (let i = 0; i < binary.length; i++) out[i] = binary.charCodeAt(i);
	return out;
}

/**
 * Save `bytes` to disk. On the desktop app this opens a native "Save as" dialog
 * (defaulting to `defaultDir` — typically the source PDF's folder — then the
 * last-used folder) and returns the chosen path, or `null` if cancelled. In the
 * plain web build it falls back to a browser download and returns `null`.
 */
export async function savePdf(
	bytes: Uint8Array,
	defaultName: string,
	defaultDir?: string | null
): Promise<string | null> {
	if (!isTauri()) {
		downloadPdf(bytes, defaultName);
		return null;
	}
	const { save } = await import('@tauri-apps/plugin-dialog');
	const { invoke } = await import('@tauri-apps/api/core');
	const dir = defaultDir ?? lastSaveDir();
	const sep = dir && dir.includes('\\') ? '\\' : '/';
	const defaultPath = dir ? `${dir}${sep}${defaultName}` : defaultName;
	const path = await save({
		defaultPath,
		filters: [{ name: 'PDF', extensions: ['pdf'] }]
	});
	if (!path) return null;
	await invoke('write_file_bytes', { path, dataBase64: toBase64(bytes) });
	rememberDir(path);
	return path;
}

export type OpenedPdf = { name: string; dir: string; bytes: Uint8Array };

/**
 * Open a PDF through the native file dialog (desktop only). Returns the file
 * name, its containing directory (so saves can default there), and its bytes.
 * Returns `null` when not in Tauri or when the user cancels.
 */
export async function openPdfViaDialog(): Promise<OpenedPdf | null> {
	if (!isTauri()) return null;
	const { open } = await import('@tauri-apps/plugin-dialog');
	const { invoke } = await import('@tauri-apps/api/core');
	const selected = await open({
		multiple: false,
		directory: false,
		filters: [{ name: 'PDF', extensions: ['pdf'] }]
	});
	if (!selected || typeof selected !== 'string') return null;
	const b64 = await invoke<string>('read_file_bytes', { path: selected });
	const sep = selected.includes('\\') ? '\\' : '/';
	const idx = selected.lastIndexOf(sep);
	return {
		name: idx >= 0 ? selected.slice(idx + 1) : selected,
		dir: idx >= 0 ? selected.slice(0, idx) : '',
		bytes: fromBase64(b64)
	};
}

/** Replace `name.pdf` with `name-stamped.pdf`, defaulting if no name was set. */
export function stampedFilename(originalName: string | null): string {
	if (!originalName) return 'stamped.pdf';
	const dot = originalName.lastIndexOf('.');
	const base = dot > 0 ? originalName.slice(0, dot) : originalName;
	return `${base}-stamped.pdf`;
}

/** Replace `name.pdf` with `name-signed.pdf`, defaulting if no name was set. */
export function signedFilename(originalName: string | null): string {
	if (!originalName) return 'signed.pdf';
	const dot = originalName.lastIndexOf('.');
	const base = dot > 0 ? originalName.slice(0, dot) : originalName;
	return `${base}-signed.pdf`;
}
