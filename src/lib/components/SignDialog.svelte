<script lang="ts">
	import Modal from './Modal.svelte';
	import { editor } from '$lib/stores/editor.svelte';
	import { buildStampedPdf, savePdf, signedFilename } from '$lib/pdf/save';
	import * as cie from '$lib/cie/api';
	import type { CertificateInfo, CieStatus } from '$lib/types';

	type Props = {
		open: boolean;
		onClose: () => void;
	};
	let { open, onClose }: Props = $props();

	const available = cie.isCieAvailable();

	let modulePath = $state('');
	let status = $state<CieStatus | null>(null);
	let selectedSlot = $state<number | null>(null);
	let certs = $state<CertificateInfo[]>([]);
	let selectedCertId = $state<string | null>(null);
	let pin = $state('');
	let reason = $state('');
	let location = $state('');

	// Enrollment ("Abilita CIE") state. `enrolled` is null while unknown.
	let enrolled = $state<boolean | null>(null);
	let checkingEnroll = $state(false);
	let enrollPin = $state('');
	let enrolling = $state(false);

	let scanning = $state(false);
	let loadingCerts = $state(false);
	let busy = $state(false);
	let error = $state<string | null>(null);
	let notice = $state<string | null>(null);

	// Re-scan whenever the dialog opens.
	$effect(() => {
		if (open && available) {
			void scan();
		}
	});

	function modulePathArg(): string | undefined {
		return modulePath.trim() ? modulePath.trim() : undefined;
	}

	async function scan() {
		scanning = true;
		error = null;
		notice = null;
		certs = [];
		selectedCertId = null;
		enrolled = null;
		enrollPin = '';
		try {
			status = await cie.cieStatus(modulePathArg());
			if (status.readers.length > 0) {
				selectedSlot = status.readers[0].slotId;
				await checkEnrolled();
			} else {
				selectedSlot = null;
			}
		} catch (err) {
			error = messageOf(err);
		} finally {
			scanning = false;
		}
	}

	/** PIN-free check: is the selected card already paired? Loads certs if so. */
	async function checkEnrolled() {
		if (selectedSlot == null) return;
		checkingEnroll = true;
		error = null;
		certs = [];
		selectedCertId = null;
		try {
			enrolled = await cie.isEnrolled(selectedSlot, modulePathArg());
			if (enrolled) {
				await loadCerts();
			}
		} catch (err) {
			error = messageOf(err);
			enrolled = null;
		} finally {
			checkingEnroll = false;
		}
	}

	async function doEnroll() {
		if (!/^\d{8}$/.test(enrollPin)) {
			error = 'Enter the full 8-digit CIE PIN to enroll this card.';
			return;
		}
		enrolling = true;
		error = null;
		notice = null;
		try {
			const res = await cie.enroll(enrollPin, modulePathArg());
			enrollPin = '';
			if (res.ok) {
				enrolled = true;
				notice = res.cardholder
					? `Card enrolled: ${res.cardholder}. You can now sign with the last 4 digits of your PIN.`
					: res.message;
				await loadCerts();
			} else {
				enrolled = false;
				error =
					res.consumedAttempt && res.attemptsLeft != null
						? `${res.message} (${res.attemptsLeft} attempt(s) left before the card locks)`
						: res.message;
			}
		} catch (err) {
			error = messageOf(err);
		} finally {
			enrolling = false;
		}
	}

	async function loadCerts() {
		if (selectedSlot == null) return;
		loadingCerts = true;
		error = null;
		try {
			certs = await cie.listCertificates(selectedSlot, modulePathArg());
			// Prefer the FEA/signing certificate.
			const preferred = certs.find((c) => c.keyUsageSign) ?? certs[0];
			selectedCertId = preferred ? preferred.idHex : null;
		} catch (err) {
			error = messageOf(err);
		} finally {
			loadingCerts = false;
		}
	}

	async function onSlotChange(e: Event) {
		selectedSlot = Number((e.target as HTMLSelectElement).value);
		enrolled = null;
		enrollPin = '';
		await checkEnrolled();
	}

	async function testKey() {
		if (selectedSlot == null || !selectedCertId || !pin) return;
		busy = true;
		error = null;
		notice = null;
		try {
			const sample = crypto.getRandomValues(new Uint8Array(32));
			const sig = await cie.signRaw({
				slotId: selectedSlot,
				certIdHex: selectedCertId,
				pin,
				data: sample,
				modulePath: modulePathArg()
			});
			notice = `PIN accepted. Card produced a ${sig.length}-byte signature.`;
		} catch (err) {
			error = messageOf(err);
		} finally {
			busy = false;
		}
	}

	async function sign() {
		if (selectedSlot == null || !selectedCertId || !pin || !editor.pdfBytes) return;
		busy = true;
		error = null;
		notice = null;
		try {
			// Flatten any placed stamps into the PDF first, then sign the result.
			const stamped = await buildStampedPdf({
				originalBytes: editor.pdfBytes,
				placements: editor.placements,
				stamps: editor.stamps
			});
			const signed = await cie.signPdf({
				slotId: selectedSlot,
				certIdHex: selectedCertId,
				pin,
				pdf: stamped,
				reason: reason.trim() || undefined,
				location: location.trim() || undefined,
				modulePath: modulePathArg()
			});
			const path = await savePdf(signed, signedFilename(editor.pdfFileName), editor.pdfSourceDir);
			notice = path
				? `Document signed with CIE (PAdES) and saved to ${path}`
				: 'Document signed with CIE (PAdES) and downloaded.';
			pin = '';
		} catch (err) {
			error = messageOf(err);
		} finally {
			busy = false;
		}
	}

	function messageOf(err: unknown): string {
		if (typeof err === 'string') return err;
		if (err instanceof Error) return err.message;
		return String(err);
	}

	const canSign = $derived(
		available &&
			enrolled === true &&
			selectedSlot != null &&
			!!selectedCertId &&
			/^\d{4}$/.test(pin) &&
			!busy
	);

	const canEnroll = $derived(
		available && selectedSlot != null && /^\d{8}$/.test(enrollPin) && !enrolling && !busy
	);
</script>

<Modal {open} title="Sign with CIE (PAdES)" {onClose} dismissOnBackdrop={!busy}>
	{#snippet body()}
		{#if !available}
			<p class="muted">
				CIE signing needs a smart-card reader and is only available in the TimbrApp desktop app.
				Open TimbrApp as the installed desktop application to sign with your CIE.
			</p>
		{:else}
			<div class="field">
				<label for="cie-module">PKCS#11 module path (optional)</label>
				<div class="row">
					<input
						id="cie-module"
						type="text"
						placeholder="auto-detect (leave empty)"
						bind:value={modulePath}
						disabled={busy}
					/>
					<button type="button" class="btn" onclick={scan} disabled={scanning || busy}>
						{scanning ? 'Scanning…' : 'Rescan'}
					</button>
				</div>
				{#if status}
					<p class="status" class:ok={status.readers.length > 0}>{status.message}</p>
					{#if status.modulePath}
						<p class="muted small">Module: {status.modulePath}</p>
					{/if}
				{/if}
			</div>

			{#if status && status.readers.length > 0}
				<div class="field">
					<label for="cie-slot">Reader</label>
					<select id="cie-slot" onchange={onSlotChange} disabled={busy}>
						{#each status.readers as r (r.slotId)}
							<option value={r.slotId} selected={r.slotId === selectedSlot}>
								{r.slotDescription || r.manufacturer || `slot ${r.slotId}`}
								{r.tokenLabel ? `— ${r.tokenLabel}` : ''}
							</option>
						{/each}
					</select>
				</div>

				{#if checkingEnroll}
					<p class="muted small">Checking whether this card is enrolled…</p>
				{:else if enrolled === false}
					<div class="field">
						<p class="status">
							This CIE isn't enrolled on this computer yet. Enrollment pairs the card once (a
							one-time step) so it can sign — it needs your <strong>full 8-digit PIN</strong>.
						</p>
						<label for="cie-enroll-pin">Full CIE PIN (8 digits)</label>
						<input
							id="cie-enroll-pin"
							type="password"
							inputmode="numeric"
							maxlength="8"
							autocomplete="off"
							bind:value={enrollPin}
							disabled={enrolling}
							placeholder="8-digit PIN (first 4 + last 4)"
						/>
						<p class="muted small">
							Used only once, locally, to pair this card. A wrong PIN uses one of the 3 attempts
							(then the PUK is required to unblock).
						</p>
					</div>
				{:else if enrolled === true}
					<div class="field">
						<label for="cie-cert">Certificate</label>
						{#if loadingCerts}
							<p class="muted small">Reading certificates…</p>
						{:else if certs.length === 0}
							<p class="muted small">No certificates found on this card.</p>
						{:else}
							<select id="cie-cert" bind:value={selectedCertId} disabled={busy}>
								{#each certs as c (c.idHex)}
									<option value={c.idHex}>
										{c.keyUsageSign ? '★ ' : ''}{c.subject || c.label || c.idHex}
									</option>
								{/each}
							</select>
							{#if selectedCertId}
								{@const cert = certs.find((c) => c.idHex === selectedCertId)}
								{#if cert}
									<p class="muted small">
										Issuer: {cert.issuer || '—'} · valid until {cert.notAfter || '—'}
									</p>
								{/if}
							{/if}
						{/if}
					</div>

					<div class="field">
						<label for="cie-pin">CIE PIN (last 4 digits)</label>
						<input
							id="cie-pin"
							type="password"
							inputmode="numeric"
							maxlength="4"
							autocomplete="off"
							bind:value={pin}
							disabled={busy}
							placeholder="last 4 digits"
						/>
						<p class="muted small">
							This card is paired, so only the last 4 digits are needed. The PIN is sent only to
							the local card. Three wrong attempts block the card (PUK needed).
						</p>
					</div>

					<details>
						<summary>Optional signature metadata</summary>
						<div class="field">
							<label for="cie-reason">Reason</label>
							<input id="cie-reason" type="text" bind:value={reason} disabled={busy} />
						</div>
						<div class="field">
							<label for="cie-location">Location</label>
							<input id="cie-location" type="text" bind:value={location} disabled={busy} />
						</div>
					</details>
				{/if}
			{/if}

			{#if notice}
				<p class="ok-box" role="status">{notice}</p>
			{/if}
			{#if error}
				<p class="err-box" role="alert">{error}</p>
			{/if}
		{/if}
	{/snippet}

	{#snippet footer()}
		<button type="button" class="btn ghost" onclick={onClose} disabled={busy || enrolling}>
			Close
		</button>
		{#if available && status && status.readers.length > 0}
			{#if enrolled === false}
				<button type="button" class="btn primary" onclick={doEnroll} disabled={!canEnroll}>
					{enrolling ? 'Enrolling…' : 'Enroll CIE'}
				</button>
			{:else if enrolled === true}
				<button type="button" class="btn" onclick={testKey} disabled={!canSign}> Test PIN </button>
				<button
					type="button"
					class="btn primary"
					onclick={sign}
					disabled={!canSign || !editor.hasPdf}
				>
					{busy ? 'Signing…' : 'Sign & download'}
				</button>
			{/if}
		{/if}
	{/snippet}
</Modal>

<style>
	.field {
		margin-bottom: 0.9rem;
	}
	label {
		display: block;
		font-size: 0.8rem;
		font-weight: 600;
		color: #334155;
		margin-bottom: 0.3rem;
	}
	input[type='text'],
	input[type='password'],
	select {
		width: 100%;
		box-sizing: border-box;
		padding: 0.45rem 0.55rem;
		border: 1px solid #cbd5e1;
		border-radius: 6px;
		font-size: 0.9rem;
		color: #0f172a;
		background: white;
	}
	.row {
		display: flex;
		gap: 0.5rem;
	}
	.row input {
		flex: 1;
	}
	.muted {
		color: #64748b;
	}
	.small {
		font-size: 0.78rem;
		margin: 0.3rem 0 0;
	}
	.status {
		font-size: 0.82rem;
		margin: 0.4rem 0 0;
		color: #b45309;
	}
	.status.ok {
		color: #15803d;
	}
	details {
		margin-bottom: 0.9rem;
	}
	summary {
		cursor: pointer;
		font-size: 0.82rem;
		color: #475569;
		margin-bottom: 0.5rem;
	}
	.ok-box {
		background: #dcfce7;
		color: #166534;
		border: 1px solid #86efac;
		border-radius: 6px;
		padding: 0.5rem 0.65rem;
		font-size: 0.85rem;
	}
	.err-box {
		background: #fee2e2;
		color: #991b1b;
		border: 1px solid #fca5a5;
		border-radius: 6px;
		padding: 0.5rem 0.65rem;
		font-size: 0.85rem;
	}
	.btn {
		appearance: none;
		border: 1px solid #cbd5e1;
		background: white;
		border-radius: 6px;
		padding: 0.4rem 0.7rem;
		font-size: 0.85rem;
		cursor: pointer;
		color: #1e293b;
	}
	.btn:hover:not(:disabled) {
		background: #f1f5f9;
	}
	.btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
	.btn.primary {
		background: #2563eb;
		color: white;
		border-color: transparent;
	}
	.btn.primary:hover:not(:disabled) {
		background: #1d4ed8;
	}
	.btn.ghost {
		background: transparent;
		color: #475569;
		border-color: transparent;
	}
</style>
