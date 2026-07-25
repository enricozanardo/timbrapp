import { v4 as uuid } from 'uuid';
import { LoadedPdf } from '../pdf/render';
import { listStamps } from '../db/stampStore';
import type { PageMeta, Placement, Stamp } from '../types';
import { clampPlacement } from '../pdf/coords';

/**
 * Sentinel `stampId` marking a placement as the visible CIE signature box
 * rather than an image stamp. It is drawn by the Rust backend as the
 * signature's appearance, so `buildStampedPdf` skips it (no matching stamp).
 */
export const SIGNATURE_STAMP_ID = '__cie_signature__';

/**
 * Reactive editor state: the loaded PDF (raw bytes + render proxy), placed
 * stamps per page, the user's stamp library, and current selections.
 *
 * Implemented as a runes-backed singleton class. Components read fields like
 * `editor.placements` directly (no `.value` boilerplate) and call mutator
 * methods like `editor.addPlacement(...)`.
 */
class EditorState {
	pdfFileName = $state<string | null>(null);
	/** Absolute directory the PDF was opened from (native dialog only); null for
	 * drag-drop / browser file input. Used to default the save location. */
	pdfSourceDir = $state<string | null>(null);
	/** Pristine bytes of the loaded PDF, kept around so pdf-lib can read them at save time. */
	pdfBytes = $state<Uint8Array | null>(null);
	loaded = $state<LoadedPdf | null>(null);
	pages = $state<PageMeta[]>([]);

	scale = $state(1.5);

	stamps = $state<Stamp[]>([]);
	selectedStampId = $state<string | null>(null);

	placements = $state<Placement[]>([]);
	selectedPlacementId = $state<string | null>(null);

	/** When true, the next click on a page drops the visible signature box. */
	placingSignature = $state(false);

	loading = $state(false);
	loadError = $state<string | null>(null);

	get hasPdf(): boolean {
		return this.loaded !== null;
	}

	/** The single visible signature-box placement, if the user added one. */
	get signaturePlacement(): Placement | null {
		return this.placements.find((p) => p.stampId === SIGNATURE_STAMP_ID) ?? null;
	}

	get selectedStamp(): Stamp | null {
		return this.stamps.find((s) => s.id === this.selectedStampId) ?? null;
	}

	getStamp(stampId: string): Stamp | undefined {
		return this.stamps.find((s) => s.id === stampId);
	}

	async refreshStamps(): Promise<void> {
		this.stamps = await listStamps();
		if (this.selectedStampId && !this.stamps.find((s) => s.id === this.selectedStampId)) {
			this.selectedStampId = null;
		}
		// Drop placements whose stamps were deleted.
		this.placements = this.placements.filter((p) => this.stamps.some((s) => s.id === p.stampId));
	}

	async loadPdfFromFile(file: File): Promise<void> {
		const buf = await file.arrayBuffer();
		await this.loadPdfFromBytes(new Uint8Array(buf), file.name, null);
	}

	async loadPdfFromBytes(
		pristine: Uint8Array,
		name: string,
		sourceDir: string | null
	): Promise<void> {
		this.loading = true;
		this.loadError = null;
		try {
			const loaded = await LoadedPdf.load(pristine);

			if (this.loaded) {
				await this.loaded.destroy().catch(() => undefined);
			}

			this.pdfFileName = name;
			this.pdfSourceDir = sourceDir;
			this.pdfBytes = pristine;
			this.loaded = loaded;
			this.pages = loaded.pages;
			this.placements = [];
			this.selectedPlacementId = null;
		} catch (err) {
			this.loadError = err instanceof Error ? err.message : String(err);
		} finally {
			this.loading = false;
		}
	}

	closePdf(): void {
		if (this.loaded) {
			void this.loaded.destroy().catch(() => undefined);
		}
		this.pdfFileName = null;
		this.pdfSourceDir = null;
		this.pdfBytes = null;
		this.loaded = null;
		this.pages = [];
		this.placements = [];
		this.selectedPlacementId = null;
		this.placingSignature = false;
	}

	selectStamp(id: string | null): void {
		this.selectedStampId = id;
		if (id !== null) this.placingSignature = false;
	}

	/** Enter "place the signature box" mode (next page click drops it). */
	startPlacingSignature(): void {
		this.placingSignature = true;
		this.selectedStampId = null;
	}

	/** Add (or replace) the single visible signature box on a page. */
	addSignaturePlacement(p: Omit<Placement, 'id' | 'stampId'>): Placement {
		this.placements = this.placements.filter((pl) => pl.stampId !== SIGNATURE_STAMP_ID);
		const placement = this.addPlacement({ ...p, stampId: SIGNATURE_STAMP_ID });
		this.placingSignature = false;
		return placement;
	}

	removeSignature(): void {
		const sig = this.signaturePlacement;
		if (sig) this.removePlacement(sig.id);
	}

	selectPlacement(id: string | null): void {
		this.selectedPlacementId = id;
	}

	addPlacement(p: Omit<Placement, 'id'>): Placement {
		const page = this.pages[p.pageIndex];
		const clamped = page
			? clampPlacement(p, page.width, page.height)
			: { x: p.x, y: p.y, width: p.width, height: p.height };
		const placement: Placement = {
			id: uuid(),
			stampId: p.stampId,
			pageIndex: p.pageIndex,
			...clamped,
			rotation: p.rotation
		};
		this.placements = [...this.placements, placement];
		this.selectedPlacementId = placement.id;
		return placement;
	}

	updatePlacement(id: string, patch: Partial<Omit<Placement, 'id' | 'stampId' | 'pageIndex'>>): void {
		this.placements = this.placements.map((p) => {
			if (p.id !== id) return p;
			const merged = { ...p, ...patch };
			const page = this.pages[p.pageIndex];
			if (!page) return merged;
			const clamped = clampPlacement(merged, page.width, page.height);
			return { ...merged, ...clamped };
		});
	}

	removePlacement(id: string): void {
		this.placements = this.placements.filter((p) => p.id !== id);
		if (this.selectedPlacementId === id) this.selectedPlacementId = null;
	}

	clearPlacements(): void {
		this.placements = [];
		this.selectedPlacementId = null;
	}

	placementsForPage(pageIndex: number): Placement[] {
		return this.placements.filter((p) => p.pageIndex === pageIndex);
	}
}

export const editor = new EditorState();
