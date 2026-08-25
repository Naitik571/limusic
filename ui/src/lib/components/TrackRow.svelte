<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		FavouriteIcon,
		MusicNote01Icon,
		PlayIcon,
		PlayListAddIcon,
		DownloadSquare01Icon
	} from '@hugeicons/core-free-icons';
	import type { SongItem } from '$lib/api';
	import { thumb } from '$lib/thumb';
	import { lt } from '$lib/lt.svelte';
	import { isLiked, toggleLike, downloadedIds, likeBursts } from '$lib/player.svelte';
	import { flyPlus } from '$lib/fx';
	import TrackMenu from './TrackMenu.svelte';
	import ArtistLine from './ArtistLine.svelte';

	let {
		song,
		index,
		active = false,
		hideThumb = false,
		compact = false,
		onplay,
		onAdd,
		onRemove,
		removeLabel = 'Remove from playlist',
		highlight = ''
	}: {
		song: SongItem;
		/** Position badge when set (playlist/queue); omitted for flat search results. */
		index?: number;
		active?: boolean;
		/** Hide the leading thumbnail (album track lists show a number, not a cover). */
		hideThumb?: boolean;
		/**
		 * Grid variant (home's Forgotten favourites): the duration joins the artist line instead of
		 * claiming its own column, and a like heart sits next to the ⋯ — narrow columns have no room
		 * for a separate duration column, and hearting is the whole point of that shelf.
		 */
		compact?: boolean;
		onplay: () => void;
		/** Adds an "Add to playlist" menu item. */
		onAdd?: () => void;
		/** Adds a remove menu item (label via `removeLabel`). */
		onRemove?: () => void;
		removeLabel?: string;
		/** Query to highlight inside title/artists (playlist type-anywhere filter). */
		highlight?: string;
	} = $props();

	function highlightParts(text: string, query: string): { text: string; match: boolean }[] {
		const q = query.trim();
		if (!q) return [{ text, match: false }];
		const esc = q.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
		const re = new RegExp(`(${esc})`, 'gi');
		const parts = text.split(re);
		const lower = q.toLowerCase();
		return parts.map((p) => ({ text: p, match: p.toLowerCase() === lower }));
	}

	// In a session as guest, clicking a song adds it to the shared queue instead of playing it —
	// reflect that in the hover icon + label so the row doesn't lie.
	const guestAdd = $derived(lt.role === 'guest');

	// Like burst: a ~600ms class on the heart that runs the spark keyframes (layout.css). Only when
	// the toggle turns liking ON — unliking stays quiet. The trigger is the shared `likeBursts`
	// stamp (player.svelte), so EVERY like-on path for this song pops the heart — the row's own
	// button, the ⋯ menu item, the player bar — not just whichever one was clicked. `shownAt`
	// guards against replaying a stale entry when the list remounts.
	let burst = $state(false);
	let burstTimer: ReturnType<typeof setTimeout> | undefined;
	let shownBurstAt = 0;

	$effect(() => {
		const at = likeBursts[song.video_id] ?? 0;
		if (at === 0 || at <= shownBurstAt || Date.now() - at > 1000) return;
		shownBurstAt = at;
		burst = true;
		clearTimeout(burstTimer);
		burstTimer = setTimeout(() => (burst = false), 600);
	});

	function like() {
		toggleLike(song);
	}

	// The add-to-playlist action lives in TrackMenu's items, so the click coordinates never reach
	// this component directly — track the last pointer press globally (capture phase, so menu
	// item clicks included) and fly the "+" from there before handing off to onAdd.
	let lastPoint = { x: 0, y: 0 };

	$effect(() => {
		const track = (e: PointerEvent) => {
			lastPoint.x = e.clientX;
			lastPoint.y = e.clientY;
		};
		window.addEventListener('pointerdown', track, true);
		return () => window.removeEventListener('pointerdown', track, true);
	});

	const handleAdd = $derived(
		onAdd
			? () => {
					flyPlus(lastPoint.x, lastPoint.y);
					onAdd();
				}
			: undefined
	);

	// The whole row is a play target (role="button"), so mirror native button keyboard activation.
	// Only when the key lands on the row itself — keydowns bubble up from nested interactive
	// elements (⋯ menu, artist link), and hijacking those would play the row instead.
	function onKey(e: KeyboardEvent) {
		if (e.target !== e.currentTarget) return;
		if (e.key === 'Enter' || e.key === ' ') {
			e.preventDefault();
			onplay();
		}
	}
</script>

<!-- content-visibility: a liked-songs playlist runs to thousands of rows and WebKit keeps every one
     in style, layout and paint. 3.5rem is a row (8px padding, 40px thumbnail, 8px); `auto` swaps in
     the measured size after first paint. Not on the compact variant: that one is laid out in CSS
     columns (ForgottenFavourites), where an unsized fragment would upset column balancing, and it
     never has more than 15 rows to skip. -->
<div
	role="button"
	tabindex="0"
	data-ctx
	data-active={active ? 'true' : undefined}
	onclick={onplay}
	onkeydown={onKey}
	aria-label={guestAdd ? `Add ${song.title} to the session queue` : `Play ${song.title}`}
	class="group flex w-full cursor-pointer items-center gap-3 rounded-xl p-2 transition-colors hover:bg-glass {active
		? 'bg-accent/10'
		: ''} {compact ? '' : '[content-visibility:auto] [contain-intrinsic-size:auto_3.5rem]'}"
>
	<div class="flex min-w-0 flex-1 items-center gap-3">
		<div class="flex min-w-0 shrink-0 items-center gap-3">
			{#if index !== undefined}
				<span
					class="relative w-5 shrink-0 text-center text-xs {active
						? 'text-primary'
						: 'text-muted-foreground'}"
				>
					<span class="group-hover:opacity-0">{index + 1}</span>
					<HugeiconsIcon
						icon={guestAdd ? PlayListAddIcon : PlayIcon}
						class="absolute inset-0 m-auto h-3.5 w-3.5 opacity-0 group-hover:opacity-100"
					/>
				</span>
			{/if}
			{#if !hideThumb}
				{#if song.thumbnail}
					<img src={thumb(song.thumbnail, 96)} alt="" draggable={false} class="h-10 w-10 shrink-0 rounded-md object-cover" loading="lazy" />
				{:else}
					<!-- An untagged file has no artwork of its own. A music note keeps the row aligned
					     with its neighbours and says so plainly. -->
					<div
						class="flex h-10 w-10 shrink-0 items-center justify-center rounded-md bg-muted text-muted-foreground/50"
					>
						<HugeiconsIcon icon={MusicNote01Icon} class="h-4 w-4" />
					</div>
				{/if}
			{/if}
		</div>
		<div class="min-w-0 flex-1">
			<div class="flex min-w-0 items-center gap-2">
				<span class="min-w-0 truncate text-sm font-medium {active ? 'text-primary' : ''}">
					{#if highlight.trim()}
						{#each highlightParts(song.title, highlight) as part}
							{#if part.match}<mark class="rounded bg-primary/30 px-0.5 text-primary">{part.text}</mark>{:else}{part.text}{/if}
						{/each}
					{:else}
						{song.title}
					{/if}
				</span>
				{#if song.queued_by}
					<span
						class="shrink-0 rounded-full bg-primary/10 px-1.5 py-0.5 text-[10px] font-medium text-primary"
					>
						{song.queued_by}
					</span>
				{/if}
			</div>
			<div class="flex min-w-0 items-center gap-1 text-xs text-muted-foreground">
				<ArtistLine runs={song.artist_runs} text={song.artists} />
				{#if compact && song.duration}
					<span class="shrink-0">· {song.duration}</span>
				{/if}
			</div>
		</div>
	</div>

	<div class="flex shrink-0 items-center {compact ? 'gap-0.5' : 'gap-2'}">
		{#if downloadedIds.has(song.video_id) && !compact}
			<!-- Persistent, not hover-only: "saved for offline" is state the row has to keep showing. -->
			<span class="shrink-0 text-primary" title="Downloaded — plays offline">
				<HugeiconsIcon icon={DownloadSquare01Icon} class="h-4 w-4" />
			</span>
		{/if}
		{#if song.duration && !compact}
			<span class="text-xs text-muted-foreground">{song.duration}</span>
		{/if}
		{#if compact}
			<!-- Persistent, not hover-only: a filled heart is state the row has to keep showing. -->
			<button
				class="relative cursor-pointer rounded-md p-1.5 text-muted-foreground transition hover:bg-accent/20 hover:text-foreground"
				aria-label={isLiked(song) ? 'Remove from liked songs' : 'Save to liked songs'}
				aria-pressed={isLiked(song)}
				onclick={(e) => {
					e.stopPropagation();
					like();
				}}
			>
				{#if burst}
					<!-- 6 sparks flying outward; angles are spread by --a in the keyframes (layout.css). -->
					{#each Array(6) as _, i}
						<span class="heart-spark" style="--a:{i * 60}deg" aria-hidden="true"></span>
					{/each}
				{/if}
				<HugeiconsIcon
					icon={FavouriteIcon}
					class="h-4 w-4 {isLiked(song) ? 'fill-current text-primary' : ''} {burst ? 'heart-pop' : ''}"
				/>
			</button>
		{/if}
		<TrackMenu
			{song}
			onAdd={handleAdd}
			{onRemove}
			{removeLabel}
			triggerClass="cursor-pointer rounded-md p-1.5 text-muted-foreground transition hover:bg-accent/20 hover:text-foreground focus-visible:opacity-100 {compact
				? ''
				: 'opacity-0 group-hover:opacity-100'}"
		/>
	</div>
</div>
