<script lang="ts">
	// Boot veil: bottom-docked startup pill (BlazePod-style), not a fullscreen layer. Staged
	// eyebrow + label while the app boots ("Library / Restoring queue…"), checkmark + fade on
	// completion. An 8s safety net hides it no matter what; pointer-events-none so a stuck
	// node can never block input.
	import { auth, ui } from '$lib/player.svelte';
	import favicon from '$lib/assets/favicon.svg';
	import { browser } from '$app/environment';

	let timer: ReturnType<typeof setTimeout> | undefined;

	// Branded first-paint splash (black fullscreen): pulsing logo mark + app name
	// rising with .stagger-in, scale-out exit into the app. Skipped entirely under
	// prefers-reduced-motion — the loading pill below is the only boot UI there.
	const reducedMotion = $derived(
		browser && window.matchMedia?.('(prefers-reduced-motion: reduce)').matches
	);
	// First paint only: once the veil clears it never comes back this session.
	let splashed = $state(true);

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
	{#if splashed && !reducedMotion}
		<!-- Branded splash: black fullscreen, logo pulse, name rises via .stagger-in,
		     scale-out exit once the boot is done. pointer-events-none so it never blocks. -->
		<div
			class="bp-splash {ui.bootVeil.done ? 'leaving' : ''}"
			role="status"
			aria-live="polite"
			aria-hidden="false"
		>
			<img decoding="async" src={favicon} alt="" class="bp-splash-logo" aria-hidden="true" />
			<div class="bp-splash-name stagger-in" style="--stagger-i:0">Limusic</div>
			<div class="bp-splash-sub stagger-in" style="--stagger-i:1">{ui.bootVeil.label}</div>
		</div>
	{/if}
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
	.bp-splash {
		position: fixed;
		inset: 0;
		z-index: 100;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 14px;
		background: #000;
		pointer-events: none;
		transition: opacity 0.32s ease-out, transform 0.32s ease-out;
	}
	.bp-splash.leaving {
		opacity: 0;
		transform: scale(1.12);
	}
	.bp-splash-logo {
		width: 76px;
		height: 76px;
		animation: bp-splash-pulse 1.6s ease-in-out infinite;
	}
	@keyframes bp-splash-pulse {
		0%, 100% { transform: scale(1); opacity: 1; }
		50% { transform: scale(1.08); opacity: 0.82; }
	}
	.bp-splash-name {
		color: #fff;
		font-size: 30px;
		font-weight: 800;
		letter-spacing: 0.14em;
		text-transform: uppercase;
	}
	.bp-splash-sub {
		color: rgb(255 255 255 / 0.6);
		font-size: 12.5px;
		letter-spacing: 0.04em;
	}
	@media (prefers-reduced-motion: reduce) {
		.bp-splash, .bp-splash-logo { animation: none; transition: none; }
	}
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
