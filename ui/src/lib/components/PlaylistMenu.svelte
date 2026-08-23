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
		DashboardSquare02Icon
	} from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import type { BrowseItem } from '$lib/api';
	import { enqueueItem } from '$lib/browse';
	import { anchorMenu, ctxHost, fitMenu, NO_ANCHOR, toBody } from '$lib/menu';
	import { addPick, personal, startRadio, togglePin } from '$lib/player.svelte';

	let {
		item,
		showPin = true,
		vertical = false,
		iconClass = 'h-4 w-4',
		// Visibility lives here too: most triggers only appear on hover, but a row that has nothing
		// else to reveal on hover shows its â‹¯ all the time.
		triggerClass = 'absolute right-1 top-1/2 flex h-7 w-7 -translate-y-1/2 cursor-pointer items-center justify-center rounded-md text-muted-foreground opacity-0 transition hover:bg-sidebar-accent hover:text-foreground focus-visible:opacity-100 focus-visible:ring-2 focus-visible:ring-ring group-hover/row:opacity-100'
	}: {
		item: BrowseItem;
		showPin?: boolean;
		vertical?: boolean;
		iconClass?: string;
		triggerClass?: string;
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

	let menuOpen = $state(false);
	let anchor = $state(NO_ANCHOR);

	// Click on the â‹¯ opens under the button; right-click on the host card or row opens at the pointer.
	function openMenu(e: MouseEvent) {
		e.preventDefault(); // a right-click must not also raise WebKit's own menu
		e.stopPropagation();
		anchor = anchorMenu(e, { align: 'right' });
		menuOpen = true;
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

<button
	class="{triggerClass} {menuOpen ? 'opacity-100' : ''}"
	onclick={openMenu}
	aria-label="Playlist options"
	{@attach ctxHost(openMenu)}
>
	<!-- icon swap via altIcon/showAlt â€” `icon` is frozen at mount -->
	<HugeiconsIcon
		icon={MoreHorizontalIcon}
		altIcon={MoreVerticalIcon}
		showAlt={vertical}
		class={iconClass}
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
		class="fixed z-50 min-w-48 animate-in rounded-xl border-transparent glass-strong p-1 text-popover-foreground shadow-xl duration-150 fade-in-0 zoom-in-95"
		style={anchor.style}
		{@attach toBody}
		{@attach fitMenu(anchor)}
	>
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
