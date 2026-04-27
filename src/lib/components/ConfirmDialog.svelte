<script lang="ts">
	import Modal from './Modal.svelte';

	type Props = {
		open: boolean;
		title: string;
		message: string;
		confirmLabel?: string;
		cancelLabel?: string;
		/** Style the confirm button as a destructive action (red). */
		danger?: boolean;
		onConfirm: () => void;
		onCancel: () => void;
	};

	let {
		open,
		title,
		message,
		confirmLabel = 'Confirm',
		cancelLabel = 'Cancel',
		danger = false,
		onConfirm,
		onCancel
	}: Props = $props();
</script>

<Modal {open} {title} onClose={onCancel}>
	{#snippet body()}
		<p class="msg">{message}</p>
	{/snippet}
	{#snippet footer()}
		<button type="button" class="btn" onclick={onCancel}>{cancelLabel}</button>
		<button
			type="button"
			class="btn primary"
			class:danger
			onclick={onConfirm}
		>
			{confirmLabel}
		</button>
	{/snippet}
</Modal>

<style>
	.msg {
		margin: 0;
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
	.btn:hover {
		background: #f1f5f9;
	}
	.btn.primary {
		background: #2563eb;
		color: white;
		border-color: transparent;
	}
	.btn.primary:hover {
		background: #1d4ed8;
	}
	.btn.primary.danger {
		background: #dc2626;
	}
	.btn.primary.danger:hover {
		background: #b91c1c;
	}
</style>
