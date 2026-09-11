<script module lang="ts">
	import * as apiMod from '$lib/api';

	// Shared crossfade-overrides map (one load for all row menus, refreshed on every save).
	let fadeMap: Record<string, number> | null = null;
	async function loadFadeMap(): Promise<Record<string, number>> {
		if (!fadeMap) {
			try {
				const s = await apiMod.getSettings();
				const raw = s.crossfade_overrides;
				fadeMap = raw ? (JSON.parse(raw) as Record<string, number>) : {};
			} catch {
				fadeMap = {};
			}
		}
		return fadeMap;
	}
	export async function saveFadeMap(map: Record<string, number>): Promise<void> {
		fadeMap = map;
		await apiMod.setSetting('crossfade_overrides', JSON.stringify(map));
	}
	export async function fadeOverrideCount(): Promise<number> {
		return Object.keys(await loadFadeMap()).length;
	}
	export async function clearFadeMap(): Promise<void> {
		await saveFadeMap({});
	}
</script>

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
		ThumbsDownIcon,
		UserListIcon,
		Vynil02Icon,
		DashboardSquare02Icon,
		Download01Icon,
		Delete01Icon
	} from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import { toast } from '$lib/player.svelte';
	import type { SongItem } from '$lib/api';
	import { anchorMenu, claimMenu, ctxHost, fitMenu, nextMenuId, NO_ANCHOR, onOtherMenuClaimed, toBody, type MenuCloseReason } from '$lib/menu';
	import { addPick, enqueue, isLiked, startRadio, toggleLike, rate, downloadedIds, markDownloaded, markNotDownloaded } from '$lib/player.svelte';
	import { crossfade } from '$lib/player.svelte';

	let {
		song,
		triggerClass = '',
		onAdd,
		onRemove,
		removeLabel = 'Remove from playlist',
		linksOnly = false,
		openAt = null,
		fadeNext = null,
		onclose = undefined
	}: {
		song: SongItem;
		/** Classes for the ⋯ trigger button (positioning differs per host: inline vs overlay). */
		triggerClass?: string;
		/** Adds an "Add to playlist" menu item. */
		onAdd?: () => void;
		/** Adds a remove menu item (label via `removeLabel`). */
		onRemove?: () => void;
		removeLabel?: string;
		/** Player-bar variant: ⋮ trigger, and only artist/album/shortcuts (queue and like already
		    have their own buttons there). */
		linksOnly?: boolean;
		/** External open request at viewport coords (palette right-click): opens without a trigger.
		    The palette dialog traps pointer events, so its menu must live outside the dialog. */
		openAt?: { x: number; y: number } | null;
		/** Queue-only: videoId of the track after this one — shows a "Crossfade into next"
		    picker for this pair (gapless auto-advances always use the global duration). */
		fadeNext?: string | null;
		/** Fired when an externally-opened menu closes, with how: an item ran (`action`), the
		    backdrop dismissed it (`dismiss`), or another menu claimed the stage (`claimed`). */
		onclose?: (reason: MenuCloseReason) => void;
	} = $props();

	let menuOpen = $state(false);
	let anchor = $state(NO_ANCHOR);

	// Per-pair crossfade editor (queue rows only — needs the track after this one). The map
	// lives in module scope so every row's menu shares one load, not one fetch per open.
	let fadeOpen = $state(false);
	let fadeCur = $state<number | null>(null);
	const FADE_STEPS = [0, 2, 4, 6, 8, 12];
	const fadeKey = $derived(`${song.video_id}__${fadeNext ?? ''}`);
	async function openFade() {
		fadeOpen = true;
		fadeCur = null;
		try {
			const map = await loadFadeMap();
			fadeCur = map[fadeKey] ?? null;
		} catch {
			fadeCur = null;
		}
	}
	async function pickFade(secs: number | null) {
		try {
			const map = await loadFadeMap();
			if (secs === null) delete map[fadeKey];
			else map[fadeKey] = secs;
			await saveFadeMap(map);
			fadeCur = secs;
			toast.success(
				secs === null
					? 'Crossfade reset to global'
					: `Crossfade into next: ${secs === 0 ? 'off' : `${secs}s`}`
			);
		} catch (e) {
			toast.error(String(e));
		}
	}

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
	let closeReason: MenuCloseReason = 'dismiss';
	$effect(() => {
		if (menuOpen) wasOpen = true;
		else if (wasOpen) {
			wasOpen = false;
			onclose?.(closeReason);
			closeReason = 'dismiss';
		}
	});

	// One menu at a time: opening here closes every other open menu (and another menu opening
	// closes this one). The palette renders many of these — one per row — so this is what keeps
	// right-clicking down a list from stacking a menu per row.
	const menuId = nextMenuId();
	$effect(() =>
		onOtherMenuClaimed(menuId, () => {
			closeReason = 'claimed';
			menuOpen = false;
		})
	);

	// Click on the ⋯ opens under the button; right-click on the host row opens at the pointer.
	function openMenu(e: MouseEvent) {
		e.preventDefault(); // a right-click must not also raise WebKit's own menu
		e.stopPropagation();
		anchor = anchorMenu(e, { align: 'right' });
		menuOpen = true;
		claimMenu(menuId);
	}
	// stopPropagation everywhere: the trigger sits inside a clickable row (TrackRow's whole row is a
	// play target), so its click must not reach the row's onplay (e.g. replacing the queue with the
	// playlist). The popup itself now lives at <body> and no longer bubbles into the row, but these
	// stay: they cost nothing and the trigger still needs them.
	function run(e: MouseEvent, action?: () => void) {
		e.stopPropagation();
		closeReason = 'action';
		menuOpen = false;
		action?.();
	}
	// Right-clicking off the menu dismisses it, same as a left click: the backdrop swallows the
	// event, so the row underneath never sees it.
	function close(e: MouseEvent) {
		e.preventDefault();
		e.stopPropagation();
		closeReason = 'dismiss';
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

<!-- Externally-opened menus (palette) need no trigger: rendering the hidden button would let
     its ctxHost claim the nearest ancestor [data-ctx] — e.g. the page behind the palette — and
     open this menu on top of that surface's own menu. -->
{#if !openAt}
	<button
		class="{triggerClass} {menuOpen ? 'opacity-100' : ''}"
		onclick={openMenu}
		aria-label="Track options"
		{@attach ctxHost(openMenu)}
	>
		<!-- icon swap via altIcon/showAlt — `icon` is frozen at mount -->
		<HugeiconsIcon
			icon={MoreHorizontalIcon}
			altIcon={MoreVerticalIcon}
			showAlt={linksOnly}
			class="h-4 w-4"
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
			<button
				class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
				onclick={(e) => run(e, () => rate(song, 'dislike'))}
			>
				<HugeiconsIcon icon={ThumbsDownIcon} class="h-4 w-4" />
				Dislike — skip and unqueue
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
						if (downloaded)
							api
								.deleteDownload(song.video_id)
								.then(() => markNotDownloaded(song.video_id))
								.catch((e) => toast.error(String(e)));
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
		{#if fadeNext && !isLocal}
			<button
				class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
				onclick={(e) => {
					e.stopPropagation();
					fadeOpen = !fadeOpen;
					if (fadeOpen) void openFade();
				}}
				aria-expanded={fadeOpen}
				title="Gapless auto-advances always use the global duration"
			>
				<HugeiconsIcon icon={Vynil02Icon} class="h-4 w-4" />
				<span class="flex-1">Crossfade into next…</span>
				<span class="text-xs text-muted-foreground">
					{fadeCur === null ? `Global (${crossfade.secs.toFixed(1)}s)` : fadeCur === 0 ? 'Off' : `${fadeCur}s`}
				</span>
			</button>
			{#if fadeOpen}
				<div class="flex flex-wrap gap-1 px-2 py-1.5" role="group" aria-label="Crossfade seconds">
					{#each FADE_STEPS as s}
						<button
							class="cursor-pointer rounded-md border px-2 py-1 text-xs transition-colors {fadeCur === s
								? 'border-transparent bg-primary text-primary-foreground'
								: 'hover:bg-accent/10'}"
							onclick={(e) => run(e, () => pickFade(s))}
						>
							{s === 0 ? 'Off' : `${s}s`}
						</button>
					{/each}
					<button
						class="cursor-pointer rounded-md border px-2 py-1 text-xs transition-colors hover:bg-accent/10 disabled:opacity-50"
						disabled={fadeCur === null}
						title="Forget this pair, use the global duration"
						onclick={(e) => run(e, () => pickFade(null))}
					>
						Global
					</button>
				</div>
			{/if}
		{/if}
	</div>
{/if}
