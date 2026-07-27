// What a BrowseItem does when you click it. One implementation for every surface that renders a
// browse result — cards, the home recents rail, anything after them — so "open" and "play whole
// thing" can never drift apart between two components.
import { goto } from '$app/navigation';
import * as api from './api';
import type { BrowseItem, SongItem } from './api';
import { playFrom, toast, touchPick } from './player.svelte';

/** A song card carries everything a queue entry needs; the ⋯ menus take this shape. */
export const asSong = (i: BrowseItem): SongItem => ({
	video_id: i.id,
	title: i.title,
	artists: i.subtitle ?? '',
	thumbnail: i.thumbnail
});

/** Where a non-song item lives. Songs have no page — they play. */
export const hrefFor = (i: BrowseItem): string =>
	i.kind === 'artist'
		? `/artist/${encodeURIComponent(i.id)}`
		: i.kind === 'album'
			? `/album/${encodeURIComponent(i.id)}`
			: `/playlist/${encodeURIComponent(i.id)}`;

/** Primary click: a song plays, everything else opens its page. */
export function openItem(item: BrowseItem): void {
	// A click counts as "used" for Shortcuts eviction wherever the item was rendered. No-op unless
	// this item is actually on the grid.
	touchPick(item.id);
	if (item.kind === 'song') api.play(asSong(item));
	else goto(hrefFor(item));
}

/**
 * Play the whole thing without opening it. A song is immediate; a playlist/album has to be fetched
 * first, so callers own the in-flight state (the pulsing button). Never throws — a failure toasts
 * and the user stays where they were.
 */
export async function playItem(item: BrowseItem): Promise<void> {
	touchPick(item.id);
	if (item.kind === 'song') {
		api.play(asSong(item));
		return;
	}
	try {
		if (item.kind === 'album') {
			const album = await api.getAlbum(item.id);
			await playFrom(item, album.items, null, album.playlistId ?? undefined);
		} else {
			const pl = await api.getPlaylist(item.id);
			// `sourceId` seeds autoplay off that playlist's radio. On Repeat is local, so there is
			// no radio for it. Pass none and let autoplay seed off the last video instead.
			await playFrom(item, pl.items, null, item.id === api.ON_REPEAT_ID ? undefined : item.id);
		}
	} catch {
		toast('Could not play — try opening it instead');
	}
}
