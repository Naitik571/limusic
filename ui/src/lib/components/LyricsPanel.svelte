<script lang="ts">
	import { fade, fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import { beforeNavigate } from '$app/navigation';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { Maximize01Icon, Minimize01Icon } from '@hugeicons/core-free-icons';
	import LyricsView from './LyricsView.svelte';
	import { playback } from '$lib/player.svelte';
	import { thumb } from '$lib/thumb';

	let { onClose, queueOpen = false }: { onClose: () => void; queueOpen?: boolean } = $props();

	let expanded = $state(false);

	// Expanded, the panel covers the page — so navigating anywhere means the user wants to see that
	// page, not the lyrics. The docked panel sits beside the content, so it stays put.
	// beforeNavigate (not a pathname effect) so clicking the tab you're already on also closes it.
	beforeNavigate(() => {
		if (expanded) onClose();
	});
</script>

<!-- Same overlay pattern as QueuePanel: always over the content, with a dismiss scrim below lg. When
     the queue is open too, this one steps left of it at lg+; narrower than that they stack. -->
<button
	class="absolute inset-0 z-20 cursor-default bg-black/40 lg:hidden"
	onclick={onClose}
	aria-label="Close lyrics"
	transition:fade={{ duration: 150 }}
></button>
<aside
	transition:fly={{ x: 32, duration: 220, easing: cubicOut }}
	class={expanded
		? // ponytail: left offsets mirror Sidebar's w-16/lg:w-60, right offset mirrors QueuePanel's
			// w-80 — keep in sync if those change.
			`side-panel absolute inset-y-0 left-16 right-0 z-30 flex h-full flex-col border-l bg-card shadow-2xl lg:left-60 ${queueOpen ? 'lg:right-80' : ''}`
		: `side-panel absolute inset-y-0 right-0 z-30 flex h-full w-80 max-w-[80vw] flex-col border-l bg-card/80 backdrop-blur-xl shadow-2xl ${queueOpen ? 'lg:right-80' : ''}`}
>
	<!-- BetterLyrics-style blurred artwork backdrop for the side lyrics panel -->
	<div class="lyrics-backdrop pointer-events-none absolute inset-0 overflow-hidden" aria-hidden="true">
		{#if playback.now?.thumbnail}
			<img
				src={thumb(playback.now.thumbnail, 320)}
				alt=""
				class="h-full w-full object-cover opacity-30 blur-3xl saturate-125"
			/>
		{/if}
		<div class="absolute inset-0 bg-background/70"></div>
	</div>

	<div class="relative flex min-h-0 flex-col">
		<div class="flex items-center justify-between border-b px-4 py-3">
			<h2 class="font-heading text-sm font-semibold">Lyrics</h2>
			<button
				onclick={() => (expanded = !expanded)}
				class="cursor-pointer text-muted-foreground transition-colors hover:text-foreground"
				aria-label={expanded ? 'Shrink lyrics' : 'Expand lyrics'}
			>
				<!-- icon swap via altIcon/showAlt: `icon` is frozen at mount -->
				<HugeiconsIcon
					icon={Maximize01Icon}
					altIcon={Minimize01Icon}
					showAlt={expanded}
					class="h-4 w-4"
				/>
			</button>
		</div>
		<LyricsView {expanded} />
	</div>
</aside>
