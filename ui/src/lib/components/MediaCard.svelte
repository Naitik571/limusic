<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		PlayIcon,
		MusicNote01Icon,
		UserIcon,
		ListRestartIcon
	} from '@hugeicons/core-free-icons';
	import { ON_REPEAT_ID } from '$lib/api';
	import type { BrowseItem } from '$lib/api';
	import { thumb } from '$lib/thumb';
	import { setDragItem } from '$lib/dnd';
	import { asSong, openItem, playItem } from '$lib/browse';
	import { openAddToPlaylist } from '$lib/player.svelte';
	import TrackMenu from './TrackMenu.svelte';
	import PlaylistMenu from './PlaylistMenu.svelte';

	let { item, compact = false }: { item: BrowseItem; compact?: boolean } = $props();

	const round = $derived(item.kind === 'artist');
	// On Repeat has no artwork by nature, so its cover is the icon rather than the neutral
	// placeholder every failed thumbnail lands on.
	const onRepeat = $derived(item.id === ON_REPEAT_ID);

	// Google's CDN doesn't serve every rewritten size — asking for one it doesn't have 404s, and the
	// browser then paints its broken-image glyph. So: try the sized URL, retry the original once, and
	// only then fall back to a neutral icon tile.
	// ponytail: 400 for every card, small tiles included — it's the size proven to work everywhere.
	let attempt = $state(0);
	$effect(() => {
		item.thumbnail; // re-arm when the card is reused for a different item
		attempt = 0;
	});
	const sized = $derived(thumb(item.thumbnail, 400));
	const src = $derived(attempt === 0 ? sized : item.thumbnail);
	// Skip the retry when `thumb` left the URL untouched — it would refetch the same dead URL.
	const imgFailed = () => (attempt = attempt === 0 && sized !== item.thumbnail ? 1 : 2);

	let playing = $state(false); // in-flight guard for the fetch-then-play path

	async function playNow() {
		if (playing) return;
		playing = true;
		try {
			await playItem(item);
		} finally {
			playing = false;
		}
	}
</script>

<div class="group relative flex w-full flex-col gap-2">
	<!-- draggable: every card is a drag source for home's Shortcuts grid (the only drop target). -->
	<div
		class="flex flex-col text-left transition-colors hover:bg-accent/10 {compact
			? 'gap-1.5 rounded-lg p-1.5'
			: 'gap-2 rounded-xl p-2'}"
		role="button"
		tabindex="0"
		draggable="true"
		ondragstart={(e) => setDragItem(e, item)}
		onclick={() => openItem(item)}
		onkeydown={(e) => {
			if (e.target !== e.currentTarget) return;
			if (e.key === 'Enter' || e.key === ' ') {
				e.preventDefault();
				openItem(item);
			}
		}}
		title={item.subtitle ? `${item.title} — ${item.subtitle}` : item.title}
	>
		<div
			class="relative aspect-square w-full overflow-hidden bg-muted shadow-sm transition-shadow duration-300 group-hover:shadow-xl {round
				? 'rounded-full'
				: 'rounded-lg'}"
		>
			{#if item.thumbnail && attempt < 2 && !onRepeat}
				<img
					{src}
					alt=""
					class="h-full w-full object-cover transition-transform duration-300 ease-out group-hover:scale-105"
					loading="lazy"
					draggable="false"
					onerror={imgFailed}
				/>
			{:else}
				<div
					class="flex h-full w-full items-center justify-center {onRepeat
						? 'bg-primary/10 text-primary'
						: 'text-muted-foreground/50'}"
				>
					<!-- altIcon/showAlt, not a third ternary: `icon` is read once at mount. -->
					<HugeiconsIcon
						icon={round ? UserIcon : MusicNote01Icon}
						altIcon={ListRestartIcon}
						showAlt={onRepeat}
						class={onRepeat ? (compact ? 'h-7 w-7' : 'h-10 w-10') : compact ? 'h-5 w-5' : 'h-7 w-7'}
					/>
				</div>
			{/if}
			{#if item.kind !== 'artist'}
				<button
					class="absolute flex translate-y-1 cursor-pointer items-center justify-center rounded-full bg-primary text-primary-foreground opacity-0 shadow-lg transition-all duration-200 ease-out group-hover:translate-y-0 group-hover:opacity-100 focus-visible:opacity-100 {compact
						? 'bottom-1.5 right-1.5 h-7 w-7'
						: 'bottom-2 right-2 h-9 w-9'}"
					class:animate-pulse={playing}
					disabled={playing}
					aria-label="Play"
					onclick={(e) => {
						e.stopPropagation();
						playNow();
					}}
				>
					<HugeiconsIcon icon={PlayIcon} class={compact ? 'h-3 w-3' : 'h-4 w-4'} />
				</button>
			{/if}
		</div>
		<div class="min-w-0 {round ? 'text-center' : ''}">
			<div class="truncate font-medium {compact ? 'text-xs' : 'text-sm'}">{item.title}</div>
			{#if item.subtitle}
				<div class="truncate text-muted-foreground {compact ? 'text-[0.6875rem]' : 'text-xs'}">
					{item.subtitle}
				</div>
			{/if}
		</div>
	</div>
	{#if item.kind === 'song'}
		<TrackMenu
			song={asSong(item)}
			onAdd={() => openAddToPlaylist(asSong(item))}
			triggerClass="absolute right-3 top-3 flex h-8 w-8 items-center justify-center rounded-full bg-background/80 text-foreground opacity-0 shadow-md backdrop-blur-sm transition hover:bg-background focus-visible:opacity-100 group-hover:opacity-100 cursor-pointer"
		/>
	{:else}
		<PlaylistMenu
			{item}
			showPin={item.kind === 'playlist'}
			triggerClass="absolute right-3 top-3 flex h-8 w-8 items-center justify-center rounded-full bg-background/80 text-foreground opacity-0 shadow-md backdrop-blur-sm transition hover:bg-background focus-visible:opacity-100 group-hover:opacity-100 cursor-pointer"
		/>
	{/if}
</div>
