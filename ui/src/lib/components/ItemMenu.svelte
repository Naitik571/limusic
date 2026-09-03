<script lang="ts">
	// The ⋯ menu for a browse item, whichever kind it is: songs get TrackMenu, everything else
	// PlaylistMenu. Cards and rows all want the same split, so it lives here instead of in each one.
	import type { BrowseItem } from '$lib/api';
	import { asSong } from '$lib/browse';
	import { openAddToPlaylist } from '$lib/player.svelte';
	import TrackMenu from './TrackMenu.svelte';
	import PlaylistMenu from './PlaylistMenu.svelte';

	let {
		item,
		triggerClass,
		openAt = null,
		onclose = undefined
	}: {
		item: BrowseItem;
		triggerClass: string;
		/** External open request at viewport coords, forwarded to the song/playlist menu. */
		openAt?: { x: number; y: number } | null;
		/** Fired when an externally-opened menu closes. */
		onclose?: () => void;
	} = $props();
</script>

{#if item.kind === 'song'}
	<TrackMenu song={asSong(item)} onAdd={() => openAddToPlaylist(asSong(item))} {triggerClass} {openAt} {onclose} />
{:else}
	<PlaylistMenu {item} showPin={item.kind === 'playlist'} {triggerClass} {openAt} {onclose} />
{/if}
