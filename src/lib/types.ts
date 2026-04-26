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

/** Cached page metadata exposed by the editor store. */
export type PageMeta = {
	pageIndex: number;
	/** Width in PDF points (1/72 inch). */
	width: number;
	/** Height in PDF points. */
	height: number;
};
