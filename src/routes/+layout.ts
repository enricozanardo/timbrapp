// pdfjs-dist is browser-only, so disable SSR for the entire app and
// use prerender to produce a static SPA shell via adapter-static.
export const ssr = false;
export const prerender = true;
export const trailingSlash = 'always';
