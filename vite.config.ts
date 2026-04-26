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
	}
});
