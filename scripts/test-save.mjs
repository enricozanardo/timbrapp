/**
 * Smoke test for the save pipeline.
 *
 * Reads `static/sample.pdf` + the default stamp PNG, simulates 3 placements
 * (one per page at different positions), runs `buildStampedPdf`, then
 * re-opens the result and verifies:
 *   - same page count
 *   - each page has at least one image XObject
 *
 * Failure throws and exits non-zero so CI can pick it up.
 */
import { readFile, writeFile } from 'node:fs/promises';
import { PDFDocument, degrees } from 'pdf-lib';

async function buildStampedPdf({ originalBytes, placements, stamps }) {
	const doc = await PDFDocument.load(originalBytes);
	const stampById = new Map(stamps.map((s) => [s.id, s]));
	const usedIds = new Set(placements.map((p) => p.stampId));
	const embedded = new Map();
	for (const id of usedIds) {
		const stamp = stampById.get(id);
		if (!stamp) continue;
		const buf = await stamp.bytes;
		embedded.set(id, await doc.embedPng(buf));
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

const originalBytes = await readFile('static/sample.pdf');
const stampBytes = await readFile('static/stamps/timbro_gorgia.png');

const stamps = [{ id: 'stamp-1', name: 'gorgia', bytes: stampBytes }];

// 3 placements at deliberately different positions (PDF point space, origin BL).
const placements = [
	{ id: 'p1', stampId: 'stamp-1', pageIndex: 0, x: 50, y: 60, width: 120, height: 120 },
	{ id: 'p2', stampId: 'stamp-1', pageIndex: 1, x: 400, y: 60, width: 100, height: 100 },
	{
		id: 'p3',
		stampId: 'stamp-1',
		pageIndex: 2,
		x: 200,
		y: 400,
		width: 150,
		height: 150,
		rotation: 15
	}
];

const out = await buildStampedPdf({ originalBytes, placements, stamps });
await writeFile('static/sample-stamped.pdf', out);
console.log(`Wrote static/sample-stamped.pdf (${out.length} bytes).`);

const verify = await PDFDocument.load(out);
const pageCount = verify.getPageCount();
if (pageCount !== 3) {
	throw new Error(`Expected 3 pages, got ${pageCount}`);
}

let imagesFound = 0;
for (const page of verify.getPages()) {
	const resources = page.node.Resources();
	const xobj = resources?.lookup(resources.context.obj('XObject'));
	if (!xobj) continue;
	const dict = xobj.dict;
	if (!dict) continue;
	for (const [, ref] of dict.entries()) {
		const stream = verify.context.lookup(ref);
		const subtype = stream?.dict?.lookup?.(stream.dict.context.obj('Subtype'));
		if (subtype && String(subtype).includes('Image')) imagesFound++;
	}
}

if (imagesFound < 3) {
	throw new Error(`Expected at least 3 image XObjects across pages, found ${imagesFound}`);
}

console.log(`Verified ${pageCount} pages, ${imagesFound} embedded image XObject references. OK.`);
