// pdfjs-dist is browser-only, so disable SSR for the entire app and
// prerender `/` so adapter-static emits a real index.html with relative
// asset paths (see svelte.config.js — SPA fallback is a separate 200.html).
export const ssr = false;
export const prerender = true;
