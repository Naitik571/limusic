<script lang="ts">
	// "Forgotten favourites" is a pile of half-remembered songs, not a row of destinations — so it
	// reads as a list you scan, in balanced columns, instead of a carousel you page through.
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { ArrowRight01Icon } from '@hugeicons/core-free-icons';
	import TrackRow from './TrackRow.svelte';
	import * as api from '$lib/api';
	import type { HomeSection, SongItem } from '$lib/api';
	import { openAddToPlaylist, playback } from '$lib/player.svelte';
	import { asSong } from '$lib/browse';

	let { section, onMore }: { section: HomeSection; onMore?: () => void } = $props();

	// ponytail: 15 keeps the block scannable (5 rows × 3 columns at full width); the shelf's "More"
	// button is where the rest of a longer shelf lives.
	const songs = $derived<SongItem[]>(
		section.items
			.filter((i) => i.kind === 'song')
			.slice(0, 15)
			.map(asSong)
	);

	// Clicking a row starts there and queues the rest of the shelf, so the section plays as a set.
	const play = (start: number) => api.playPlaylist(songs, start, undefined, section.title);
</script>

<section>
	<div class="mb-3 flex items-baseline justify-between gap-3">
		{#if onMore}
			<button class="min-w-0 cursor-pointer text-left hover:underline" onclick={onMore}>
				<h2 class="truncate font-heading text-lg font-semibold">{section.title}</h2>
			</button>
			<button
				class="flex shrink-0 cursor-pointer items-center gap-0.5 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground"
				onclick={onMore}
			>
				See all
				<HugeiconsIcon icon={ArrowRight01Icon} class="h-3.5 w-3.5" />
			</button>
		{:else}
			<h2 class="truncate font-heading text-lg font-semibold">{section.title}</h2>
		{/if}
	</div>
	<!-- CSS columns, not a grid: it fills top-to-bottom like the shelf it replaces, balances the last
	     column itself, and needs no row count per breakpoint. -->
	<div class="columns-1 gap-x-6 md:columns-2 xl:columns-3">
		{#each songs as song, i (song.video_id + ':' + i)}
			<div class="break-inside-avoid">
				<TrackRow
					{song}
					compact
					active={playback.now?.videoId === song.video_id}
					onplay={() => play(i)}
					onAdd={() => openAddToPlaylist(song)}
				/>
			</div>
		{/each}
	</div>
</section>
