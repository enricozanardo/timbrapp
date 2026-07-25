/**
 * Typed wrappers over the CIE signing Tauri commands (see
 * `src-tauri/src/cie`). All of these are desktop-only: they invoke the Rust
 * backend, which talks to the CIE via the official PKCS#11 middleware.
 *
 * Every call is gated behind {@link isCieAvailable}; in the plain web build (no
 * Tauri) the functions throw a clear error instead of failing obscurely.
 */
import type {
	CertificateInfo,
	CieStatus,
	EnrollOutcome,
	ReaderInfo,
	SignatureAppearance
} from '$lib/types';

function isTauri(): boolean {
	return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/** True when running inside the Tauri desktop shell (signing is possible). */
export function isCieAvailable(): boolean {
	return isTauri();
}

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
	if (!isTauri()) {
		throw new Error(
			'CIE signing is only available in the TimbrApp desktop app (it needs a smart-card reader).'
		);
	}
	const { invoke } = await import('@tauri-apps/api/core');
	return invoke<T>(cmd, args);
}

/** Probe module + readers for the diagnostics panel. Never throws. */
export async function cieStatus(modulePath?: string): Promise<CieStatus> {
	return invoke<CieStatus>('cie_status', { modulePath });
}

export async function listReaders(modulePath?: string): Promise<ReaderInfo[]> {
	return invoke<ReaderInfo[]>('cie_list_readers', { modulePath });
}

export async function listCertificates(
	slotId: number,
	modulePath?: string
): Promise<CertificateInfo[]> {
	return invoke<CertificateInfo[]>('cie_list_certificates', { slotId, modulePath });
}

/**
 * Check whether the CIE in a slot is already enrolled (paired). PIN-free and
 * safe to call before deciding whether to prompt for enrollment.
 */
export async function isEnrolled(slotId: number, modulePath?: string): Promise<boolean> {
	return invoke<boolean>('cie_is_enrolled', { slotId, modulePath });
}

/**
 * One-time CIE enrollment ("Abilita CIE"). Requires the FULL 8-digit PIN.
 *
 * WARNING: a wrong PIN (outcome code 160) consumes one of the card's 3 attempts.
 * Callers must gate this behind {@link isEnrolled} and explicit user intent.
 */
export async function enroll(pin: string, modulePath?: string): Promise<EnrollOutcome> {
	return invoke<EnrollOutcome>('cie_enroll', { pin, modulePath });
}

/** Base64-encode raw bytes for the IPC boundary. */
export function toBase64(bytes: Uint8Array): string {
	let binary = '';
	const chunk = 0x8000;
	for (let i = 0; i < bytes.length; i += chunk) {
		binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
	}
	return btoa(binary);
}

/** Decode base64 back into bytes. */
export function fromBase64(b64: string): Uint8Array {
	const binary = atob(b64);
	const out = new Uint8Array(binary.length);
	for (let i = 0; i < binary.length; i++) out[i] = binary.charCodeAt(i);
	return out;
}

/**
 * Proof-of-concept raw signature: SHA256+RSA sign arbitrary bytes on the card.
 * Returns the raw signature bytes. Useful to validate the key + PIN before
 * trusting the full PAdES path.
 */
export async function signRaw(params: {
	slotId: number;
	certIdHex: string;
	pin: string;
	data: Uint8Array;
	modulePath?: string;
}): Promise<Uint8Array> {
	const sig = await invoke<string>('cie_sign_raw', {
		modulePath: params.modulePath,
		slotId: params.slotId,
		certIdHex: params.certIdHex,
		pin: params.pin,
		dataBase64: toBase64(params.data)
	});
	return fromBase64(sig);
}

/** Sign a PDF as PAdES-B-B; returns the signed PDF bytes. */
export async function signPdf(params: {
	slotId: number;
	certIdHex: string;
	pin: string;
	pdf: Uint8Array;
	reason?: string;
	location?: string;
	/** Optional visible signature "stamp" appearance. */
	appearance?: SignatureAppearance;
	modulePath?: string;
}): Promise<Uint8Array> {
	const signed = await invoke<string>('cie_sign_pdf', {
		modulePath: params.modulePath,
		slotId: params.slotId,
		certIdHex: params.certIdHex,
		pin: params.pin,
		pdfBase64: toBase64(params.pdf),
		reason: params.reason,
		location: params.location,
		appearance: params.appearance
	});
	return fromBase64(signed);
}

/** Sign arbitrary bytes as a detached CAdES-B-B `.p7s`; returns the CMS bytes. */
export async function signBytesDetached(params: {
	slotId: number;
	certIdHex: string;
	pin: string;
	data: Uint8Array;
	modulePath?: string;
}): Promise<Uint8Array> {
	const p7s = await invoke<string>('cie_sign_bytes', {
		modulePath: params.modulePath,
		slotId: params.slotId,
		certIdHex: params.certIdHex,
		pin: params.pin,
		dataBase64: toBase64(params.data)
	});
	return fromBase64(p7s);
}
