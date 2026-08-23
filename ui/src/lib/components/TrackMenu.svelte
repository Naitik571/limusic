<script lang="ts">
	// The â‹¯ options menu shared by TrackRow (inline trigger) and MediaCard (overlay trigger).
	// Right-clicking anywhere in the surrounding `[data-ctx]` element opens the same menu at the
	// pointer (see `ctxHost`), which is what a track row's whole surface is for.
	// The queue actions + like are universal; go-to-artist/album/playlist show when the song carries
	// them. The popup is `fixed`, anchored at the trigger and moved to <body> (`toBody`), so no
	// scroll container clips it and no contained ancestor becomes its containing block.
	import { goto } from '$app/navigation';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		MoreHorizontalIcon,
		MoreVerticalIcon,
		PlayListAddIcon,
		PlayListRemoveIcon,
		ArrowUpNarrowWideIcon,
		ArrowDownWideNarrowIcon,
		Radio02Icon,
		FavouriteIcon,
		UserListIcon,
		Vynil02Icon,
		DashboardSquare02Icon,
		Download01Icon,
		Delete01Icon
	} from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import { toast } from '$lib/player.svelte';
	import type { SongItem } from '$lib/api';
	import { anchorMenu, ctxHost, fitMenu, NO_ANCHOR, toBody } from '$lib/menu';
	import { addPick, enqueue, isLiked, startRadio, toggleLike, downloadedIds, markDownloaded, markNotDownloaded } from '$lib/player.svelte';

	let {
		song,
		triggerClass = '',
		onAdd,
		onRemove,
		removeLabel = 'Remove from playlist',
		linksOnly = false
	}: {
		song: SongItem;
		/** Classes for the â‹¯ trigger button (positioning differs per host: inline vs overlay). */
		triggerClass?: string;
		/** Adds an "Add to playlist" menu item. */
		onAdd?: () => void;
		/** Adds a remove menu item (label via `removeLabel`). */
		onRemove?: () => void;
		removeLabel?: string;
		/** Player-bar variant: â‹® trigger, and only artist/album/shortcuts (queue and like already
		    have their own buttons there). */
		linksOnly?: boolean;
	} = $props();

	let menuOpen = $state(false);
	let anchor = $state(NO_ANCHOR);

	// Click on the â‹¯ opens under the button; right-click on the host row opens at the pointer.
	function openMenu(e: MouseEvent) {
		e.preventDefault(); // a right-click must not also raise WebKit's own menu
		e.stopPropagation();
		anchor = anchorMenu(e, { align: 'right' });
		menuOpen = true;
	}
	// stopPropagation everywhere: the trigger sits inside a clickable row (TrackRow's whole row is a
	// play target), so its click must not reach the row's onplay (e.g. replacing the queue with the
	// playlist). The popup itself now lives at <body> and no longer bubbles into the row, but these
	// stay: they cost nothing and the trigger still needs them.
	function run(e: MouseEvent, action?: () => void) {
		e.stopPropagation();
		menuOpen = false;
		action?.();
	}
	// Right-clicking off the menu dismisses it, same as a left click: the backdrop swallows the
	// event, so the row underneath never sees it.
	function close(e: MouseEvent) {
		e.preventDefault();
		e.stopPropagation();
		menuOpen = false;
	}

	const liked = $derived(isLiked(song));
	// A local file has no YouTube identity: liking it or putting it in a YTM playlist is not a
	// thing, so those items don't show. Queue, shortcuts and go-to-album work normally.
	const isLocal = $derived(api.isLocalId(song.video_id));
	// Shared with every other row: downloading here, in a playlist bulk download, or in Settings
	// flips this instantly (and deleting anywhere clears it).
	const downloaded = $derived(downloadedIds.has(song.video_id));
</script>

<button
	class="{triggerClass} {menuOpen ? 'opacity-100' : ''}"
	onclick={openMenu}
	aria-label="Track options"
	{@attach ctxHost(openMenu)}
>
	<!-- icon swap via altIcon/showAlt â€” `icon` is frozen at mount -->
	<HugeiconsIcon
		icon={MoreHorizontalIcon}
		altIcon={MoreVerticalIcon}
		showAlt={linksOnly}
		class="h-4 w-4"
	/>
</button>

{#if menuOpen}
	<button
		class="fixed inset-0 z-40 cursor-default"
		onclick={close}
		oncontextmenu={close}
		aria-label="Close menu"
		{@attach toBody}
	></button>
	<div
		class="fixed z-50 min-w-44 animate-in rounded-xl border-transparent glass-strong p-1 text-popover-foreground shadow-xl duration-150 fade-in-0 zoom-in-95"
		style={anchor.style}
		{@attach toBody}
		{@attach fitMenu(anchor)}
	>
		{#if !linksOnly}
			<button
				class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
				onclick={(e) => run(e, () => enqueue([song], true))}
			>
				<HugeiconsIcon icon={ArrowUpNarrowWideIcon} class="h-4 w-4" /> Play next
			</button>
			<button
				class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
				onclick={(e) => run(e, () => enqueue([song], false))}
			>
				<HugeiconsIcon icon={ArrowDownWideNarrowIcon} class="h-4 w-4" /> Add to queue
			</button>
		{/if}
		<!-- Radio is the one action worth having in the player bar too (`linksOnly`): it's how you
		     say "keep going with more like this" about the song that's playing. -->
		{#if !isLocal}
			<button
				class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
				onclick={(e) => run(e, () => startRadio('song', song.video_id, song.title))}
			>
				<HugeiconsIcon icon={Radio02Icon} class="h-4 w-4" /> Start radio
			</button>
		{/if}
		{#if !isLocal && !linksOnly}
			<button
				class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
				onclick={(e) => run(e, () => toggleLike(song))}
			>
				<HugeiconsIcon icon={FavouriteIcon} class="h-4 w-4 {liked ? 'fill-current text-primary' : ''}" />
				{liked ? 'Remove from Liked Songs' : 'Save to Liked Songs'}
			</button>
		{/if}
		{#if song.artist_id}
			<button
				class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
				onclick={(e) => run(e, () => goto(`/artist/${encodeURIComponent(song.artist_id!)}`))}
			>
				<HugeiconsIcon icon={UserListIcon} class="h-4 w-4" /> Go to artist
			</button>
		{/if}
		<!-- Local files carry no album_id (local.rs). Checked here too: a queue restored from before
		     that changed still has one on its rows, and it would open a page this menu shouldn't offer. -->
		{#if song.album_id && !isLocal}
			<button
				class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
				onclick={(e) => run(e, () => goto(`/album/${encodeURIComponent(song.album_id!)}`))}
			>
				<HugeiconsIcon icon={Vynil02Icon} class="h-4 w-4" /> Go to album
			</button>
		{/if}
		<button
			class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
			onclick={(e) =>
				run(e, () =>
					addPick({
						kind: 'song',
						id: song.video_id,
						title: song.title,
						subtitle: song.artists,
						thumbnail: song.thumbnail
					})
				)}
		>
			<HugeiconsIcon icon={DashboardSquare02Icon} class="h-4 w-4" /> Add to shortcuts
		</button>
		{#if !isLocal}
			<button
				class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm {downloaded
					? 'text-destructive hover:bg-destructive/10'
					: 'hover:bg-accent/10'}"
				onclick={(e) =>
					run(e, () => {
						if (downloaded) api.deleteDownload(song.video_id).then(() => markNotDownloaded(song.video_id));
						else
							api
								.downloadTrack({
									videoId: song.video_id,
									title: song.title,
									artists: song.artists,
									album: song.album ? String(song.album) : null,
									duration: Number(song.duration ?? 0),
									thumb: song.thumbnail
								})
									.then(() => markDownloaded(song.video_id))
									.catch((err) => toast.error(`Download failed: ${err}`));
					})}
				>
				<HugeiconsIcon icon={downloaded ? Delete01Icon : Download01Icon} class="h-4 w-4" />
				{downloaded ? 'Remove download' : 'Download'}
			</button>
		{/if}
		{#if onAdd && !isLocal}
			<button
				class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
				onclick={(e) => run(e, onAdd)}
			>
				<HugeiconsIcon icon={PlayListAddIcon} class="h-4 w-4" /> Add to playlist
			</button>
		{/if}
		{#if onRemove}
			<button
				class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm text-destructive hover:bg-destructive/10"
				onclick={(e) => run(e, onRemove)}
			>
				<HugeiconsIcon icon={PlayListRemoveIcon} class="h-4 w-4" /> {removeLabel}
			</button>
		{/if}
	</div>
{/if}
