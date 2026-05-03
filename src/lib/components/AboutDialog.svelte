<script lang="ts">
	import Modal from './Modal.svelte';
	import { APP_VERSION } from '$lib/version';

	type Props = {
		open: boolean;
		onClose: () => void;
	};

	let { open, onClose }: Props = $props();
</script>

<Modal {open} title="About timbrapp" {onClose}>
	{#snippet body()}
		<p class="lead">
			Apply PNG stamps and signatures to PDF documents — entirely on your machine.
			No backend, nothing uploaded.
		</p>

		<dl class="meta">
			<dt>Version</dt>
			<dd>{APP_VERSION}</dd>

			<dt>Developer</dt>
			<dd>Enrico Zanardo</dd>

			<dt>Contact</dt>
			<dd>
				<span class="email" title="Replace [at] with @">ezanardo[at]stellab.it</span>
			</dd>

			<dt>Source</dt>
			<dd>
				<a href="https://github.com/enricozanardo/timbrapp" target="_blank" rel="noopener">
					github.com/enricozanardo/timbrapp
				</a>
			</dd>

			<dt>License</dt>
			<dd>MIT</dd>
		</dl>

		<section class="updater">
			<a
				href="https://github.com/enricozanardo/timbrapp/releases/latest"
				target="_blank"
				rel="noopener"
				class="btn"
			>
				Check releases page
			</a>
		</section>

		<p class="credits">
			Built with <a href="https://svelte.dev/" target="_blank" rel="noopener">Svelte 5</a>,
			<a href="https://v2.tauri.app/" target="_blank" rel="noopener">Tauri 2</a>,
			<a href="https://github.com/mozilla/pdf.js" target="_blank" rel="noopener">pdf.js</a>
			and <a href="https://pdf-lib.js.org/" target="_blank" rel="noopener">pdf-lib</a>.
		</p>
	{/snippet}
	{#snippet footer()}
		<button type="button" class="btn primary" onclick={onClose}>Close</button>
	{/snippet}
</Modal>

<style>
	.lead {
		margin: 0 0 1rem;
		color: #334155;
	}
	.meta {
		display: grid;
		grid-template-columns: max-content 1fr;
		gap: 0.4rem 0.9rem;
		margin: 0 0 1rem;
		font-size: 0.9rem;
	}
	dt {
		color: #64748b;
		font-weight: 500;
	}
	dd {
		margin: 0;
		color: #0f172a;
	}
	dd a {
		color: #2563eb;
		text-decoration: none;
	}
	dd a:hover {
		text-decoration: underline;
	}
	.email {
		font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
		background: #f1f5f9;
		padding: 0.1rem 0.4rem;
		border-radius: 4px;
		font-size: 0.85rem;
		user-select: all;
	}
	.updater {
		margin: 0 0 1rem;
		padding: 0.6rem 0.7rem;
		background: #f8fafc;
		border: 1px solid #e2e8f0;
		border-radius: 6px;
	}
	.credits {
		margin: 0;
		font-size: 0.8rem;
		color: #64748b;
		line-height: 1.5;
	}
	.credits a {
		color: #475569;
	}
	.btn {
		appearance: none;
		border: 1px solid #cbd5e1;
		background: white;
		border-radius: 6px;
		padding: 0.45rem 0.9rem;
		font-size: 0.9rem;
		cursor: pointer;
		color: #1e293b;
	}
	.btn:disabled {
		opacity: 0.6;
		cursor: not-allowed;
	}
	.btn.primary {
		background: #2563eb;
		color: white;
		border-color: transparent;
	}
	.btn.primary:hover {
		background: #1d4ed8;
	}
</style>
