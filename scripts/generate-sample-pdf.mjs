import { PDFDocument, StandardFonts, rgb } from 'pdf-lib';
import { writeFileSync } from 'node:fs';

const doc = await PDFDocument.create();
const font = await doc.embedFont(StandardFonts.Helvetica);

for (let i = 1; i <= 3; i++) {
	const page = doc.addPage([595.28, 841.89]);
	page.setFont(font);
	page.drawText(`timbrapp sample document — page ${i} of 3`, {
		x: 50,
		y: 780,
		size: 18,
		color: rgb(0.05, 0.1, 0.4)
	});
	for (let line = 0; line < 25; line++) {
		page.drawText(`Line ${line + 1}: lorem ipsum dolor sit amet, consectetur adipiscing elit.`, {
			x: 50,
			y: 740 - line * 24,
			size: 11,
			color: rgb(0.15, 0.15, 0.15)
		});
	}
	page.drawRectangle({
		x: 40,
		y: 40,
		width: 515,
		height: 760,
		borderColor: rgb(0.6, 0.6, 0.6),
		borderWidth: 1
	});
}

const bytes = await doc.save();
writeFileSync('static/sample.pdf', bytes);
console.log(`Wrote static/sample.pdf (${bytes.length} bytes, 3 pages).`);
