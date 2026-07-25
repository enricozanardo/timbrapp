/** A reusable stamp/signature image stored in the user's library. */
export type Stamp = {
	id: string;
	name: string;
	/** Always image/png. */
	blob: Blob;
	createdAt: number;
};

/**
 * A stamp instance placed on a specific PDF page.
 *
 * Coordinates are stored in PDF points with origin at the bottom-left of the
 * page (the same coordinate space `pdf-lib` uses). This makes the save step
 * trivial and keeps the conversion logic in exactly one place — the overlay
 * components convert from screen-space when creating/dragging/resizing.
 */
export type Placement = {
	id: string;
	stampId: string;
	pageIndex: number;
	x: number;
	y: number;
	width: number;
	height: number;
	/** Counter-clockwise degrees, optional. Reserved for a future rotation UI. */
	rotation?: number;
};

/** A PC-SC slot/reader with a card inserted (from the Rust backend). */
export type ReaderInfo = {
	slotId: number;
	slotDescription: string;
	manufacturer: string;
	tokenPresent: boolean;
	tokenLabel: string | null;
};

/** A certificate available on the CIE (from the Rust backend). */
export type CertificateInfo = {
	/** PKCS#11 CKA_ID (hex) used to select the key/cert for signing. */
	idHex: string;
	label: string;
	subject: string;
	issuer: string;
	serialHex: string;
	notBefore: string;
	notAfter: string;
	slotId: number;
	/** True for the likely FEA/subscription (signing) certificate. */
	keyUsageSign: boolean;
};

/** Diagnostics summary for the CIE setup panel. */
export type CieStatus = {
	modulePath: string | null;
	moduleFound: boolean;
	readers: ReaderInfo[];
	message: string;
};

/** Result of a one-time CIE enrollment ("Abilita CIE"). */
export type EnrollOutcome = {
	/** Raw vendor return code (0 = OK, 240 = already enrolled, 160 = wrong PIN, …). */
	code: number;
	ok: boolean;
	/** True when a wrong PIN consumed one of the 3 attempts. */
	consumedAttempt: boolean;
	message: string;
	pan: string | null;
	cardholder: string | null;
	serial: string | null;
	/** Remaining PIN attempts as reported by the module, when available. */
	attemptsLeft: number | null;
};

/**
 * A visible signature "stamp" appearance passed to the backend when signing.
 * `rect` is `[x0, y0, x1, y1]` in PDF points (bottom-left origin).
 */
export type SignatureAppearance = {
	pageIndex: number;
	rect: [number, number, number, number];
	lines: string[];
};

/** Cached page metadata exposed by the editor store. */
export type PageMeta = {
	pageIndex: number;
	/** Width in PDF points (1/72 inch). */
	width: number;
	/** Height in PDF points. */
	height: number;
};
