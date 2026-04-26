import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess(),
	kit: {
		adapter: adapter({
			fallback: 'index.html',
			strict: false
		}),
		// Tauri serves the frontend over a custom protocol where the build dir
		// is the root, so absolute asset paths work — but emit relative URLs
		// where possible so the same build also works if served from a
		// subpath (e.g. from a static host).
		paths: {
			relative: true
		}
	}
};

export default config;
