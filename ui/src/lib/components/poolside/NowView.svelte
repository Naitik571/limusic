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

	// drag-to-eject: pull the disc past the threshold and release -> music pauses. Drag back
	// to centre and release -> the disc snaps home. One threshold (60px) for both the visual
	// "ejecting" class and the eject action, so there's no dead zone where it looks like it's
	// ejecting but does nothing.
	const EJECT_PX = 60;
	let dragX = $state(0);
	let dragY = $state(0);
	let isDragging = $state(false);
	let dragStartX = 0;
	let dragStartY = 0;
	let dragStartState: { paused: boolean } | null = null;
	let ejected = $state(false);

	// Tonearm angle: -15° (rest) -> -28° (cue-down, track start) and then drifts toward
	// -34° (track end, near the center). Real tonearms don't really do this, but it
	// sells "this turntable is reading the record".
	const tonearmAngle = $derived(() => {
		if (!cur) return -15;
		if (ejected) return -8; // lifted off entirely
		if (paused) return -15;
		if (dur <= 0) return -28;
		// 0 -> -28 (cue), 1 -> -34 (end of side)
		return -28 - Math.min(1, pos / dur) * 6;
	});
	// Spinning the platter independently of the disc gives a sense of "the platter
	// is what drives the disc". The disc has its own wobble + sheen rotation.
	const platterSpinning = $derived(!paused && !!cur);

	// Play button: brief "pressed" class adds a ring ripple, and on every play action
	// we spawn 2-3 aqua bubbles that float up from the button.
	let playBtn = $state<HTMLButtonElement>();
	let playPressed = $state(false);
	function onPlayPress(e: MouseEvent) {
		playPressed = true;
		setTimeout(() => (playPressed = false), 650);
		// spawn bubbles from the click point
		if (playBtn) {
			const r = playBtn.getBoundingClientRect();
			const cx = e.clientX - r.left;
			const cy = e.clientY - r.top;
			for (let i = 0; i < 3; i++) {
				const b = document.createElement('span');
				b.className = 'ps-bubble';
				const size = 4 + Math.random() * 5;
				b.style.cssText = `width:${size}px;height:${size}px;left:${cx}px;top:${cy}px;--bx:${(Math.random() - 0.5) * 30}px;`;
				playBtn.appendChild(b);
				setTimeout(() => b.remove(), 1500);
			}
		}
	}

	async function onPlayClick(e: MouseEvent) {
		onPlayPress(e);
		await api.togglePause().catch(() => {});
	}

	function onDiscPointerDown(e: PointerEvent) {
		if (!cur) return;
		isDragging = true;
		dragStartX = e.clientX;
		dragStartY = e.clientY;
		// remember the playing state at the start of the drag so we only resume if the user
		// was actually playing when they grabbed the disc (don't un-pause a paused track).
		dragStartState = { paused };
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
		if (distance > EJECT_PX && !ejected) {
			// drag-out: pause and "eject" — the disc slides off-screen
			if (!paused) api.togglePause().catch(() => {});
			toast.info('Disc removed — music paused');
			ejected = true;
		} else if (distance <= EJECT_PX && ejected) {
			// drag-back from ejected: resume only if we were playing when the drag started
			if (dragStartState && !dragStartState.paused) api.togglePause().catch(() => {});
			ejected = false;
		}
		// snap back
		dragX = 0;
		dragY = 0;
		isDragging = false;
	}
	function onDiscDoubleClick() {
		// explicit "drop the disc back" gesture, even without a drag — useful for the small
		// quick tap that doesn't register as a drag.
		if (ejected) {
			if (dragStartState && !dragStartState.paused) api.togglePause().catch(() => {});
			ejected = false;
		}
	}
</script>

<div class="ps-np">
	<!-- turntable with plinth, platter, and tonearm -->
	<div class="ps-turntable">
		<div class="ps-plinth">
			<div class="ps-platter" class:spin={platterSpinning}></div>
			<div class="ps-tonearm" style="transform: rotate({tonearmAngle()}deg);">
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
			class:ejected
			role="application"
			aria-label="Vinyl disc — drag off to pause, drop back to resume"
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
			<button bind:this={playBtn} class="ps-tbtn ps-play ps-aqua {playPressed ? 'pressed' : ''}" onclick={onPlayClick} title="Play / pause" aria-label="Play or pause">
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
