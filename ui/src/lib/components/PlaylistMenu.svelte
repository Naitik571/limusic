<script lang="ts">
	// The â‹¯ menu on a sidebar library row, a card, or an artist row. Positioned `fixed` and moved to
	// <body> like TrackMenu: the playlist list is a scroll container, so an absolute popup would be
	// clipped by it. Right-clicking the surrounding `[data-ctx]` element opens it at the pointer.
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		MoreHorizontalIcon,
		MoreVerticalIcon,
		PinIcon,
		PinOffIcon,
		Radio02Icon,
		ArrowUpNarrowWideIcon,
		ArrowDownWideNarrowIcon,
		DashboardSquare02Icon,
		PlayIcon,
		ShuffleIcon
	} from '@hugeicons/core-free-icons';
import * as api from '$lib/api';
import type { BrowseItem } from '$lib/api';
import { enqueueItem, playItem } from '$lib/browse';
import { anchorMenu, claimMenu, ctxHost, fitMenu, nextMenuId, NO_ANCHOR, onOtherMenuClaimed, toBody } from '$lib/menu';
import { addPick, personal, playFrom, startRadio, togglePin } from '$lib/player.svelte';

	let {
		item,
		showPin = true,
		vertical = false,
		iconClass = 'h-4 w-4',
		// Visibility lives here too: most triggers only appear on hover, but a row that has nothing
		// else to reveal on hover shows its ⋯ all the time.
		triggerClass = 'absolute right-1 top-1/2 flex h-7 w-7 -translate-y-1/2 cursor-pointer items-center justify-center rounded-md text-muted-foreground opacity-0 transition hover:bg-sidebar-accent hover:text-foreground focus-visible:opacity-100 focus-visible:ring-2 focus-visible:ring-ring group-hover/row:opacity-100',
		openAt = null,
		onclose = undefined
	}: {
		item: BrowseItem;
		showPin?: boolean;
		vertical?: boolean;
		iconClass?: string;
		triggerClass?: string;
		/** External open request at viewport coords (palette right-click): opens without a trigger.
		    The palette dialog traps pointer events, so its menu must live outside the dialog. */
		openAt?: { x: number; y: number } | null;
		/** Fired when an externally-opened menu closes (backdrop, action, or another menu claim). */
		onclose?: () => void;
	} = $props();

	const pinned = $derived(personal.pins.includes(item.id));
	// Radio needs a YouTube item behind it: local folders and the locally-built On Repeat have none.
	const hasRadio = $derived(!api.isLocalId(item.id) && item.id !== api.ON_REPEAT_ID);
	// An artist isn't a track list â€” there's nothing unambiguous to queue. Songs, albums and
	// playlists (local ones included) all are.
	const canQueue = $derived(item.kind === 'song' || item.kind === 'album' || item.kind === 'playlist');

	// The tracks have to be fetched before anything can be queued, so the menu stays open and the
	// row shows it's working. Guards a second click from queueing the album twice.
	let queueing = $state(false);
	async function queue(next: boolean) {
		if (queueing) return;
		queueing = true;
		try {
			await enqueueItem(item, next);
			menuOpen = false;
		} finally {
			queueing = false;
		}
	}

	let playing = $state(false);
	async function play(shuffle = false) {
		if (playing) return;
		playing = true;
		try {
			if (item.kind === 'playlist' || item.kind === 'album') {
				// Fetch then play via playFrom so shuffle is honored and autoplay seeds correctly
				if (item.kind === 'album') {
					const album = await api.getAlbum(item.id);
					await playFrom(item, album.items, null, album.playlistId ?? undefined, shuffle, album.continuation);
				} else {
					const pl = await api.getPlaylist(item.id);
					await playFrom(item, pl.items, null, item.id === api.ON_REPEAT_ID ? undefined : item.id, shuffle, pl.continuation);
				}
			} else {
				await playItem(item);
				// playItem for artists/songs doesn't use shuffle; for playlists above we already handled it
				if (shuffle && item.kind === 'song') {
					// no-op for single song
				}
			}
			menuOpen = false;
		} catch {
			// playFrom/playItem toasts on failure
		} finally {
			playing = false;
		}
	}

	let menuOpen = $state(false);
	let anchor = $state(NO_ANCHOR);

	// External open (see `openAt`): anchor at the saved pointer, then open like a right-click.
	// Runs on mount when the parent keys a fresh instance per open. One-shot: without the guard,
	// closing the menu (menuOpen → false) would re-trigger this effect and instantly reopen it,
	// making the menu unclosable.
	let externalOpened = false;
	$effect(() => {
		if (openAt && !externalOpened) {
			externalOpened = true;
			anchor = {
				style: NO_ANCHOR.style,
				box: { left: openAt.x, right: openAt.x, top: openAt.y, bottom: openAt.y },
				gap: 0,
				align: 'left'
			};
			menuOpen = true;
			claimMenu(menuId);
		}
	});

	// Report external closes back to the parent so it can drop its pending state. Internal
	// (trigger-driven) menus pass no `onclose` and are unaffected.
	let wasOpen = $state(false);
	$effect(() => {
		if (menuOpen) wasOpen = true;
		else if (wasOpen) {
			wasOpen = false;
			onclose?.();
		}
	});

	// One menu at a time (see TrackMenu).
	const menuId = nextMenuId();
	$effect(() => onOtherMenuClaimed(menuId, () => (menuOpen = false)));

	// Click on the ⋯ opens under the button; right-click on the host card or row opens at the pointer.
	function openMenu(e: MouseEvent) {
		e.preventDefault(); // a right-click must not also raise WebKit's own menu
		e.stopPropagation();
		anchor = anchorMenu(e, { align: 'right' });
		menuOpen = true;
		claimMenu(menuId);
	}
	// stopPropagation everywhere: the trigger sits over a clickable host (a card's whole surface is a
	// play/navigate target), so its click must not reach the host's handler. The popup itself now
	// lives at <body> and no longer bubbles into the host, but these stay: the trigger needs them.
	function run(e: MouseEvent, action?: () => void) {
		e.stopPropagation();
		menuOpen = false;
		action?.();
	}
	// Right-clicking off the menu dismisses it, same as a left click.
	function close(e: MouseEvent) {
		e.preventDefault();
		e.stopPropagation();
		menuOpen = false;
	}
</script>

<!-- Externally-opened menus (palette) need no trigger: rendering the hidden button would let
     its ctxHost claim the nearest ancestor [data-ctx] — e.g. the page behind the palette — and
     open this menu on top of that surface's own menu. -->
{#if !openAt}
	<button
		class="{triggerClass} {menuOpen ? 'opacity-100' : ''}"
		onclick={openMenu}
		aria-label="Playlist options"
		{@attach ctxHost(openMenu)}
	>
		<!-- icon swap via altIcon/showAlt — `icon` is frozen at mount -->
		<HugeiconsIcon
			icon={MoreHorizontalIcon}
			altIcon={MoreVerticalIcon}
			showAlt={vertical}
			class={iconClass}
		/>
	</button>
{/if}

{#if menuOpen}
	<button
		class="fixed inset-0 z-[55] cursor-default"
		onclick={close}
		oncontextmenu={close}
		aria-label="Close menu"
		data-menu
		{@attach toBody}
	></button>
	<div
		class="fixed z-[60] min-w-44 animate-in rounded-xl border-transparent glass-strong p-1 text-popover-foreground shadow-xl duration-150 fade-in-0 zoom-in-95"
		style={anchor.style}
		data-menu
		{@attach toBody}
		{@attach fitMenu(anchor)}
	>
		{#if item.kind === 'playlist' || item.kind === 'album'}
			<button
				class="flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10 disabled:opacity-50"
				disabled={playing}
				onclick={(e) => {
					e.stopPropagation();
					play(false);
				}}
			>
				<HugeiconsIcon icon={PlayIcon} class="h-4 w-4" /> Play
			</button>
			<button
				class="flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10 disabled:opacity-50"
				disabled={playing}
				onclick={(e) => {
					e.stopPropagation();
					play(true);
				}}
			>
				<HugeiconsIcon icon={ShuffleIcon} class="h-4 w-4" /> Shuffle play
			</button>
		{/if}
		{#if showPin}
			<button
				class="flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
				onclick={(e) => run(e, () => togglePin(item.id))}
			>
				<HugeiconsIcon icon={pinned ? PinOffIcon : PinIcon} class="h-4 w-4" />
				{pinned ? 'Unpin' : 'Pin to top'}
			</button>
		{/if}
		{#if canQueue}
			<button
				class="flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10 disabled:opacity-50"
				disabled={queueing}
				onclick={(e) => {
					e.stopPropagation();
					queue(true);
				}}
			>
				<HugeiconsIcon icon={ArrowUpNarrowWideIcon} class="h-4 w-4" /> Play next
			</button>
			<button
				class="flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10 disabled:opacity-50"
				disabled={queueing}
				onclick={(e) => {
					e.stopPropagation();
					queue(false);
				}}
			>
				<HugeiconsIcon icon={ArrowDownWideNarrowIcon} class="h-4 w-4" /> Add to queue
			</button>
		{/if}
		{#if hasRadio}
			<button
				class="flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
				onclick={(e) => run(e, () => startRadio(item.kind as 'artist' | 'album' | 'playlist', item.id, item.title))}
			>
				<HugeiconsIcon icon={Radio02Icon} class="h-4 w-4" /> Start radio
			</button>
		{/if}
		<button
			class="flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
			onclick={(e) => run(e, () => addPick(item))}
		>
			<HugeiconsIcon icon={DashboardSquare02Icon} class="h-4 w-4" /> Add to shortcuts
		</button>
	</div>
{/if}
