<script lang="ts">
	// Poolside Now Playing — real turntable with draggable disc.
	// Pull the disc off the platter to stop music; drop it back to play.
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		PlayIcon, PauseIcon, PreviousIcon, NextIcon,
		ShuffleIcon, RepeatIcon, RepeatOne01Icon,
		FavouriteIcon, VolumeHighIcon, VolumeMute02Icon
	} from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import {
		playback, commitVolume, dragVolume,
		cycleRepeat, toggleNowPlayingLike, toast
	} from '$lib/player.svelte';
	import Vinyl from './Vinyl.svelte';

	let { onOpenLibrary }: { onOpenLibrary: () => void } = $props();

	const cur = $derived(playback.now);
	const paused = $derived(playback.paused);
	const q = $derived(playback.queue);
	const nextItem = $derived(q.items[q.currentIndex + 1] ?? null);
	const upcoming = $derived(q.items.slice(q.currentIndex + 1, q.currentIndex + 7));
	const pos = $derived(Math.min(playback.position, playback.duration || playback.position));
	const dur = $derived(playback.duration || 0);

	function fmt(s: number): string {
		if (!s || Number.isNaN(s)) return '0:00';
		const t = Math.max(0, Math.floor(s));
		return `${Math.floor(t / 60)}:${String(t % 60).padStart(2, '0')}`;
	}
	function seek(e: Event) {
		api.seek(Number((e.currentTarget as HTMLInputElement).value)).catch(() => {});
	}
	function playIndex(i: number) {
		api.playIndex(i).catch((e) => toast.error(String(e)));
	}

	// drag-to-eject: pull disc off = stop, drop back = play
	let dragX = $state(0);
	let dragY = $state(0);
	let isDragging = $state(false);
	let dragStartX = 0;
	let dragStartY = 0;

	function onDiscPointerDown(e: PointerEvent) {
		if (!cur) return;
		isDragging = true;
		dragStartX = e.clientX;
		dragStartY = e.clientY;
		(e.target as HTMLElement).setPointerCapture(e.pointerId);
	}
	function onDiscPointerMove(e: PointerEvent) {
		if (!isDragging) return;
		dragX = e.clientX - dragStartX;
		dragY = e.clientY - dragStartY;
	}
	function onDiscPointerUp() {
		if (!isDragging) return;
		const distance = Math.sqrt(dragX * dragX + dragY * dragY);
		if (distance > 100) {
			// ejected — stop playback
			if (!paused) api.togglePause().catch(() => {});
			toast.info('Disc removed — music paused');
		}
		// snap back
		dragX = 0;
		dragY = 0;
		isDragging = false;
	}
	function onDiscDoubleClick() {
		if (!cur) return;
		// drop disc back = play
		if (paused) api.togglePause().catch(() => {});
	}
</script>

<div class="ps-np">
	<!-- turntable with plinth, platter, and tonearm -->
	<div class="ps-turntable">
		<div class="ps-plinth">
			<div class="ps-platter"></div>
			<div class="ps-tonearm">
				<div class="ps-arm-pivot"></div>
				<div class="ps-arm-shaft">
					<div class="ps-arm-head"></div>
				</div>
			</div>
			<div class="ps-speed-badge">33⅓ RPM</div>
			<div class="ps-power-led"></div>
		</div>
	</div>

	<!-- deck: disc sitting on the turntable -->
	<div class="ps-deck">
		<div
			class="ps-deck-unit"
			class:ejecting={isDragging && Math.sqrt(dragX * dragX + dragY * dragY) > 60}
			role="application"
			aria-label="Vinyl disc — drag to eject, double-click to drop back"
			onpointerdown={onDiscPointerDown}
			onpointermove={onDiscPointerMove}
			onpointerup={onDiscPointerUp}
			onpointercancel={onDiscPointerUp}
			ondblclick={onDiscDoubleClick}
		>
			<div class="ps-sleeve"><div class="mouth"></div></div>
			<div style="
				position: absolute; width: 78%; left: -14%; top: 50%;
				transform: translateY(-50%) translate({dragX}px, {dragY}px);
				transition: {isDragging ? 'none' : 'left .7s var(--ease-spring), transform .5s'};
			">
				<Vinyl
					src={cur?.thumbnail ?? ''}
					playing={!paused && !isDragging}
					style="width:100%"
					flightTarget
					title="Drag to eject · Double-click to drop back"
				/>
			</div>
			<svg class="ps-sticker" style="right:-4%;top:-7%" width="56" height="56" viewBox="0 0 58 58">
				<circle cx="29" cy="29" r="26" fill="#fff" stroke="#111" stroke-width="3" />
				<text x="29" y="33" text-anchor="middle" font-family="monospace" font-size="7" font-weight="bold" letter-spacing="1.5" fill="#111">NOW PLAYING</text>
			</svg>
		</div>
		{#if nextItem}
			<div class="ps-deck-unit">
				<div class="ps-sleeve"><div class="mouth"></div></div>
				<Vinyl src={nextItem.thumbnail ?? ''} playing={false} style="width:100%" title="Up next" />
				<div class="ps-eject-hint">UP NEXT</div>
			</div>
		{/if}
	</div>

	<!-- caption: clear size hierarchy -->
	<div class="ps-track-caption">
		<span class="np-label">{cur ? 'NOW PLAYING' : 'NO TRACK LOADED'}</span>
		<span class="np-title">{cur ? cur.title : 'Drop a disc onto the turntable'}</span>
		<span class="np-artist">{cur ? cur.artists : ''}</span>
	</div>

	<!-- queue strip: quiet thumbnails at the bottom -->
	{#if upcoming.length}
		<div class="ps-queue-strip">
			{#each upcoming as item (item.video_id + item.queued_from)}
				<button
					class="qtile"
					title={item.title}
					onclick={() => {
						const idx = q.items.indexOf(item);
						if (idx >= 0) playIndex(idx);
					}}
				>
					{#if item.thumbnail}<img src={item.thumbnail} alt={item.title} />{/if}
				</button>
			{/each}
		</div>
	{/if}

	<!-- seek + transport -->
	<div class="ps-bottom">
		<div class="ps-seek-row">
			<span>{fmt(pos)}</span>
			<input class="ps-seek" type="range" min="0" max={Math.max(1, Math.floor(dur))} value={Math.floor(pos)} oninput={seek} aria-label="Seek" />
			<span>{fmt(dur)}</span>
		</div>
		<div class="ps-transport">
			<button class="ps-tbtn {q.shuffle ? 'on' : ''}" onclick={() => api.toggleShuffle().catch(() => {})} title="Shuffle" aria-label="Shuffle">
				<HugeiconsIcon icon={ShuffleIcon} />
			</button>
			<button class="ps-tbtn" onclick={() => api.prevTrack().catch(() => {})} title="Previous" aria-label="Previous">
				<HugeiconsIcon icon={PreviousIcon} />
			</button>
			<button class="ps-tbtn ps-play ps-aqua" onclick={() => api.togglePause().catch(() => {})} title="Play / pause" aria-label="Play or pause">
				{#if paused}<HugeiconsIcon icon={PlayIcon} />{:else}<HugeiconsIcon icon={PauseIcon} />{/if}
			</button>
			<button class="ps-tbtn" onclick={() => api.nextTrack().catch(() => {})} title="Next" aria-label="Next">
				<HugeiconsIcon icon={NextIcon} />
			</button>
			<button class="ps-tbtn {q.repeat !== 'off' ? 'on' : ''}" onclick={() => cycleRepeat().catch(() => {})} title="Repeat" aria-label="Repeat">
				<HugeiconsIcon icon={q.repeat === 'one' ? RepeatOne01Icon : RepeatIcon} />
			</button>
			<button class="ps-tbtn {playback.liked ? 'on' : ''}" onclick={() => toggleNowPlayingLike()} title="Like" aria-label="Like">
				<HugeiconsIcon icon={FavouriteIcon} />
			</button>
			<div class="hidden items-center gap-2 md:flex">
				<button class="ps-tbtn" onclick={() => commitVolume(playback.volume === 0 ? 100 : 0)} title="Mute" aria-label="Mute">
					<HugeiconsIcon icon={playback.volume === 0 ? VolumeMute02Icon : VolumeHighIcon} />
				</button>
				<input type="range" min="0" max="100" value={playback.volume}
					oninput={(e) => dragVolume(Number(e.currentTarget.value))}
					onchange={(e) => commitVolume(Number(e.currentTarget.value))}
					class="w-20 accent-white" aria-label="Volume" />
			</div>
		</div>
	</div>

	<div class="ps-hint">
		<button onclick={onOpenLibrary} class="cursor-pointer hover:text-white">Logo = library</button>
	</div>
</div>
