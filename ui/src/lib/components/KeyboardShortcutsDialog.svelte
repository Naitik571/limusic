<script lang="ts">
	// Ctrl+H: one dialog listing EVERY way to drive playback — keyboard, mouse gestures and the
	// gamepad map. Static data (the bindings are fixed in code), so this is pure presentation.
	import * as Dialog from '$lib/components/ui/dialog';
	import { ui } from '$lib/player.svelte';

	const groups: { title: string; rows: [string, string][] }[] = [
		{
			title: 'Playback',
			rows: [
				['Space / K', 'Play or pause'],
				['Shift + N', 'Next track'],
				['Shift + P', 'Previous track'],
				['M', 'Mute (restores previous level)'],
				['Click cover art', 'Play / pause'],
				['Wheel over cover art (maximized)', 'Volume up / down']
			]
		},
		{
			title: 'Seek & volume',
			rows: [
				['↑ / ↓', 'Volume ±5'],
				['← / →', 'Seek ∓5s'],
				['J / L', 'Seek ∓10s'],
				['Drag the timeline', 'Seek anywhere']
			]
		},
		{
			title: 'App',
			rows: [
				['Ctrl + K', 'Quick search from anywhere'],
				['Ctrl + E', 'Show / hide the now-playing view'],
				['Ctrl + > / Ctrl + <', 'Volume step'],
				['Ctrl + H', 'This list'],
				['Ctrl + +/-', 'Interface zoom']
			]
		},
		{
			title: 'Gamepad (any controller)',
			rows: [
				['A · B · X · Y', 'Play-pause · Next · Previous · Mute'],
				['D-pad ↑↓ ←→', 'Volume · Seek ±10s'],
				['Left stick', 'Seek scrub (hold)'],
				['Right stick', 'Fast seek ±30s (hold)'],
				['LB / RB', 'Seek ∓10s'],
				['LT / RT', 'Volume ∓5'],
				['Start', 'Toggle mini player']
			]
		}
	];
</script>

<Dialog.Root bind:open={ui.shortcutsOpen}>
	<Dialog.Content class="max-h-[80vh] overflow-y-auto sm:max-w-lg">
		<Dialog.Header>
			<Dialog.Title>Keyboard & controller shortcuts</Dialog.Title>
			<Dialog.Description>Everything playback responds to, on one page.</Dialog.Description>
		</Dialog.Header>
		<div class="grid gap-5 pb-2">
			{#each groups as g (g.title)}
				<section>
					<h3 class="mb-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
						{g.title}
					</h3>
					<div class="divide-y divide-border/60 rounded-lg border">
						{#each g.rows as [key, action] (key)}
							<div class="flex items-center justify-between gap-4 px-3 py-1.5 text-sm">
								<kbd
									class="whitespace-nowrap rounded border bg-muted px-1.5 py-0.5 font-mono text-[0.6875rem] font-medium"
								>
									{key}
								</kbd>
								<span class="text-right text-muted-foreground">{action}</span>
							</div>
						{/each}
					</div>
				</section>
			{/each}
		</div>
	</Dialog.Content>
</Dialog.Root>
