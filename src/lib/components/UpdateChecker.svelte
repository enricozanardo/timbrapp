<script lang="ts">
	import { onMount } from 'svelte';
	import Modal from './Modal.svelte';
	import { updater, isLinux } from '$lib/stores/updater.svelte';

	const progressPct = $derived.by(() => {
		const s = updater.state;
		if (s.kind !== 'installing') return null;
		if (!s.total || s.total <= 0) return null;
		return Math.min(100, Math.round((s.downloaded / s.total) * 100));
	});

	onMount(() => {
		// Only auto-check on macOS/Windows — disabled on Linux.
		if (isLinux()) return;
		const t = setTimeout(() => updater.check(true), 3000);
		return () => clearTimeout(t);
	});
</script>

{#if updater.state.kind === 'available'}
	<aside class="banner" role="status" aria-live="polite">
		<div class="content">
			<strong>Update available</strong>
			<span class="versions">v{updater.state.version}</span>
		</div>
		<div class="actions">
			<button type="button" class="btn ghost" onclick={() => updater.dismiss()}>Later</button>
			<button type="button" class="btn primary" onclick={() => updater.install()}>Install</button>
		</div>
	</aside>
{/if}

<Modal
	open={updater.state.kind === 'installing' || updater.state.kind === 'restarting'}
	title={updater.state.kind === 'restarting' ? 'Restarting…' : 'Installing update'}
	onClose={() => {}}
	dismissOnBackdrop={false}
>
	{#snippet body()}
		{#if updater.state.kind === 'installing'}
			<p>Downloading <strong>v{updater.state.version}</strong>… The app will restart automatically.</p>
			<div class="bar"><div class="fill" style="width:{progressPct ?? 40}%"></div></div>
			<p class="bytes">{(updater.state.downloaded/1024/1024).toFixed(1)} MB{updater.state.total ? ` / ${(updater.state.total/1024/1024).toFixed(1)} MB` : ''}</p>
		{:else if updater.state.kind === 'restarting'}
			<p>Update v{updater.state.version} installed. Relaunching…</p>
		{/if}
	{/snippet}
</Modal>

<Modal open={updater.state.kind === 'error'} title="Update failed" onClose={() => updater.close()}>
	{#snippet body()}
		{#if updater.state.kind === 'error'}
			<p>The update couldn't be installed:</p>
			<pre class="err">{updater.state.message}</pre>
		{/if}
	{/snippet}
	{#snippet footer()}
		<button type="button" class="btn primary" onclick={() => updater.close()}>Close</button>
	{/snippet}
</Modal>

<style>
	.banner{position:fixed;bottom:1rem;right:1rem;max-width:360px;background:white;border:1px solid #e2e8f0;border-radius:10px;box-shadow:0 12px 30px rgba(15,23,42,.18);padding:.9rem 1rem;z-index:900;}
	.content{display:grid;gap:.25rem;margin-bottom:.7rem;}
	.content strong{color:#0f172a;font-size:.95rem;}
	.versions{font-size:.8rem;color:#2563eb;font-family:monospace;}
	.actions{display:flex;justify-content:flex-end;gap:.5rem;}
	.btn{appearance:none;border:1px solid #cbd5e1;background:white;border-radius:6px;padding:.4rem .9rem;font-size:.85rem;cursor:pointer;color:#1e293b;}
	.btn.primary{background:#2563eb;color:white;border-color:transparent;}
	.btn.ghost{background:transparent;color:#64748b;border-color:transparent;}
	.bar{height:8px;background:#e2e8f0;border-radius:999px;overflow:hidden;margin:.6rem 0 .4rem;}
	.fill{height:100%;background:linear-gradient(90deg,#3b82f6,#2563eb);transition:width 200ms ease-out;}
	.bytes{margin:0;font-size:.8rem;color:#64748b;font-family:monospace;}
	.err{background:#fef2f2;border:1px solid #fecaca;border-radius:6px;padding:.5rem .7rem;font-size:.8rem;color:#991b1b;white-space:pre-wrap;}
</style>
