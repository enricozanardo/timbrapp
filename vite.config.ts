import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [sveltekit()],
	optimizeDeps: {
		// pdfjs-dist ships an .mjs worker; pre-bundle the lib for dev speed.
		include: ['pdfjs-dist', 'pdf-lib', 'idb', 'uuid']
	},
	worker: {
		format: 'es'
	},
	build: {
		// Transpile modern syntax (e.g. private class fields) so older
		// macOS WebViews on Intel Macs can parse the bundle. Safari 14 ≈
		// Big Sur; Apple Silicon Macs never ship below that.
		target: ['es2020', 'safari14']
	}
});
