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
		playback, commitVolume, dragVolume, toggleMute,
		cycleRepeat, toggleNowPlayingLike, toast
	} from '$lib/player.svelte';
	import { thumb } from '$lib/thumb';
	import Vinyl from './Vinyl.svelte';

	let {
		onOpenLibrary,
		discSkin = 'photo'
	}: {
		onOpenLibrary: () => void;
		/** Disc treatment for the main deck (pool settings → deck skin). */
		discSkin?: 'photo' | 'noir' | 'crimson' | 'auto';
	} = $props();

	const cur = $derived(playback.now);
	const paused = $derived(playback.paused);
	const q = $derived(playback.queue);
	const nextItem = $derived(q.items[q.currentIndex + 1] ?? null);
	const upcoming = $derived(q.items.slice(q.currentIndex + 1, q.currentIndex + 7));
	const pos = $derived(Math.min(playback.position, playback.duration || playback.position));
	const dur = $derived(playback.duration || 0);

	// Shared h:mm:ss formatter (same contract as PlayerBar): hours appear only past 1h.
	function fmt(s: number): string {
		if (!s || s < 0 || Number.isNaN(s)) return '0:00';
		const t = Math.floor(s);
		const h = Math.floor(t / 3600);
		const m = Math.floor((t % 3600) / 60);
		const sec = t % 60;
		const mm = h ? String(m).padStart(2, '0') : `${m}`;
		return `${h ? `${h}:` : ''}${mm}:${String(sec).padStart(2, '0')}`;
	}
	// Audit 22: seekDrag/shownPosition ported from PlayerBar — while dragging, hold a
	// local value so incoming position ticks can't yank the thumb back under the
	// pointer. Live seeks are throttled (~150ms); the exact value lands on release.
	let seekDrag = $state<number | null>(null);
	const shownPosition = $derived(seekDrag ?? pos);
	let lastSliderSeekAt = 0;
	function onSeekInput(e: Event) {
		const v = Number((e.currentTarget as HTMLInputElement).value);
		seekDrag = v;
		const now = performance.now();
		if (now - lastSliderSeekAt > 150) {
			lastSliderSeekAt = now;
			api.seek(v).catch(() => {});
		}
	}
	function onSeekCommit(e: Event) {
		const v = Number((e.currentTarget as HTMLInputElement).value);
		playback.position = v;
		seekDrag = null;
		api.seek(v).catch(() => {});
	}
	function playIndex(i: number) {
		api.playIndex(i).catch((e) => toast.error(String(e)));
	}

	// drag-to-eject: pull the disc past the threshold and release -> music pauses. Drag back
	// to centre and release -> the disc snaps home. One threshold (60px) for both the visual
	// "ejecting" class and the eject action, so there's no dead zone where it looks like it's
	// ejecting but does nothing.
	const EJECT_PX = 60;
	const SNAP_PX = 90; // generous drop-back radius for resume; tighter snap anim is separate
	let dragX = $state(0);
	let dragY = $state(0);
	let isDragging = $state(false);
	let dragStartX = 0;
	let dragStartY = 0;
	let dragStartState: { paused: boolean } | null = null;
	// Spin-to-seek gesture state (see onDiscPointerDown/Move/Up).
	let seekCX = 0;
	let seekCY = 0;
	let seekLastAngle = 0;
	let seekSwept = 0;
	let seekMode = $state(false);
	let seekStartPos = 0;
	let seekTarget = $state(0);
	let lastSeekAt = 0;
	// Local "drag intent" — true while the disc is being dragged past the eject
	// threshold. Distinct from the visual "ejected" class so we can show a strong
	// visual only during the drag itself.
	let dragPastEject = $state(false);
	// True briefly after a successful eject so we can fire the toast once.
	let ejectJustHappened = $state(false);
	// True for ~280ms after the disc snaps home (used for the "click into place"
	// animation that scales the disc up briefly).
	let justSnapped = $state(false);
	let snapTimer: ReturnType<typeof setTimeout> | null = null;

	// Audit 21: the eject gesture owns its own state, separate from `paused`. A disc
	// is "ejected" only when the user physically dragged it off the platter (or it
	// started that way) — an external pause (transport button, sleep timer, error)
	// leaves ejectedGesture false, so a later nudge can never resume what the user
	// didn't eject. Tonearm: -15° resting/paused, -8° lifted-ejected.
	let ejectedGesture = $state(false);
	// Visual eject: mid-drag past the threshold, or resting in the ejected pose.
	const ejected = $derived(!!cur && (ejectedGesture || dragPastEject));

	function onDiscPointerDown(e: PointerEvent) {
		if (!cur) return;
		isDragging = true;
		dragStartX = e.clientX;
		dragStartY = e.clientY;
		dragStartState = { paused };
		dragPastEject = false;
		// Spin-to-seek state: the disc center anchors angle math; the gesture becomes a
		// seek (not an eject) once the pointer sweeps 12° around it before translating
		// past the eject threshold. Whichever wins first owns the gesture.
		const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
		seekCX = r.left + r.width / 2;
		seekCY = r.top + r.height / 2;
		seekLastAngle = Math.atan2(e.clientY - seekCY, e.clientX - seekCX);
		seekSwept = 0;
		seekMode = false;
		seekStartPos = pos;
		seekTarget = pos;
		(e.target as HTMLElement).setPointerCapture(e.pointerId);
	}
	function onDiscPointerMove(e: PointerEvent) {
		if (!isDragging) return;
		if (!seekMode && dur > 0) {
			// Circular sweep test runs until the eject threshold fires first.
			const a = Math.atan2(e.clientY - seekCY, e.clientX - seekCX);
			let d = ((a - seekLastAngle) * 180) / Math.PI;
			if (d > 180) d -= 360;
			if (d < -180) d += 360;
			seekLastAngle = a;
			seekSwept += d;
			const dx0 = e.clientX - dragStartX;
			const dy0 = e.clientY - dragStartY;
			if (Math.abs(seekSwept) >= 12 && Math.sqrt(dx0 * dx0 + dy0 * dy0) <= EJECT_PX) {
				seekMode = true;
				dragPastEject = false;
			}
		}
		if (seekMode) {
			// Rotation scrubs: one full turn = the whole track. Throttled live seeks
			// while turning, final exact seek on release.
			seekTarget = Math.min(dur, Math.max(0, seekStartPos + (seekSwept / 360) * dur));
			const now = performance.now();
			if (now - lastSeekAt > 150) {
				lastSeekAt = now;
				api.seek(seekTarget).catch(() => {});
			}
			return;
		}
		dragX = e.clientX - dragStartX;
		dragY = e.clientY - dragStartY;
		const dist = Math.sqrt(dragX * dragX + dragY * dragY);
		const past = dist > EJECT_PX;
		if (past !== dragPastEject) dragPastEject = past;
	}
	async function onDiscPointerUp() {
		if (!isDragging) return;
		if (seekMode) {
			// Land exactly where the dial points, then reset the gesture.
			seekMode = false;
			isDragging = false;
			api.seek(seekTarget).catch(() => {});
			return;
		}
		const distance = Math.sqrt(dragX * dragX + dragY * dragY);
		const withinSnapRadius = distance < SNAP_PX;
		isDragging = false;

		if (distance > EJECT_PX && dragStartState && !dragStartState.paused) {
			// user dragged the disc off the platter while it was playing
			await api.togglePause().catch(() => {});
			ejectedGesture = true;
			toast.info('Disc removed — drop it back to resume');
			ejectJustHappened = true;
		} else if (ejectedGesture && withinSnapRadius) {
			// user dragged an ejected disc back to center -> resume. Gated on the
			// gesture flag (not `paused`), so nudging a disc that was paused
			// externally never resumes it.
			await api.togglePause().catch(() => {});
			ejectedGesture = false;
			triggerSnap();
		} else if (!ejectedGesture) {
			// dragged but never ejected (or paused externally): leave pause state alone.
		}
		// snap back to spindle visually
		dragX = 0;
		dragY = 0;
		dragPastEject = false;
	}
	// Cancellation is not a release (OS interrupt, Alt-Tab, touch stolen): reset the gesture
	// with zero side effects — no pause, no resume, no seek commit, no toast.
	function onDiscPointerCancel() {
		if (!isDragging) return;
		isDragging = false;
		seekMode = false;
		dragX = 0;
		dragY = 0;
		dragPastEject = false;
	}
	function triggerSnap() {
		if (snapTimer) clearTimeout(snapTimer);
		justSnapped = true;
		snapTimer = setTimeout(() => {
			justSnapped = false;
			snapTimer = null;
		}, 280);
	}
	function onDiscDoubleClick() {
		// explicit "drop the disc back" gesture, even without a drag — useful for the
		// small quick tap that doesn't register as a drag. Gesture-gated: an
		// externally-paused disc ignores it (audit 21).
		if (ejectedGesture) {
			api.togglePause().catch(() => {});
			ejectedGesture = false;
			triggerSnap();
		}
	}

	// A new track drops a fresh disc onto the platter — clear any stale eject pose.
	$effect(() => {
		cur?.videoId;
		ejectedGesture = false;
	});

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
	</script>

<div class="ps-np">
	<!-- One vertical column: deck (turntable + disc sleeve + disc) -> caption -> transport.
	     Z-stack inside the deck (low to high): plinth (z:0) -> sleeve (z:1) -> disc (z:2) ->
	     tonearm (z:3, above disc so it can hover over the record). The deck itself is the
	     only "hero" element; caption and transport are clearly below it. -->

	<div class="ps-np-deck">
		<div class="ps-turntable">
			<div class="ps-plinth">
				<div class="ps-platter" class:spin={platterSpinning}></div>
				<div class="ps-speed-badge">33⅓ RPM</div>
				<div class="ps-power-led"></div>
			</div>
			<div class="ps-tonearm" style="transform: rotate({tonearmAngle()}deg);">
				<div class="ps-arm-pivot"></div>
				<div class="ps-arm-shaft">
					<div class="ps-arm-head"></div>
				</div>
			</div>
		</div>

		<!-- Disc sleeve + disc are inside the same .ps-deck-unit so the disc slides
		     out of the sleeve as the user drags. Sleeve z:1, disc z:2, NOW PLAYING sticker
		     z:3, UP NEXT hint z:3. Tonearm lives in the turntable wrapper so it can
		     hover above the record. -->
		<div class="ps-deck">
			{#if nextItem}
				<div class="ps-deck-unit ps-deck-unit--next" aria-hidden="true">
					<div class="ps-sleeve"><div class="mouth"></div></div>
					<Vinyl src={thumb(nextItem.thumbnail ?? null, 540) ?? ''} playing={false} style="width:100%" title="Up next" />
					<div class="ps-eject-hint">UP NEXT</div>
				</div>
			{/if}
			<!-- The deck-unit always renders; the inner disc-wrap holds the disc.
			     `key={cur?.videoId ?? 'none'}` re-mounts the disc on every track change
			     so we never get a frame where the previous track's artwork bleeds
			     through (Vinyl caches its own image internally, and remounting is
			     the simplest way to ensure the swap is clean). -->
			<div
				class="ps-deck-unit"
				class:ejecting={dragPastEject}
				class:snapped={justSnapped}
				class:ejected
				role="application"
				aria-label="Vinyl disc — drag off to pause, drop back to resume, or spin in a circle to seek"
				onpointerdown={onDiscPointerDown}
				onpointermove={onDiscPointerMove}
				onpointerup={onDiscPointerUp}
				onpointercancel={onDiscPointerCancel}
				ondblclick={onDiscDoubleClick}
			>
				<div class="ps-sleeve"><div class="mouth"></div></div>
				<div class="ps-deck-disc-wrap" style="
					transform: translate({dragX}px, {dragY}px);
					transition: {isDragging ? 'none' : 'transform .5s var(--ease-spring)'};
				">
					{#key cur?.videoId ?? 'none'}
						<Vinyl
							src={thumb(cur?.thumbnail ?? null, 720) ?? ''}
							playing={!paused && !!cur && !isDragging}
							style="width:100%"
							flightTarget
							title="Drag to eject · Spin in a circle to seek · Double-click to drop back"
							skin={discSkin}
						/>
					{/key}
				</div>
				<svg class="ps-sticker" style="right:-4%;top:-7%" width="56" height="56" viewBox="0 0 58 58">
					<circle cx="29" cy="29" r="26" fill="#fff" stroke="#111" stroke-width="3" />
					<text x="29" y="33" text-anchor="middle" font-family="monospace" font-size="7" font-weight="bold" letter-spacing="1.5" fill="#111">NOW PLAYING</text>
				</svg>
				{#if seekMode}
					<div class="ps-seek-dial" aria-hidden="true">
						<span>{fmt(seekTarget)}</span>
					</div>
				{/if}
			</div>
		</div>
	</div>

	<!-- caption: clear size hierarchy, sits below the deck, no z-conflict with discs -->
	<div class="ps-track-caption">
		<span class="np-label">{cur ? 'NOW PLAYING' : 'NO TRACK LOADED'}</span>
		<span class="np-title">{cur ? cur.title : 'Drop a disc onto the turntable'}</span>
		<span class="np-artist">{cur ? cur.artists : ''}</span>
	</div>

	<!-- seek + transport, all in one clean column -->
	<div class="ps-bottom">
		<div class="ps-seek-row">
			<span>{fmt(shownPosition)}</span>
			<input class="ps-seek" type="range" min="0" max={Math.max(0.1, dur)} step="0.1" value={shownPosition} oninput={onSeekInput} onchange={onSeekCommit} aria-label="Seek" />
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
				<button class="ps-tbtn" onclick={() => toggleMute()} title="Mute" aria-label="Mute">
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
		<button onclick={onOpenLibrary} class="cursor-pointer hover:text-white">Library</button>
		<span class="ps-hint-sep">·</span>
		<button onclick={() => window.dispatchEvent(new CustomEvent('ps:open-lyrics'))} class="cursor-pointer hover:text-white">Lyrics</button>
	</div>
</div>
