<script lang="ts">
	// Poolside Now Playing: two vinyl decks (current + up next), queue strip, seek + transport.
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		PlayIcon,
		PauseIcon,
		PreviousIcon,
		NextIcon,
		ShuffleIcon,
		RepeatIcon,
		RepeatOne01Icon,
		FavouriteIcon,
		VolumeHighIcon,
		VolumeMute02Icon
	} from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import {
		playback,
		commitVolume,
		dragVolume,
		cycleRepeat,
		toggleNowPlayingLike,
		playSong,
		toast
	} from '$lib/player.svelte';
	import Vinyl from './Vinyl.svelte';

	let { onOpenLibrary }: { onOpenLibrary: () => void } = $props();

	const cur = $derived(playback.now);
	const paused = $derived(playback.paused);
	const q = $derived(playback.queue);
	const nextItem = $derived(q.items[q.currentIndex + 1] ?? null);

	const artCur = $derived(cur?.thumbnail ?? '');
	const artNext = $derived(nextItem?.thumbnail ?? '');

	// upcoming queue strip (after the current track)
	const upcoming = $derived(q.items.slice(q.currentIndex + 1, q.currentIndex + 8));
	// recently played strip (before the current track, newest first)
	const recents = $derived(
		q.items
			.slice(Math.max(0, q.currentIndex - 5), q.currentIndex)
			.slice()
			.reverse()
	);

	const pos = $derived(Math.min(playback.position, playback.duration || playback.position));
	const dur = $derived(playback.duration || 0);

	function fmt(s: number): string {
		if (!s || Number.isNaN(s)) return '0:00';
		const t = Math.max(0, Math.floor(s));
		return `${Math.floor(t / 60)}:${String(t % 60).padStart(2, '0')}`;
	}

	function seek(e: Event) {
		const v = Number((e.currentTarget as HTMLInputElement).value);
		api.seek(v).catch(() => {});
	}
	function playIndex(i: number) {
		api.playIndex(i).catch((e) => toast.error(String(e)));
	}
</script>

<div class="ps-view on ps-np">
	<div class="ps-np-head">
		<div class="kicker">Now Playing · 33⅓ RPM</div>
		<div class="title ps-title-glow">{cur ? cur.title.toUpperCase() : 'Nothing playing'}</div>
		<div class="artist">{cur ? cur.artists : 'Pick something from the library'}</div>
	</div>

	<div class="ps-stage">
		<!-- current disc, sliding out of its kraft sleeve -->
		<div class="ps-deck-unit">
			<div class="ps-sleeve"><div class="mouth"></div></div>
			<Vinyl
				src={artCur}
				playing={!paused}
				style="width:100%"
				flightTarget
				title="Current track"
			/>
			<!-- SIDE A sticker -->
			<svg class="ps-sticker-badge" width="56" height="56" viewBox="0 0 58 58">
				<circle cx="29" cy="29" r="26" fill="#fff" stroke="#111" stroke-width="3" />
				<text x="29" y="26" text-anchor="middle" font-family="monospace" font-size="9" font-weight="bold" fill="#111">SIDE</text>
				<text x="29" y="41" text-anchor="middle" font-family="monospace" font-size="13" font-weight="bold" fill="#E02020">A</text>
			</svg>
			<div class="ps-deck-label">Picture disc · side A</div>
		</div>

		<!-- up-next disc -->
		{#if nextItem}
			<div class="ps-deck-unit">
				<div class="ps-sleeve"><div class="mouth"></div></div>
				<Vinyl src={artNext} playing={false} style="width:100%" title="Up next" />
				<div class="ps-deck-label">Up next</div>
			</div>
		{/if}

		<!-- queue panel -->
		{#if upcoming.length}
			<aside class="ps-queue-panel ps-glass" aria-label="Up next queue">
				<div class="ps-q-label"><span class="ps-q-dot"></span>Up next</div>
				{#each upcoming as item, i (item.video_id + i)}
					<div
						class="ps-queue-row"
						role="button"
						tabindex="0"
						onclick={() => playIndex(q.currentIndex + 1 + i)}
						onkeydown={(e) => e.key === 'Enter' && playIndex(q.currentIndex + 1 + i)}
					>
						{#if item.thumbnail}
							<img src={item.thumbnail} alt="" />
						{/if}
						<div class="min-w-0">
							<div class="t">{item.title.toUpperCase()}</div>
							<div class="a">{item.artists}</div>
						</div>
						<div class="d">{item.duration ?? ''}</div>
					</div>
				{/each}
			</aside>
		{/if}
	</div>

	<div class="ps-bottom">
		<div class="ps-seek-row">
			<span>{fmt(pos)}</span>
			<input
				class="ps-seek"
				type="range"
				min="0"
				max={Math.max(1, Math.floor(dur))}
				value={Math.floor(pos)}
				oninput={seek}
				aria-label="Seek"
			/>
			<span>{fmt(dur)}</span>
		</div>
		<div class="ps-transport">
			<button
				class="ps-tbtn {q.shuffle ? 'on' : ''}"
				onclick={() => api.toggleShuffle().catch(() => {})}
				title="Shuffle"
				aria-label="Shuffle"
			>
				<HugeiconsIcon icon={ShuffleIcon} />
			</button>
			<button class="ps-tbtn" onclick={() => api.prevTrack().catch(() => {})} title="Previous" aria-label="Previous">
				<HugeiconsIcon icon={PreviousIcon} />
			</button>
			<button
				class="ps-tbtn ps-play ps-aqua"
				onclick={() => api.togglePause().catch(() => {})}
				title="Play / pause"
				aria-label="Play or pause"
			>
				{#if paused}<HugeiconsIcon icon={PlayIcon} />{:else}<HugeiconsIcon icon={PauseIcon} />{/if}
			</button>
			<button class="ps-tbtn" onclick={() => api.nextTrack().catch(() => {})} title="Next" aria-label="Next">
				<HugeiconsIcon icon={NextIcon} />
			</button>
			<button
				class="ps-tbtn {q.repeat !== 'off' ? 'on' : ''}"
				onclick={() => cycleRepeat().catch(() => {})}
				title="Repeat"
				aria-label="Repeat"
			>
				<HugeiconsIcon icon={q.repeat === 'one' ? RepeatOne01Icon : RepeatIcon} />
			</button>
			<button
				class="ps-tbtn {playback.liked ? 'on' : ''}"
				onclick={() => toggleNowPlayingLike()}
				title="Like"
				aria-label="Like"
			>
				<HugeiconsIcon icon={FavouriteIcon} />
			</button>
			<div class="hidden items-center gap-2 md:flex">
				<button
					class="ps-tbtn"
					onclick={() => commitVolume(playback.volume === 0 ? 100 : 0)}
					title="Mute"
					aria-label="Mute"
				>
					<HugeiconsIcon icon={playback.volume === 0 ? VolumeMute02Icon : VolumeHighIcon} />
				</button>
				<input
					type="range"
					min="0"
					max="100"
					value={playback.volume}
					oninput={(e) => dragVolume(Number(e.currentTarget.value))}
					onchange={(e) => commitVolume(Number(e.currentTarget.value))}
					class="w-20 accent-white"
					aria-label="Volume"
				/>
			</div>
		</div>
		{#if recents.length}
			<div class="ps-recent" aria-label="Recently played">
				{#each recents as item (item.video_id)}
					<button
						title={item.title}
						onclick={() => {
							const idx = q.items.indexOf(item);
							if (idx >= 0) playIndex(idx);
							else playSong(item);
						}}
					>
						{#if item.thumbnail}<img src={item.thumbnail} alt={item.title} />{/if}
					</button>
				{/each}
			</div>
		{/if}
	</div>

	<div class="absolute bottom-4 left-6 text-[9px] tracking-[0.28em] uppercase opacity-70">
		<button class="hover:text-white cursor-pointer" onclick={onOpenLibrary}>Logo = library · click a sleeve for the album</button>
	</div>
</div>
