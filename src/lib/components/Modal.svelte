<script lang="ts">
	import type { Snippet } from 'svelte';

	type Props = {
		open: boolean;
		title: string;
		onClose: () => void;
		body: Snippet;
		footer?: Snippet;
		/** When true, clicks on the backdrop close the dialog (default true). */
		dismissOnBackdrop?: boolean;
	};

	let {
		open,
		title,
		onClose,
		body,
		footer,
		dismissOnBackdrop = true
	}: Props = $props();

	function onBackdropClick(e: MouseEvent) {
		if (!dismissOnBackdrop) return;
		if (e.target === e.currentTarget) onClose();
	}

	function onKey(e: KeyboardEvent) {
		if (!open) return;
		if (e.key === 'Escape') {
			e.preventDefault();
			onClose();
		}
	}
</script>

<svelte:window onkeydown={onKey} />

{#if open}
	<div class="backdrop" role="presentation" onclick={onBackdropClick}>
		<div class="modal" role="dialog" aria-modal="true" aria-labelledby="modal-title">
			<header>
				<h3 id="modal-title">{title}</h3>
				<button type="button" class="x" onclick={onClose} aria-label="Close dialog">×</button>
			</header>
			<div class="body">
				{@render body()}
			</div>
			{#if footer}
				<footer>
					{@render footer()}
				</footer>
			{/if}
		</div>
	</div>
{/if}

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		background: rgba(15, 23, 42, 0.45);
		display: grid;
		place-items: center;
		padding: 1rem;
		z-index: 1000;
		animation: fade 120ms ease-out;
	}
	.modal {
		background: white;
		border-radius: 10px;
		box-shadow: 0 20px 60px rgba(15, 23, 42, 0.25);
		width: 100%;
		max-width: 460px;
		max-height: 90vh;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		animation: pop 140ms ease-out;
	}
	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.9rem 1.1rem;
		border-bottom: 1px solid #e2e8f0;
	}
	h3 {
		margin: 0;
		font-size: 1rem;
		font-weight: 600;
		color: #0f172a;
	}
	.x {
		appearance: none;
		border: 0;
		background: transparent;
		font-size: 1.5rem;
		line-height: 1;
		color: #64748b;
		cursor: pointer;
		padding: 0 0.25rem;
		border-radius: 4px;
	}
	.x:hover {
		color: #0f172a;
		background: #f1f5f9;
	}
	.body {
		padding: 1rem 1.1rem;
		overflow-y: auto;
		color: #1e293b;
		font-size: 0.95rem;
		line-height: 1.5;
	}
	footer {
		display: flex;
		justify-content: flex-end;
		gap: 0.5rem;
		padding: 0.75rem 1.1rem;
		border-top: 1px solid #e2e8f0;
		background: #f8fafc;
	}
	@keyframes fade {
		from {
			opacity: 0;
		}
		to {
			opacity: 1;
		}
	}
	@keyframes pop {
		from {
			opacity: 0;
			transform: translateY(8px) scale(0.98);
		}
		to {
			opacity: 1;
			transform: translateY(0) scale(1);
		}
	}
</style>
