<script lang="ts">
	// Ctrl+K search, without leaving the page you're on. Runs the same debounced `search_all` preview
	// the search field runs (`searchPreview`, same page-cache key), so the two show the same rows and
	// a query previewed here doesn't get searched again when you open the full results.
	//
	// shouldFilter={false}: the rows come back already ranked by YouTube, and re-scoring them against
	// the raw query locally would hide results whose title doesn't contain what you typed.
	// vimBindings={false}: those bind ctrl+k to "move up", which is the key that opens this.
	import { goto } from '$app/navigation';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { Search01Icon, MusicNote01Icon, UserIcon, FavouriteIcon } from '@hugeicons/core-free-icons';
	import * as Command from '$lib/components/ui/command/index.js';
	import { Skeleton } from '$lib/components/ui/skeleton';
	import type { BrowseItem } from '$lib/api';
	import { asSong, openItem, searchPreview } from '$lib/browse';
	import { ui, toast, openMiniPlayer, setSleepTimer } from '$lib/player.svelte';
	import { LAYOUTS, layout, applyLayout, appearance, setAppearance } from '$lib/theme.svelte';
	import { isLiked } from '$lib/player.svelte';
	import { checkForUpdatesInteractive } from '$lib/updater.svelte';
	import { thumb } from '$lib/thumb';
	import ItemMenu from './ItemMenu.svelte';

	const KIND = { song: 'Song', album: 'Album', artist: 'Artist', playlist: 'Playlist' };

	let query = $state('');
	let items = $state<BrowseItem[]>([]);
	let loading = $state(false);
	let loadedFor = ''; // query `items` belongs to, so a stale response can't land
	// Right-click menu for a result row. The dialog traps pointer events (focus trap +
	// interact-outside), so a menu rendered inside it opens but never receives clicks.
	// Instead the row stashes the item + pointer, closes the palette, and the menu below
	// (a sibling of the dialog, in <body> via toBody) opens at the saved point.
	let pendingMenu = $state<{ item: BrowseItem; x: number; y: number } | null>(null);

	function openRowMenu(e: MouseEvent, item: BrowseItem) {
		e.preventDefault(); // no native menu, no cmdk selection
		e.stopPropagation();
		pendingMenu = { item, x: e.clientX, y: e.clientY };
		ui.paletteOpen = false;
	}

	// The menu's popup lives on <body>, which the dialog counts as an interaction outside itself and
	// would close on, unmounting the menu mid-click. `data-menu` marks the popup and its backdrop, so
	// clicking one is treated as still being inside. Everything else outside still dismisses.
	const inMenu = (e: Event) => {
		const t = e.target;
		return t instanceof Element && !!t.closest('[data-menu]');
	};

	// Opening is itself a keystroke, so nothing is fetched until the typing pauses. `loading` is set
	// on the keystroke rather than when the timer fires: otherwise the empty list reads as "no
	// results" for the whole debounce, on every query.
	$effect(() => {
		const q = query.trim();
		if (q.length < 2) {
			items = [];
			loading = false;
			loadedFor = '';
			return;
		}
		if (q === loadedFor) return;
		items = [];
		loading = true;
		const timer = setTimeout(() => load(q), 300);
		return () => clearTimeout(timer);
	});

	// Closing clears the field, which the effect above turns into an empty list: reopening starts
	// fresh instead of on the last search's rows. Opening drops any stale pending menu.
	$effect(() => {
		if (!ui.paletteOpen) query = '';
		else pendingMenu = null;
	});

	async function load(q: string) {
		loadedFor = q;
		try {
			const next = await searchPreview(q);
			if (loadedFor === q) items = next;
		} catch {
			if (loadedFor === q) items = [];
		} finally {
			if (loadedFor === q) loading = false;
		}
	}

	function choose(item: BrowseItem) {
		ui.paletteOpen = false;
		openItem(item); // a song plays, everything else opens its page
	}

	function allResults() {
		const q = query.trim();
		if (!q) return;
		ui.paletteOpen = false;
		goto(`/search?q=${encodeURIComponent(q)}`);
	}

	// --- Actions: app control from the palette ------------------------------------------------------------
	// Static list — every mutation goes through the stores' own setters, so nothing here needs to
	// be reactive. `layoutId` (layouts only) marks the active arrangement in the row.
	type PaletteAction = { label: string; hint?: string; layoutId?: (typeof LAYOUTS)[number]['id']; run: () => void };

	const SETTINGS_TABS: [string, string][] = [
		['general', 'General'],
		['themes', 'Appearance'],
		['playback', 'Playback'],
		['downloads', 'Downloads'],
		['data', 'Data'],
		['about', 'About']
	];

	const ACTIONS: PaletteAction[] = [
		...LAYOUTS.map((l): PaletteAction => ({
			label: `Layout: ${l.label}`,
			hint: l.description,
			layoutId: l.id,
			run: () => applyLayout(l.id)
		})),
		{
			label: 'Toggle ambient mode',
			hint: 'Blurred artwork backdrop',
			run: () => setAppearance({ ambientMode: !appearance.ambientMode })
		},
		{
			label: 'Toggle artwork accent',
			hint: 'Recolor from the cover',
			run: () => setAppearance({ artworkAccent: !appearance.artworkAccent })
		},
		{
			label: 'Toggle tabbed player',
			hint: 'Queue/lyrics tabs in the player view',
			run: () => setAppearance({ tabbedPlayer: !appearance.tabbedPlayer })
		},
		...SETTINGS_TABS.map(([id, label]): PaletteAction => ({
			label: `Settings: ${label}`,
			hint: 'Open Settings',
			run: () => {
				ui.settingsTab = id;
				ui.settingsOpen = true;
			}
		})),
		{ label: 'Open mini player', hint: 'Floating widget', run: () => openMiniPlayer() },
		{ label: 'Sleep timer: 15 min', run: () => setSleepTimer('minutes', 15) },
		{ label: 'Sleep timer: 30 min', run: () => setSleepTimer('minutes', 30) },
		{ label: 'Sleep timer: 60 min', run: () => setSleepTimer('minutes', 60) },
		{ label: 'Sleep timer: End of song', run: () => setSleepTimer('end_of_song') },
		{ label: 'Sleep timer: Off', run: () => setSleepTimer('off') },
		{
			label: 'Check for updates',
			run: () => {
				checkForUpdatesInteractive().then((r) =>
					r.error ? toast.error(r.message) : toast.success(r.message)
				);
			}
		}
	];

	function runAction(a: PaletteAction) {
		ui.paletteOpen = false;
		a.run();
	}

	const actionQuery = $derived(query.trim().toLowerCase());
	// Substring match on the label; empty query shows a few quick actions. The results themselves
	// are unfiltered here (shouldFilter={false}) — this list is ours alone.
	const visibleActions = $derived(
		actionQuery
			? ACTIONS.filter((a) => a.label.toLowerCase().includes(actionQuery)).slice(0, 6)
			: ACTIONS.slice(0, 4)
	);
</script>

<Command.Dialog
	bind:open={ui.paletteOpen}
	shouldFilter={false}
	vimBindings={false}
	loop
	title="Search"
	description="Search songs, albums, artists and playlists"
	class="sm:max-w-xl"
	contentProps={{
		'data-ctx': '',
		onInteractOutside: (e: PointerEvent) => {
			if (inMenu(e)) e.preventDefault();
		},
		onFocusOutside: (e: FocusEvent) => {
			if (inMenu(e)) e.preventDefault();
		}
	}}
>
	<Command.Input bind:value={query} placeholder="Search songs, albums, artists, playlists…" />
	<Command.List class="max-h-[22rem]">
		{#if visibleActions.length}
			<Command.Group heading="Actions">
				{#each visibleActions as a (a.label)}
					<Command.Item value={`action:${a.label}`} onSelect={() => runAction(a)} class="gap-2">
						<span class="truncate">{a.label}</span>
						{#if a.layoutId === layout.id}
							<span
								class="rounded bg-primary/15 px-1 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-primary"
							>
								Active
							</span>
						{/if}
						{#if a.hint}
							<span class="ml-auto shrink-0 truncate pl-4 text-xs text-muted-foreground">
								{a.hint}
							</span>
						{/if}
					</Command.Item>
				{/each}
			</Command.Group>
		{/if}

		{#if loading}
			{#each Array(4) as _, i (i)}
				<div class="flex items-center gap-3 px-3 py-2">
					<Skeleton class="h-10 w-10 shrink-0 rounded-md" />
					<div class="min-w-0 flex-1">
						<Skeleton class="h-3 w-40 rounded" />
						<Skeleton class="mt-2 h-2.5 w-24 rounded" />
					</div>
				</div>
			{/each}
		{:else if !items.length}
			<div class="px-4 py-6 text-center text-sm text-muted-foreground">
				{query.trim().length < 2 ? 'Type to search.' : 'Nothing quick for that.'}
			</div>
		{:else}
			<Command.Group heading="Results">
				{#each items as item (item.id)}
					<Command.Item
						value={item.id}
						onSelect={() => choose(item)}
						oncontextmenu={(e) => openRowMenu(e, item)}
						class="gap-3 px-2 py-1.5"
					>
						{#if item.thumbnail}
							<!-- 400, the same size the cards ask for: the CDN doesn't serve every rewritten
							     size, that one is verified, and the row lands on an image the grid already
							     fetched. -->
							<img
								src={thumb(item.thumbnail, 400)}
								alt=""
								class="h-10 w-10 shrink-0 object-cover {item.kind === 'artist'
									? 'rounded-full'
									: 'rounded-md'}"
							/>
						{:else}
							<div
								class="flex h-10 w-10 shrink-0 items-center justify-center bg-muted text-muted-foreground/50 {item.kind ===
								'artist'
									? 'rounded-full'
									: 'rounded-md'}"
							>
								<HugeiconsIcon
									icon={item.kind === 'artist' ? UserIcon : MusicNote01Icon}
									class="h-5 w-5"
								/>
							</div>
						{/if}
						<div class="min-w-0 flex-1">
							<div class="truncate text-sm">{item.title}</div>
							<div class="flex items-center gap-1 text-xs text-muted-foreground">
								<span class="truncate">
									{KIND[item.kind]}{item.subtitle ? ` • ${item.subtitle}` : ''}
								</span>
								{#if item.kind === 'song' && isLiked(asSong(item))}
									<HugeiconsIcon icon={FavouriteIcon} class="h-3.5 w-3.5 fill-primary text-primary" />
								{/if}
							</div>
						</div>
					</Command.Item>
				{/each}
			</Command.Group>
		{/if}

		{#if query.trim().length >= 2}
			<Command.Group>
				<Command.Item value="__all__" onSelect={allResults} class="gap-2 text-muted-foreground">
					<HugeiconsIcon icon={Search01Icon} class="h-3.5 w-3.5" />
					<span class="truncate">All results for “{query.trim()}”</span>
				</Command.Item>
			</Command.Group>
		{/if}
	</Command.List>
	<!-- No visible trigger: a palette row is too small for a hover-only ⋯, and the menu only ever
	     opens from a right-click (see `openRowMenu`). Rendered here, outside the dialog, so the
	     dialog's focus trap can't swallow the menu's clicks. Keyed per open for a fresh instance. -->
</Command.Dialog>
{#if pendingMenu}
	{#key `${pendingMenu.item.id}-${pendingMenu.x}-${pendingMenu.y}`}
		<ItemMenu
			item={pendingMenu.item}
			triggerClass="hidden"
			openAt={{ x: pendingMenu.x, y: pendingMenu.y }}
			onclose={() => (pendingMenu = null)}
		/>
	{/key}
{/if}
