import * as pdfjs from 'pdfjs-dist';
import type { PDFDocumentProxy, PDFPageProxy, RenderTask } from 'pdfjs-dist';
import workerUrl from 'pdfjs-dist/build/pdf.worker.mjs?url';
import type { PageMeta } from '../types';

let workerConfigured = false;

function ensureWorker(): void {
	if (workerConfigured) return;
	pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;
	workerConfigured = true;
}

/**
 * A loaded PDF, ready to render its pages and expose their natural sizes
 * (in PDF points). Holds the underlying `PDFDocumentProxy`; call `destroy`
 * when done.
 */
export class LoadedPdf {
	readonly pages: PageMeta[];

	private constructor(
		readonly doc: PDFDocumentProxy,
		pages: PageMeta[]
	) {
		this.pages = pages;
	}

	static async load(bytes: ArrayBuffer | Uint8Array): Promise<LoadedPdf> {
		ensureWorker();
		// pdfjs may take ownership of the provided buffer, so always pass a copy
		// to avoid corrupting the editor's pristine copy used later by pdf-lib.
		const data = bytes instanceof Uint8Array ? bytes.slice() : new Uint8Array(bytes).slice();
		const task = pdfjs.getDocument({ data });
		const doc = await task.promise;
		const pages: PageMeta[] = [];
		for (let i = 1; i <= doc.numPages; i++) {
			const page = await doc.getPage(i);
			const vp = page.getViewport({ scale: 1 });
			pages.push({ pageIndex: i - 1, width: vp.width, height: vp.height });
			page.cleanup();
		}
		return new LoadedPdf(doc, pages);
	}

	get pageCount(): number {
		return this.pages.length;
	}

	getPage(pageIndex: number): Promise<PDFPageProxy> {
		return this.doc.getPage(pageIndex + 1);
	}

	/**
	 * Render a page into the provided canvas at the given CSS scale.
	 * The canvas is resized in both physical (HiDPI-aware) and CSS pixels.
	 *
	 * Returns the active `RenderTask` so the caller can `cancel()` it on cleanup.
	 */
	async renderPage(pageIndex: number, canvas: HTMLCanvasElement, scale: number): Promise<RenderTask> {
		const page = await this.getPage(pageIndex);
		const dpr = typeof window === 'undefined' ? 1 : window.devicePixelRatio || 1;
		const viewport = page.getViewport({ scale: scale * dpr });
		const cssViewport = page.getViewport({ scale });

		canvas.width = Math.floor(viewport.width);
		canvas.height = Math.floor(viewport.height);
		canvas.style.width = `${Math.floor(cssViewport.width)}px`;
		canvas.style.height = `${Math.floor(cssViewport.height)}px`;

		const task = page.render({ canvas, viewport });
		return task;
	}

	async destroy(): Promise<void> {
		await this.doc.destroy();
	}
}
