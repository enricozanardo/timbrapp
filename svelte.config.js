import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess(),
	kit: {
		adapter: adapter({
			// Keep the SPA fallback OFF of index.html. When fallback is
			// 'index.html', adapter-static overwrites the prerendered `/` page
			// with absolute `/_app/...` asset URLs. Tauri's custom protocol
			// then fails to load those assets on some macOS WebViews (notably
			// Intel), producing a white empty window. A separate fallback
			// leaves the prerendered index.html with relative `./_app/...`
			// paths that resolve correctly.
			fallback: '200.html',
			strict: false
		}),
		paths: {
			relative: true
		}
	}
};

export default config;
