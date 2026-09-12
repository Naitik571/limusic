<script lang="ts">
	// Boot veil: bottom-docked startup pill (BlazePod-style), not a fullscreen layer. Staged
	// eyebrow + label while the app boots ("Library / Restoring queue…"), checkmark + fade on
	// completion. An 8s safety net hides it no matter what; pointer-events-none so a stuck
	// node can never block input.
	import { auth, ui } from '$lib/player.svelte';

	let timer: ReturnType<typeof setTimeout> | undefined;

	// Onboarding (interior #1): while the signed-out hero card waits for a choice, the boot
	// pill stands down — the card owns the visitor's attention, and a pill over it reads as
	// a second thing to answer. Signed-in boots are untouched.

	$effect(() => {
		// Re-arm the safety net on every stage change: a progressing boot keeps its pill.
		ui.bootVeil;
		if (timer) clearTimeout(timer);
		if (ui.bootVeil && !ui.bootVeil.done) {
			timer = setTimeout(() => {
				ui.bootVeil = null;
			}, 8000);
		}
		return () => {
			if (timer) clearTimeout(timer);
		};
	});

	const onboardingHold = $derived(!!auth.account && !auth.account.signedIn && !ui.welcomed);
</script>

{#if ui.bootVeil && !onboardingHold}
	<div
		class="bp-boot-veil {ui.bootVeil.done ? 'complete' : ''}"
		role="status"
		aria-live="polite"
		aria-hidden="false"
	>
		<div class="bp-boot-pill">
			{#if ui.bootVeil.done}
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M20 6 9 17l-5-5" /></svg>
			{:else}
				<span class="bp-boot-spinner" aria-hidden="true"></span>
			{/if}
			<span class="bp-boot-meta">
				<span class="bp-boot-eyebrow">{ui.bootVeil.eyebrow}</span>
				<span class="bp-boot-label">{ui.bootVeil.label}</span>
			</span>
		</div>
	</div>
{/if}

<style>
	.bp-boot-veil {
		position: fixed;
		left: 50%;
		bottom: max(18px, env(safe-area-inset-bottom));
		transform: translateX(-50%);
		z-index: 80;
		pointer-events: none;
		transition: opacity 0.3s ease-out;
	}
	.bp-boot-veil.complete {
		opacity: 0;
	}
	.bp-boot-pill {
		display: flex;
		align-items: center;
		gap: 10px;
		max-width: min(340px, calc(100vw - 32px));
		padding: 10px 16px;
		border-radius: 16px;
		background: var(--popover, #fff);
		border: 1px solid var(--border, #e7e4de);
		box-shadow: 0 12px 32px rgb(0 0 0 / 0.18);
	}
	.bp-boot-pill svg {
		width: 16px;
		height: 16px;
		flex: none;
		color: var(--primary, #e0402a);
	}
	.bp-boot-spinner {
		width: 16px;
		height: 16px;
		flex: none;
		border-radius: 50%;
		border: 2px solid var(--border, #c9c6bf);
		border-top-color: var(--primary, #e0402a);
		animation: bp-boot-spin 0.9s linear infinite;
	}
	@keyframes bp-boot-spin {
		to {
			transform: rotate(360deg);
		}
	}
	.bp-boot-meta {
		display: flex;
		flex-direction: column;
		gap: 1px;
		flex: 1;
		min-width: 0;
	}
	.bp-boot-eyebrow {
		font-family: var(--font-mono, ui-monospace, monospace);
		font-size: 9.5px;
		font-weight: 500;
		letter-spacing: 0.1em;
		text-transform: uppercase;
		color: var(--muted-foreground, #75726c);
	}
	.bp-boot-label {
		font-size: 13px;
		font-weight: 600;
		letter-spacing: 0.04em;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
</style>
