<script lang="ts">
	// Ctrl+K search, without leaving the page you're on. Runs the same debounced `search_all` preview
	// the search field runs (`searchPreview`, same page-cache key), so the two show the same rows and
	// a query previewed here doesn't get searched again when you open the full results.
	//
	// shouldFilter={false}: the rows come back already ranked by YouTube, and re-scoring them against
	// the raw query locally would hide results whose title doesn't contain what you typed.
	// vimBindings={false}: those bind ctrl+k to "move up", which is the key that opens this.
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { Search01Icon, UserIcon, FavouriteIcon } from '@hugeicons/core-free-icons';
	import * as Command from '$lib/components/ui/command/index.js';
	import { Skeleton } from '$lib/components/ui/skeleton';
	import type { BrowseItem } from '$lib/api';
	import { asSong, openItem, searchPreview } from '$lib/browse';
	import { ui, toast, openMiniPlayer, setSleepTimer } from '$lib/player.svelte';
	import { LAYOUTS, layout, applyLayout, appearance, setAppearance } from '$lib/theme.svelte';
	import { isLiked } from '$lib/player.svelte';
	import { checkForUpdatesInteractive } from '$lib/updater.svelte';
	import { thumb } from '$lib/thumb';
	import { fallbackArt } from '$lib/fallbackArt';
	import ItemMenu from './ItemMenu.svelte';

	const KIND = { song: 'Song', album: 'Album', artist: 'Artist', playlist: 'Playlist' };

	let query = $state('');
	let items = $state<BrowseItem[]>([]);
	let loading = $state(false);
	let loadedFor = ''; // query `items` belongs to, so a stale response can't land
	// Bound the rendered result list: every row pays for an image decode plus cmdk
	// registration, so an uncapped `{#each}` over a large backend page freezes the main
	// thread on open/keystroke and the frozen thread then can't process Escape/close.
	// The full count stays visible via the label under the list.
	const MAX_RESULTS = 50;
	const displayItems = $derived(items.slice(0, MAX_RESULTS));
	const hiddenCount = $derived(items.length - displayItems.length);
	// Debounce for the remote preview: ~120ms feels instant while still collapsing a
	// burst of keystrokes into one `search_all` call.
	const SEARCH_DEBOUNCE_MS = 120;
	// data-uri SVG placeholders are pure functions of (id, title) but cost a string
	// build + encodeURIComponent each — cache per row so re-renders (keystrokes,
	// stagger remounts) reuse them instead of rebuilding.
	const artCache = new Map<string, string>();
	function cachedArt(item: BrowseItem): string {
		const key = `${item.id}::${item.title}`;
		let hit = artCache.get(key);
		if (!hit) {
			hit = fallbackArt(item.id, item.title);
			// Bounded: one entry per rendered row, cleared when the result set is replaced.
			if (artCache.size > MAX_RESULTS * 2) artCache.clear();
			artCache.set(key, hit);
		}
		return hit;
	}
	// Right-click menu for a result row. The dialog traps pointer events (focus trap +
	// interact-outside), so a menu rendered inside it opens but never receives clicks.
	// Instead the row stashes the search + item + pointer, closes the palette, and the menu
	// below (a sibling of the dialog, in <body> via toBody) opens at the saved point.
	// Dismissing the menu (backdrop click) restores the palette with the search intact;
	// picking an action or moving on to another menu leaves it closed.
	let pendingMenu = $state<{ item: BrowseItem; x: number; y: number } | null>(null);
	let stashed: { query: string; items: BrowseItem[]; loadedFor: string } | null = null;

	function openRowMenu(e: MouseEvent, item: BrowseItem) {
		e.preventDefault(); // no native menu, no cmdk selection
		e.stopPropagation();
		stashed = { query, items, loadedFor };
		pendingMenu = { item, x: e.clientX, y: e.clientY };
		ui.paletteOpen = false;
	}

	function closeRowMenu(reason: import('$lib/menu').MenuCloseReason) {
		pendingMenu = null;
		const s = stashed;
		stashed = null;
		// Backdrop-dismissed: bring the palette back exactly as it was (query, rows and the
		// loaded marker, so no refetch). An action means the user is done; a claim means they
		// moved on to another menu — both stay closed.
		if (reason === 'dismiss' && s) {
			query = s.query;
			items = s.items;
			loadedFor = s.loadedFor;
			ui.paletteOpen = true;
		}
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
		navTouched = false; // a new query means no explicit row choice yet (see paletteKeys)
		if (q.length < 2) {
			items = [];
			loading = false;
			loadedFor = '';
			return;
		}
		if (q === loadedFor) return;
		items = [];
		loading = true;
		const timer = setTimeout(() => load(q), SEARCH_DEBOUNCE_MS);
		return () => clearTimeout(timer);
	});

	// Enter behavior. cmdk auto-highlights the first row, so a bare Enter would play whatever
	// happens to be on top instead of opening the full search page the user asked for. Arrowing
	// to a row first means "that one" and is left alone. Hooked on window in capture phase:
	// the dialog portals its content to <body>, so an ancestor wrapper never sees the events,
	// and cmdk's own Enter listener on the input runs after any window-capture hook.
	// IME composition Enter must reach the input untouched.
	let navTouched = false;
	function paletteKeys(e: KeyboardEvent) {
		if (!ui.paletteOpen) return;
		// Escape must always close, synchronously and without awaiting anything: after
		// arrowing, focus sits on a row (not the input) where the dialog's own Escape
		// handling can lose to the focus trap — and a frozen main thread can't process
		// close at all, which the row cap above prevents. Capture phase so cmdk's own
		// listeners never swallow it first.
		if (e.key === 'Escape') {
			e.preventDefault();
			e.stopPropagation();
			ui.paletteOpen = false;
			return;
		}
		const t = e.target;
		if (!(t instanceof HTMLInputElement)) return;
		if (e.isComposing) return;
		if (
			e.key === 'ArrowUp' ||
			e.key === 'ArrowDown' ||
			e.key === 'Home' ||
			e.key === 'End' ||
			e.key === 'PageUp' ||
			e.key === 'PageDown'
		) {
			navTouched = true;
			return;
		}
		if (e.key !== 'Enter' || navTouched || !query.trim()) return;
		e.preventDefault();
		e.stopPropagation();
		allResults();
	}
	onMount(() => {
		window.addEventListener('keydown', paletteKeys, true);
		return () => window.removeEventListener('keydown', paletteKeys, true);
	});

	// Closing clears the field, which the effect above turns into an empty list: reopening starts
	// fresh instead of on the last search's rows. Opening drops any stale pending menu.
	$effect(() => {
		if (!ui.paletteOpen) query = '';
		else pendingMenu = null;
	});

	let searchError = $state('');
	async function load(q: string) {
		loadedFor = q;
		searchError = '';
		try {
			// Never spin forever: a hung backend must surface as an error, not skeletons.
			const next = await Promise.race([
				searchPreview(q),
				new Promise<never>((_, rej) => setTimeout(() => rej(new Error('timeout')), 8000))
			]);
			if (loadedFor === q) items = next;
		} catch {
			if (loadedFor === q) {
				items = [];
				searchError = 'Search timed out — check your connection and try again.';
			}
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
	// be reactive. `layoutId` (layouts only) marks the active arrangement in the row. `group`
	// (interior #12) buckets every row under an always-visible Jump to / Playback / System header.
	type PaletteGroup = 'Jump to' | 'Playback' | 'System';
	type PaletteAction = { label: string; hint?: string; layoutId?: (typeof LAYOUTS)[number]['id']; group: PaletteGroup; run: () => void };
	const GROUPS: PaletteGroup[] = ['Jump to', 'Playback', 'System'];

	const SETTINGS_TABS: [string, string][] = [
		['general', 'General'],
		['themes', 'Appearance'],
		['playback', 'Playback'],
		['downloads', 'Downloads'],
		['data', 'Data'],
		['about', 'About']
	];

	const ACTIONS: PaletteAction[] = [
		{ label: 'Go to Home', group: 'Jump to', run: () => goto('/') },
		{ label: 'Go to Search', group: 'Jump to', run: () => goto('/search') },
		{ label: 'Go to Library', group: 'Jump to', run: () => goto('/library') },
		{ label: 'Go to History', group: 'Jump to', run: () => goto('/history') },
		{ label: 'Go to Downloads', group: 'Jump to', run: () => goto('/downloads') },
		{ label: 'Open mini player', hint: 'Floating widget', group: 'Playback', run: () => openMiniPlayer() },
		{ label: 'Sleep timer: 15 min', group: 'Playback', run: () => setSleepTimer('minutes', 15) },
		{ label: 'Sleep timer: 30 min', group: 'Playback', run: () => setSleepTimer('minutes', 30) },
		{ label: 'Sleep timer: 60 min', group: 'Playback', run: () => setSleepTimer('minutes', 60) },
		{ label: 'Sleep timer: End of song', group: 'Playback', run: () => setSleepTimer('end_of_song') },
		{ label: 'Sleep timer: Off', group: 'Playback', run: () => setSleepTimer('off') },
		...LAYOUTS.map((l): PaletteAction => ({
			label: `Layout: ${l.label}`,
			hint: l.description,
			layoutId: l.id,
			group: 'System',
			run: () => applyLayout(l.id)
		})),
		{
			label: 'Toggle ambient mode',
			hint: 'Blurred artwork backdrop',
			group: 'System',
			run: () => setAppearance({ ambientMode: !appearance.ambientMode })
		},
		{
			label: 'Toggle artwork accent',
			hint: 'Recolor from the cover',
			group: 'System',
			run: () => setAppearance({ artworkAccent: !appearance.artworkAccent })
		},
		{
			label: 'Toggle tabbed player',
			hint: 'Queue/lyrics tabs in the player view',
			group: 'System',
			run: () => setAppearance({ tabbedPlayer: !appearance.tabbedPlayer })
		},
		...SETTINGS_TABS.map(([id, label]): PaletteAction => ({
			label: `Settings: ${label}`,
			hint: 'Open Settings',
			group: 'System',
			run: () => {
				ui.settingsTab = id;
				ui.settingsOpen = true;
			}
		})),
		{
			label: 'Check for updates',
			group: 'System',
			run: () => {
				checkForUpdatesInteractive().then((r) =>
					r.error ? toast.error(r.message) : toast.success(r.message)
				);
			}
		}
	];

	// The eight rows an empty query opens on (interior #12): jumps, playback (mini player + sleep
	// timer), and settings jumps — every group represented so all three headers always show.
	const DEFAULT_LABELS = [
		'Go to Search',
		'Go to Library',
		'Open mini player',
		'Sleep timer: 30 min',
		'Sleep timer: Off',
		'Settings: Appearance',
		'Settings: Playback',
		'Check for updates'
	];

	function runAction(a: PaletteAction) {
		ui.paletteOpen = false;
		a.run();
	}

	const actionQuery = $derived(query.trim().toLowerCase());
	// The open path does no data work at all — the dialog shell (8 static action rows)
	// renders first and results fill in async via `load()` after the debounce. Never build
	// an index or filter a large list here: any synchronous per-open scan over library /
	// playlist / settings rows freezes the whole app before first paint, and a frozen
	// thread can't process the close that follows. Action filtering below is over ~25
	// static labels (no debounce needed); the remote search is the only debounced path.
	// (A previous {#key}-remount-per-open + stagger replay is deliberately gone: remounting
	// rows on every open churned the underlying item registry and risked focus/selection
	// desync. The dialog's own fade/zoom carries the entrance now.)
	// Substring match on the label; empty query shows the eight defaults above. The results
	// themselves are unfiltered here (shouldFilter={false}) — this list is ours alone.
	const visibleActions = $derived(
		actionQuery
			? ACTIONS.filter((a) => a.label.toLowerCase().includes(actionQuery)).slice(0, 8)
			: DEFAULT_LABELS.map((l) => ACTIONS.find((a) => a.label === l)!).filter(Boolean)
	);
</script>

	<Command.Dialog
	bind:open={ui.paletteOpen}
	onOpenChange={(o) => {
		// Two-way sync: an outside-click close updates bits-ui internally; without
		// this the store stays true and the next Ctrl+K toggles to a no-op false.
		if (!o && ui.paletteOpen) ui.paletteOpen = false;
	}}
	shouldFilter={false}
	vimBindings={false}
	loop
	title="Search"
	description="Search songs, albums, artists and playlists"
	class="palette-content sm:max-w-xl"
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
			<!-- Grouped actions (interior #12): every group carries its header whenever it has rows,
			     so Jump to / Playback / System read as sections, not one long list. No
			     keyed remount here: rows mount once with the dialog and stay registered. -->
			{#each GROUPS as g (g)}
				{@const grows = visibleActions.filter((a) => a.group === g)}
				{#if grows.length}
					<Command.Group heading={g}>
						{#each grows as a (a.label)}
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
			{/each}
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
				{searchError || (query.trim().length < 2 ? 'Type to search.' : 'Nothing quick for that.')}
			</div>
		{:else}
			<Command.Group heading="Results">
				{#each displayItems as item (item.id)}
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
							<img decoding="async" loading="lazy"
								src={thumb(item.thumbnail, 400)}
								alt=""
								class="h-10 w-10 shrink-0 object-cover {item.kind === 'artist'
									? 'rounded-full'
									: 'rounded-md'}"
							/>
						{:else}
							{#if item.kind === 'artist'}
								<div
									class="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-muted text-muted-foreground/50"
								>
									<HugeiconsIcon icon={UserIcon} class="h-5 w-5" />
								</div>
							{:else}
								<img decoding="async" loading="lazy"
									src={cachedArt(item)}
									alt=""
									class="h-10 w-10 shrink-0 rounded-md object-cover"
								/>
							{/if}
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
			{#if hiddenCount > 0}
				<p class="px-4 py-1.5 text-center text-xs text-muted-foreground">
					Showing {displayItems.length} of {items.length} — Enter for all results
				</p>
			{/if}
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
	<!-- Key hint (interior #12): Enter opens the full results until ↑↓ picks a row (see paletteKeys). -->
	<p class="border-t px-3 py-2 text-[11px]" style="color:var(--text-3);border-color:var(--border)">
		↑↓ to pick a row · Enter to open
	</p>
	<!-- No visible trigger: a palette row is too small for a hover-only ⋯, and the menu only ever
	     opens from a right-click (see `openRowMenu`). Rendered here, outside the dialog, so the
	     dialog's focus trap can't swallow the menu's clicks. 		Keyed per open for a fresh instance. -->
</Command.Dialog>
{#if pendingMenu}
	{#key `${pendingMenu.item.id}-${pendingMenu.x}-${pendingMenu.y}`}
		<ItemMenu
			item={pendingMenu.item}
			triggerClass="hidden"
			openAt={{ x: pendingMenu.x, y: pendingMenu.y }}
			onclose={closeRowMenu}
		/>
	{/key}
{/if}

<style>
	/* Palette close: fade + slide down + settle scale, coordinated with the dialog open state
	   (bits-ui holds the content through data-state="closed" before unmounting). Slightly
	   longer than a menu pop so the list reads as sinking away, not blinking out. */
	:global([data-slot='dialog-content'].palette-content[data-state='closed']) {
		animation: paletteOut 180ms var(--ease-out) forwards;
	}
	@keyframes paletteOut {
		from { opacity: 1; transform: translateY(0) scale(1); }
		to { opacity: 0; transform: translateY(8px) scale(0.98); }
	}
	@media (prefers-reduced-motion: reduce) {
		:global([data-slot='dialog-content'].palette-content[data-state='closed']) {
			animation: none;
		}
	}
</style>
