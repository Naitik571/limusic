<script lang="ts">
	// Ctrl+H: every way to drive playback — keyboard (rebindable), spatial nav, mouse
	// gestures and the gamepad map. Playback/app groups render live from the keymap, so a
	// rebound key shows up here the moment it lands.
	import { onDestroy } from 'svelte';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Button } from '$lib/components/ui/button';
	import { ui, toast } from '$lib/player.svelte';
	import {
		ACTION_LABELS,
		bindingsFor,
		describe,
		holderOf,
		resetBindings,
		setBinding,
		type ActionId,
		type Binding
	} from '$lib/keys';
	import { setSpatialEnabled, spatialEnabled } from '$lib/spatial';

	const PLAYBACK: ActionId[] = ['playpause', 'next', 'prev', 'mute'];
	const SEEK: ActionId[] = ['volup', 'voldown', 'seekback', 'seekfwd', 'seekback10', 'seekfwd10'];
	const APP: ActionId[] = ['palette', 'shortcuts', 'nowplaying'];

	// Re-render chip labels after any rebind (the map itself isn't reactive).
	let tick = $state(0);
	const slotsOf = (a: ActionId): Binding[] => (void tick, bindingsFor(a));

	let capturing = $state<{ action: ActionId; slot: number } | null>(null);
	let clashMsg = $state('');
	let forceArmedUntil = 0;
	let spatialOn = $state(spatialEnabled());

	function startCapture(action: ActionId, slot: number) {
		capturing = { action, slot };
		clashMsg = '';
		window.addEventListener('keydown', onCapture, true);
	}
	function stopCapture() {
		capturing = null;
		clashMsg = '';
		forceArmedUntil = 0;
		window.removeEventListener('keydown', onCapture, true);
	}

	// Closing the dialog abandons the capture too — otherwise the armed listener (and a
	// stale force window) survives behind the closed modal.
	$effect(() => {
		if (!ui.shortcutsOpen) stopCapture();
	});
	function onCapture(e: KeyboardEvent) {
		e.preventDefault();
		e.stopPropagation();
		if (!capturing) return;
		if (e.key === 'Escape') {
			stopCapture();
			return;
		}
		// Bare modifiers aren't bindings.
		if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) return;
		const b: Binding = { key: e.key.length === 1 ? e.key.toLowerCase() : e.key };
		if (e.ctrlKey || e.metaKey) b.ctrl = true;
		if (e.altKey) b.alt = true;
		if (e.shiftKey) b.shift = true;
		const force = Date.now() < forceArmedUntil;
		const res = setBinding(capturing.action, capturing.slot, b, force);
		if (res.ok) {
			forceArmedUntil = 0;
			tick++;
			stopCapture();
			toast.success(`Bound to ${describe(b)}`);
		} else {
			clashMsg = `Taken by ${ACTION_LABELS[res.holder]} — press the keys again to take it`;
			forceArmedUntil = Date.now() + 3000;
		}
	}
	onDestroy(stopCapture);

	function resetAll() {
		resetBindings();
		tick++;
		toast.success('Shortcuts reset to defaults');
	}
</script>

<Dialog.Root bind:open={ui.shortcutsOpen}>
	<Dialog.Content class="max-h-[80vh] overflow-y-auto sm:max-w-lg">
		<Dialog.Header>
			<Dialog.Title>Keyboard & controller shortcuts</Dialog.Title>
			<Dialog.Description>Everything playback responds to, on one page.</Dialog.Description>
		</Dialog.Header>
		<div class="grid gap-5 pb-2">
			{#snippet actionRow(action: ActionId)}
				<div class="flex items-center justify-between gap-4 px-3 py-1.5 text-sm">
					<div class="flex flex-wrap items-center gap-1.5">
						{#each slotsOf(action) as b, i (i)}
							<button
								class="cursor-pointer rounded border bg-muted px-1.5 py-0.5 font-mono text-[0.6875rem] font-medium whitespace-nowrap transition-colors hover:border-primary/60 {capturing?.action === action &&
								capturing.slot === i
									? 'border-primary text-primary'
									: ''}"
								title="Click, then press new keys"
								onclick={() => startCapture(action, i)}
							>
								{capturing?.action === action && capturing.slot === i
									? 'press keys…'
									: describe(b)}
							</button>
						{/each}
					</div>
					<span class="text-right text-muted-foreground">{ACTION_LABELS[action]}</span>
				</div>
			{/snippet}
			{#if capturing}
				<p class="rounded-lg border border-primary/40 bg-primary/10 px-3 py-2 text-xs" role="status">
					Press keys for <strong>{ACTION_LABELS[capturing.action]}</strong> (Esc cancels).{clashMsg
						? ` ${clashMsg}.`
						: ''}
				</p>
			{/if}
			<section>
				<div class="mb-2 flex items-center justify-between">
					<h3 class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Playback</h3>
					<Button size="sm" variant="ghost" onclick={resetAll}>Reset all</Button>
				</div>
				<div class="divide-y divide-border/60 rounded-lg border">
					{#each PLAYBACK as a (a)}
						{@render actionRow(a)}
					{/each}
					<div class="flex items-center justify-between gap-4 px-3 py-1.5 text-sm">
						<span class="text-muted-foreground">Click cover art · wheel over it for volume</span>
						<span class="text-right text-muted-foreground">Play / pause</span>
					</div>
				</div>
			</section>
			<section>
				<h3 class="mb-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
					Seek & volume
				</h3>
				<div class="divide-y divide-border/60 rounded-lg border">
					{#each SEEK as a (a)}
						{@render actionRow(a)}
					{/each}
					<div class="flex items-center justify-between gap-4 px-3 py-1.5 text-sm">
						<span class="text-muted-foreground">Drag the timeline</span>
						<span class="text-right text-muted-foreground">Seek anywhere</span>
					</div>
				</div>
			</section>
			<section>
				<h3 class="mb-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">App</h3>
				<div class="divide-y divide-border/60 rounded-lg border">
					{#each APP as a (a)}
						{@render actionRow(a)}
					{/each}
					<div class="flex items-center justify-between gap-4 px-3 py-1.5 text-sm">
						<span class="text-muted-foreground">Ctrl + &gt; / Ctrl + &lt; · Ctrl + +/-</span>
						<span class="text-right text-muted-foreground">Volume step · Interface zoom</span>
					</div>
					<div class="flex items-center justify-between gap-4 px-3 py-1.5 text-sm">
						<span class="text-muted-foreground">Alt + arrows</span>
						<span class="flex items-center gap-2 text-right text-muted-foreground">
							Spatial focus
							<button
								class="cursor-pointer rounded border px-1.5 py-0.5 font-mono text-[0.6875rem] {spatialOn
									? 'border-primary/60 text-primary'
									: ''}"
								aria-pressed={spatialOn}
								title="Move focus with Alt+Arrow keys"
								onclick={() => {
									spatialOn = !spatialOn;
									setSpatialEnabled(spatialOn);
								}}
							>
								{spatialOn ? 'ON' : 'OFF'}
							</button>
						</span>
					</div>
				</div>
			</section>
			<section>
				<h3 class="mb-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
					Gamepad (any controller)
				</h3>
				<div class="divide-y divide-border/60 rounded-lg border">
					{#each [
						['A · B · X · Y', 'Play-pause · Next · Previous · Mute'],
						['D-pad ↑↓ ←→', 'Volume · Seek ±10s'],
						['Left stick', 'Seek scrub (hold)'],
						['Right stick', 'Fast seek ±30s (hold)'],
						['LB / RB', 'Seek ∓10s'],
						['LT / RT', 'Volume ∓5'],
						['Start', 'Toggle mini player']
					] as [key, action] (key)}
						<div class="flex items-center justify-between gap-4 px-3 py-1.5 text-sm">
							<kbd
								class="rounded border bg-muted px-1.5 py-0.5 font-mono text-[0.6875rem] font-medium whitespace-nowrap"
							>
								{key}
							</kbd>
							<span class="text-right text-muted-foreground">{action}</span>
						</div>
					{/each}
				</div>
			</section>
		</div>
	</Dialog.Content>
</Dialog.Root>
