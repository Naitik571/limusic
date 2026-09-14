<!--
   MiniPlayerPill — floating island mini player (mooziac-style Dynamic Island) that sits at
   the bottom-center of the screen, on top of every other UI. Near-black glass shell with a
   live waveform seekbar: peaks are decoded once per track by Rust (waveform.rs, cached in
   SQLite) and glow white behind the playhead. Click or drag-scrub the wave to seek; the
   thin fill shows until peaks land.
-->
<script module lang="ts">
	import * as apiMod from '$lib/api';

	// Peak cache, genuinely module-level: every pill mount (coverflow, stack, shell) shares
	// it, with an LRU cap and inflight dedup so rapid A→B→A skips never double-fetch.
	const PEAK_CAP = 50;
	const peakCache = new Map<string, number[]>();
	const peakInflight = new Map<string, Promise<number[]>>();
	function peakGet(id: string, bars: number): Promise<number[]> {
		const hit = peakCache.get(id);
		if (hit) {
			peakCache.delete(id);
			peakCache.set(id, hit);
			return Promise.resolve(hit);
		}
		const flying = peakInflight.get(id);
		if (flying) return flying;
		const p = apiMod
			.waveformPeaks(id, bars)
			.then((res) => {
				peakInflight.delete(id);
				if (!res.length) throw new Error('empty');
				if (peakCache.size >= PEAK_CAP) {
					const oldest = peakCache.keys().next();
					if (!oldest.done) peakCache.delete(oldest.value);
				}
				peakCache.set(id, res);
				return res;
			})
			.catch((e) => {
				peakInflight.delete(id);
				throw e;
			});
		peakInflight.set(id, p);
		return p;
	}
</script>

<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { onMount } from 'svelte';
	import { PlayIcon, PauseIcon, PreviousIcon, NextIcon, FavouriteIcon, ShuffleIcon, RepeatIcon, RepeatOne01Icon, ArrowUp01Icon } from '@hugeicons/core-free-icons';
	import { playback, dragVolume, commitVolume, toggleNowPlayingLike, cycleRepeat, sleepTimer, setSleepTimer, toast, type SleepTimerMode } from '$lib/player.svelte';
	import { thumb } from '$lib/thumb';
	import * as api from '$lib/api';

	let { onOpenNow, accent = null }: { onOpenNow?: () => void; accent?: string | null } = $props();

	const cur = $derived(playback.now);
	const paused = $derived(playback.paused);
	const pos = $derived(playback.position);
	const dur = $derived(playback.duration || 0);
	const pct = $derived(dur > 0 ? Math.min(100, (pos / dur) * 100) : 0);
	const liked = $derived(playback.liked ?? false);
	const shuffleOn = $derived(playback.queue.shuffle ?? false);
	const repeat = $derived(playback.queue.repeat ?? 'off');
	const upcoming = $derived(
		playback.queue.items
			.map((item, i) => ({ item, i }))
			.slice(playback.queue.currentIndex + 1, playback.queue.currentIndex + 7)
	);
	const sleepText = $derived(
		sleepTimer.mode === 'off'
			? null
			: sleepTimer.mode === 'end_of_song'
				? 'End of song'
				: `${Math.floor(sleepTimer.remaining / 60)}:${String(sleepTimer.remaining % 60).padStart(2, '0')}`
	);
	let expanded = $state(false);
	// Audit 25: focus trap + return for the up-next sheet.
	let expandBtn = $state<HTMLButtonElement>();
	let sheetEl = $state<HTMLDivElement>();
	$effect(() => {
		if (expanded) {
			// Focus the sheet when it opens so Escape + arrows work immediately.
			requestAnimationFrame(() => sheetEl?.focus());
		} else {
			// Return focus to the expand trigger when the sheet closes.
			if (sheetEl && document.activeElement instanceof Node && sheetEl.contains(document.activeElement)) {
				expandBtn?.focus();
			}
		}
	});
	function closeSheet() {
		expanded = false;
		expandBtn?.focus();
	}
	function playUpcoming(i: number) {
		expanded = false;
		api.playIndex(i).catch((err) => toast.error(String(err)));
	}

	let pill = $state<HTMLDivElement>();

	// Auto-shrink title (BlazePod-style): step the font down until the full title fits,
	// floor 9px, and only then let the ellipsis take the rest. Runs on track change and
	// on pill resize (the meta column width follows the island width).
	let titleEl = $state<HTMLSpanElement>();
	function fitTitle() {
		const el = titleEl;
		if (!el) return;
		el.style.fontSize = '';
		let size = 12;
		while (size > 9 && el.scrollWidth > el.clientWidth + 1) {
			size -= 0.5;
			el.style.fontSize = `${size}px`;
		}
	}
	$effect(() => {
		cur?.title;
		fitTitle();
	});
	onMount(() => {
		if (!pill) return;
		const ro = new ResizeObserver(() => fitTitle());
		ro.observe(pill);
		return () => ro.disconnect();
	});

	// Waveform peaks for the island seekbar (mooziac-style): decoded once per track by Rust,
	// then cached in SQLite. Shared module cache above, so remounts don't refetch; the plain
	// thin fill shows until peaks land (decode needs the audio bytes first).
	const WAVE_BARS = 96;
	let peaks = $state<number[] | null>(null);
	$effect(() => {
		const id = cur?.videoId;
		if (!id) {
			peaks = null;
			return;
		}
		peaks = null;
		let live = true;
		let retries = 0;
		let retryTimer: ReturnType<typeof setTimeout> | null = null;
		function load() {
			if (!id) return;
			peakGet(id, WAVE_BARS)
				.then((bars) => {
					if (live) peaks = bars;
				})
				.catch(() => {
					// Audit 28: retry peaks on revisit — one delayed retry while the
					// track is still current (decode may need the audio bytes first).
					if (live && retries < 2) {
						retries += 1;
						retryTimer = setTimeout(() => {
							if (live) load();
						}, 3000);
					}
				});
		}
		load();
		return () => {
			live = false;
			if (retryTimer) clearTimeout(retryTimer);
		};
	});

	function fmt(s: number): string {
		// Shared h:mm:ss contract (same as PlayerBar): hours appear only past 1h.
		if (!s || s < 0 || Number.isNaN(s)) return '0:00';
		const t = Math.floor(s);
		const h = Math.floor(t / 3600);
		const m = Math.floor((t % 3600) / 60);
		const sec = t % 60;
		const mm = h ? String(m).padStart(2, '0') : `${m}`;
		return `${h ? `${h}:` : ''}${mm}:${String(sec).padStart(2, '0')}`;
	}
	function seekRatio(e: MouseEvent): number | null {
		if (!dur) return null;
		const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
		if (!r.width) return null;
		return Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
	}
	function seek(e: MouseEvent) {
		// Audit 28: pointerdown already seeks (scrub path below); swallow the
		// post-scrub click so one gesture doesn't fire two seeks.
		if (justScrubbed) {
			justScrubbed = false;
			return;
		}
		const ratio = seekRatio(e);
		if (ratio !== null) api.seek(ratio * dur).catch((err) => toast.error(String(err)));
	}
	// Drag-scrub across the waveform: press seeks, moving with the button held keeps seeking.
	// Throttled live seeks (150ms) with an exact seek on release — unthrottled pointermove
	// floods IPC and stutters the audio.
	let lastScrubAt = 0;
	let justScrubbed = false;
	function scrubDown(e: PointerEvent) {
		const ratio = seekRatio(e);
		if (ratio === null) return;
		lastScrubAt = performance.now();
		justScrubbed = true;
		api.seek(ratio * dur).catch((err) => toast.error(String(err)));
	}
	function scrub(e: PointerEvent) {
		if (e.buttons !== 1) return;
		const ratio = seekRatio(e);
		if (ratio === null) return;
		const now = performance.now();
		if (now - lastScrubAt < 150) return;
		lastScrubAt = now;
		justScrubbed = true;
		api.seek(ratio * dur).catch((err) => toast.error(String(err)));
	}
	function scrubEnd(e: PointerEvent) {
		const ratio = seekRatio(e);
		if (ratio !== null) {
			justScrubbed = true;
			api.seek(ratio * dur).catch((err) => toast.error(String(err)));
		}
	}
	// Audit 26: sleep chip opens the preset list (15/30/60/end-of-song/off),
	// mirroring PlayerBar's sleep menu instead of acting as a bare off-switch.
	let sleepOpen = $state(false);
	function pickSleep(mode: SleepTimerMode, minutes = 30) {
		sleepOpen = false;
		setSleepTimer(mode, minutes);
	}
	function openNow() {
		onOpenNow?.();
	}
</script>

{#if cur}
	<div
		bind:this={pill}
		class="ps-mini-pill"
		role="group"
		aria-label="Mini player"
		style={accent ? `--ps-pill-accent: ${accent};` : ''}
	>
		<button class="ps-mini-pill-art" onclick={openNow} title="Open now playing" aria-label="Open now playing">
			{#if cur.thumbnail}
				<img decoding="async" src={thumb(cur.thumbnail, 64) ?? cur.thumbnail} alt="" />
			{:else}
				<div class="ps-mini-pill-art-fallback"></div>
			{/if}
		</button>
		<div class="ps-mini-pill-meta" onclick={openNow} role="button" tabindex="0" onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && openNow()}>
			<span class="ps-mini-pill-title" bind:this={titleEl}>{cur.title}</span>
			<span class="ps-mini-pill-artist">{cur.artists}</span>
		</div>
		{#if !paused}
			<span class="ps-live-dot" aria-hidden="true" title="Playing"></span>
		{/if}
		<button class="ps-mini-pill-btn" onclick={() => api.prevTrack().catch((err) => toast.error(String(err)))} aria-label="Previous">
			<HugeiconsIcon strokeWidth={2} icon={PreviousIcon} />
		</button>
		<button class="ps-mini-pill-btn ps-mini-pill-btn--play" onclick={() => api.togglePause().catch((err) => toast.error(String(err)))} aria-label={paused ? 'Play' : 'Pause'}>
			<HugeiconsIcon strokeWidth={2} icon={paused ? PlayIcon : PauseIcon} />
		</button>
		<button class="ps-mini-pill-btn" onclick={() => api.nextTrack().catch((err) => toast.error(String(err)))} aria-label="Next">
			<HugeiconsIcon strokeWidth={2} icon={NextIcon} />
		</button>
		<button
			class="ps-mini-pill-btn {liked ? 'is-liked' : ''}"
			onclick={() => toggleNowPlayingLike().catch(() => {})}
			aria-label={liked ? 'Remove from Liked Songs' : 'Save to Liked Songs'}
			title={liked ? 'Remove from Liked Songs' : 'Save to Liked Songs'}
		>
			<HugeiconsIcon strokeWidth={2} icon={FavouriteIcon} />
		</button>
		<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
		<!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
		<div
			class="ps-mini-pill-progress"
			onclick={seek}
			onpointerdown={scrubDown}
			onpointermove={scrub}
			onpointerup={scrubEnd}
			onkeydown={(e) => {
				if (e.key === 'ArrowLeft') { e.preventDefault(); api.seek(Math.max(0, pos - 5)).catch((err) => toast.error(String(err))); }
				if (e.key === 'ArrowRight') { e.preventDefault(); api.seek(Math.min(dur, pos + 5)).catch((err) => toast.error(String(err))); }
				if (e.key === 'Home') { e.preventDefault(); api.seek(0).catch((err) => toast.error(String(err))); }
				if (e.key === 'End') { e.preventDefault(); api.seek(dur).catch((err) => toast.error(String(err))); }
			}}
			role="slider"
			aria-label="Seek"
			aria-valuemin="0"
			aria-valuemax={Math.round(dur)}
			aria-valuenow={Math.round(pos)}
			tabindex="0"
		>
			{#if peaks}
				<!-- Waveform seekbar: played bars glow, the rest sit dim. Bar count is fixed so
				     layout never shifts when peaks land; heights come straight from the decoder. -->
				<div class="ps-mini-pill-wave" aria-hidden="true">
					{#each peaks as p, i (i)}
						{@const played = (i / peaks.length) * 100 <= pct}
						<span
							class="ps-mini-pill-bar {played ? 'on' : ''}"
							style="height: {Math.max(12, Math.round((p / 255) * 100))}%"
						></span>
					{/each}
				</div>
			{:else}
				<div class="ps-mini-pill-progress-track">
					<div class="ps-mini-pill-progress-fill" style="width: {pct}%"></div>
				</div>
			{/if}
			<div class="ps-mini-pill-progress-times">
				<span>{fmt(pos)}</span>
				<span>{fmt(dur)}</span>
			</div>
		</div>
		<div class="ps-mini-pill-volume" aria-label="Volume">
			<input
				type="range"
				min="0"
				max="100"
				value={playback.volume}
				oninput={(e) => dragVolume(Number(e.currentTarget.value))}
				onchange={(e) => commitVolume(Number(e.currentTarget.value))}
				aria-label="Volume"
			/>
		</div>
		{#if sleepText}
			<span class="ps-mini-pill-sleepwrap">
				<button
					class="ps-mini-pill-sleep"
					onclick={() => (sleepOpen = !sleepOpen)}
					title="Sleep timer — open presets"
					aria-label="Sleep timer on, activate to change"
					aria-expanded={sleepOpen}
				>
					{sleepText}
				</button>
				{#if sleepOpen}
					<button
						class="ps-mini-pill-sleep-scrim"
						onclick={() => (sleepOpen = false)}
						aria-label="Close sleep timer menu"
						tabindex="0"
						onkeydown={(e) => e.key === 'Escape' && (sleepOpen = false)}
					></button>
					<div
						class="ps-mini-pill-sleep-menu"
						role="menu"
						aria-label="Sleep timer presets"
						tabindex="-1"
						onkeydown={(e) => e.key === 'Escape' && (sleepOpen = false)}
					>
						<button role="menuitem" onclick={() => pickSleep('minutes', 15)}>15 minutes</button>
						<button role="menuitem" onclick={() => pickSleep('minutes', 30)}>30 minutes</button>
						<button role="menuitem" onclick={() => pickSleep('minutes', 60)}>60 minutes</button>
						<button role="menuitem" onclick={() => pickSleep('end_of_song')}>End of song</button>
						<button role="menuitem" onclick={() => pickSleep('off')}>Cancel timer</button>
					</div>
				{/if}
			</span>
		{:else}
			<span class="ps-mini-pill-sleepwrap">
				<button
					class="ps-mini-pill-sleep ps-mini-pill-sleep--off"
					onclick={() => (sleepOpen = !sleepOpen)}
					title="Sleep timer — open presets"
					aria-label="Sleep timer off, activate to set"
					aria-expanded={sleepOpen}
				>
					☾
				</button>
				{#if sleepOpen}
					<button
						class="ps-mini-pill-sleep-scrim"
						onclick={() => (sleepOpen = false)}
						aria-label="Close sleep timer menu"
						tabindex="0"
						onkeydown={(e) => e.key === 'Escape' && (sleepOpen = false)}
					></button>
					<div
						class="ps-mini-pill-sleep-menu"
						role="menu"
						aria-label="Sleep timer presets"
						tabindex="-1"
						onkeydown={(e) => e.key === 'Escape' && (sleepOpen = false)}
					>
						<button role="menuitem" onclick={() => pickSleep('minutes', 15)}>15 minutes</button>
						<button role="menuitem" onclick={() => pickSleep('minutes', 30)}>30 minutes</button>
						<button role="menuitem" onclick={() => pickSleep('minutes', 60)}>60 minutes</button>
						<button role="menuitem" onclick={() => pickSleep('end_of_song')}>End of song</button>
					</div>
				{/if}
			</span>
		{/if}
		<button
			bind:this={expandBtn}
			class="ps-mini-pill-btn ps-mini-pill-expand {expanded ? 'open' : ''}"
			onclick={() => (expanded = !expanded)}
			aria-label={expanded ? 'Collapse up next' : 'Expand up next'}
			aria-expanded={expanded}
			title="Up next"
		>
			<HugeiconsIcon strokeWidth={2} icon={ArrowUp01Icon} />
		</button>
	</div>
	{#if expanded}
		<button
			class="ps-mini-pill-scrim"
			onclick={() => closeSheet()}
			onkeydown={(e) => e.key === 'Escape' && closeSheet()}
			aria-label="Collapse up next"
		></button>
		<div
			bind:this={sheetEl}
			class="ps-mini-pill-sheet"
			role="dialog"
			aria-modal="false"
			aria-label="Up next"
			tabindex="-1"
			onkeydown={(e) => {
				if (e.key === 'Escape') {
					e.stopPropagation();
					closeSheet();
				}
			}}
		>
			<div class="ps-mini-pill-sheet-head">
				<span>Up next</span>
				<div class="ps-mini-pill-sheet-modes">
					<button
						class="ps-mini-pill-btn sm {shuffleOn ? 'is-on' : ''}"
						onclick={() => api.toggleShuffle().catch((err) => toast.error(String(err)))}
						aria-label="Toggle shuffle"
						title="Shuffle"
					>
						<HugeiconsIcon strokeWidth={2} icon={ShuffleIcon} />
					</button>
					<button
						class="ps-mini-pill-btn sm {repeat !== 'off' ? 'is-on' : ''}"
						onclick={() => cycleRepeat().catch((err) => toast.error(String(err)))}
						aria-label="Cycle repeat mode"
						title={repeat === 'one' ? 'Repeat one' : repeat === 'all' ? 'Repeat all' : 'Repeat off'}
					>
						<HugeiconsIcon strokeWidth={2} icon={repeat === 'one' ? RepeatOne01Icon : RepeatIcon} />
					</button>
				</div>
			</div>
			{#if upcoming.length}
				{#each upcoming as { item, i } (item.video_id + i)}
					<button class="ps-mini-pill-next" onclick={() => playUpcoming(i)}>
						<span class="ps-mini-pill-next-title">{item.title}</span>
						<span class="ps-mini-pill-next-artist">{item.artists}</span>
					</button>
				{/each}
			{:else}
				<p class="ps-mini-pill-next-empty">Nothing queued — the night ends here.</p>
			{/if}
		</div>
	{/if}
{/if}

<style>
	.ps-mini-pill {
		position: fixed;
		left: 50%;
		bottom: 18px;
		transform: translateX(-50%);
		z-index: 50;
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 8px 14px;
		border-radius: var(--r-full);
		/* Dynamic-island look: near-black glass, white-on-dark chrome. The pool tint comes
		   from the glow accents, not the shell, so it reads as hardware on any backdrop. */
		background: linear-gradient(180deg, rgba(12, 12, 14, 0.84) 0%, rgba(12, 12, 14, 0.68) 100%);
		backdrop-filter: blur(24px) saturate(1.8);
		-webkit-backdrop-filter: blur(24px) saturate(1.8);
		border: 1px solid rgba(255, 255, 255, 0.12);
		box-shadow: 0 18px 44px rgba(0, 0, 0, 0.5), 0 4px 12px rgba(0, 0, 0, 0.3), inset 0 1px 0 rgba(255, 255, 255, 0.08);
		color: #fff;
		min-width: 520px;
		max-width: calc(100vw - 36px);
	}
	.ps-mini-pill-art {
		all: unset;
		cursor: pointer;
		width: 44px;
		height: 44px;
		border-radius: 50%;
		overflow: hidden;
		border: 1.5px solid rgba(255, 255, 255, 0.5);
		flex: none;
		background: conic-gradient(from 210deg, #e8e8e8, #9fb6bc, #fff, #7fa6ae, #e8e8e8);
	}
	.ps-mini-pill-art img { width: 100%; height: 100%; object-fit: cover; display: block; }
	.ps-mini-pill-art-fallback { width: 100%; height: 100%; background: inherit; }
	.ps-mini-pill-meta {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
		max-width: 220px;
		padding: 0 4px;
		cursor: pointer;
	}
	.ps-mini-pill-title {
		font-size: var(--text-caption);
		font-weight: 700;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.ps-mini-pill-artist {
		font-size: 10px;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		opacity: 0.7;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.ps-mini-pill-btn {
		all: unset;
		cursor: pointer;
		width: 32px;
		height: 32px;
		border-radius: 50%;
		display: grid;
		place-items: center;
		color: #fff;
		opacity: 0.85;
		transition: opacity var(--dur-1), transform var(--dur-1);
	}
	.ps-mini-pill-btn:hover { opacity: 1; transform: scale(1.08); }
	.ps-mini-pill-btn:active:not(:disabled) { transform: scale(0.88); }
	.ps-mini-pill-btn svg { width: 14px; height: 14px; }
	.ps-mini-pill-btn--play {
		background: linear-gradient(180deg, #8fdef6, var(--accent) 55%, #2e9ecb);
		color: #111;
		opacity: 1;
		box-shadow: 0 4px 12px rgba(14, 110, 140, 0.5);
	}
	.ps-mini-pill-btn--play svg { width: 16px; height: 16px; }
	.ps-mini-pill-btn.is-liked { color: #ff5d7a; opacity: 1; }
	.ps-mini-pill-btn.is-liked svg { fill: currentColor; }
	.ps-mini-pill-btn.sm { width: 26px; height: 26px; }
	.ps-mini-pill-btn.sm svg { width: 13px; height: 13px; }
	.ps-mini-pill-btn.is-on { color: #8ce1f0; opacity: 1; }
	.ps-mini-pill-expand svg { transition: transform var(--dur-2); }
	.ps-mini-pill-expand.open svg { transform: rotate(180deg); }
	.ps-mini-pill-sleep {
		all: unset;
		cursor: pointer;
		flex: none;
		font-size: 9px;
		letter-spacing: 0.08em;
		font-variant-numeric: tabular-nums;
		color: #ffd88a;
		background: rgba(255, 216, 138, 0.12);
		border: 1px solid rgba(255, 216, 138, 0.3);
		border-radius: var(--r-full);
		padding: 3px 8px;
		white-space: nowrap;
	}
	.ps-mini-pill-sleep:hover { background: rgba(255, 216, 138, 0.22); }
	/* Audit 26: preset menu anchored to the chip. */
	.ps-mini-pill-sleepwrap { position: relative; display: inline-flex; flex: none; }
	.ps-mini-pill-sleep--off { color: rgba(255, 255, 255, 0.65); background: rgba(255, 255, 255, 0.08); border-color: rgba(255, 255, 255, 0.16); }
	.ps-mini-pill-sleep-scrim {
		all: unset;
		position: fixed;
		inset: 0;
		z-index: 51;
		cursor: default;
	}
	.ps-mini-pill-sleep-menu {
		position: absolute;
		right: 0;
		bottom: calc(100% + 8px);
		z-index: 52;
		display: flex;
		flex-direction: column;
		min-width: 150px;
		padding: 4px;
		border-radius: var(--r-xl);
		background: linear-gradient(180deg, rgba(12, 12, 14, 0.95) 0%, rgba(12, 12, 14, 0.88) 100%);
		border: 1px solid rgba(255, 255, 255, 0.14);
		box-shadow: 0 18px 44px rgba(0, 0, 0, 0.5);
	}
	.ps-mini-pill-sleep-menu button {
		all: unset;
		cursor: pointer;
		padding: 7px 10px;
		border-radius: var(--r-lg);
		font-size: 12px;
		color: #fff;
		white-space: nowrap;
	}
	.ps-mini-pill-sleep-menu button:hover,
	.ps-mini-pill-sleep-menu button:focus-visible {
		background: rgba(255, 255, 255, 0.1);
		outline: none;
	}
	/* Live pulse dot: now-playing heartbeat next to the transport. Follows the album
	   accent when one is sampled (--ps-pill-accent), signature red otherwise. */
	.ps-live-dot {
		width: 7px;
		height: 7px;
		flex: none;
		border-radius: 50%;
		background: var(--ps-pill-accent, #ff5d5d);
		box-shadow: 0 0 8px var(--ps-pill-accent, rgba(255, 93, 93, 0.9));
		animation: ps-live-pulse 1.6s ease-in-out infinite;
	}
	@keyframes ps-live-pulse {
		0%,
		100% {
			opacity: 1;
			transform: scale(1);
		}
		50% {
			opacity: 0.45;
			transform: scale(0.8);
		}
	}
	/* Up-next sheet: same island glass, floating above the pill. */
	.ps-mini-pill-scrim {
		all: unset;
		position: fixed;
		inset: 0;
		z-index: 49;
		cursor: default;
	}
	.ps-mini-pill-sheet {
		position: fixed;
		left: 50%;
		transform: translateX(-50%);
		bottom: 76px;
		z-index: 50;
		width: 340px;
		max-width: calc(100vw - 36px);
		max-height: 320px;
		overflow-y: auto;
		border-radius: var(--r-2xl);
		padding: 10px;
		background: linear-gradient(180deg, rgba(12, 12, 14, 0.92) 0%, rgba(12, 12, 14, 0.8) 100%);
		backdrop-filter: blur(24px) saturate(1.8);
		-webkit-backdrop-filter: blur(24px) saturate(1.8);
		border: 1px solid rgba(255, 255, 255, 0.12);
		box-shadow: 0 18px 44px rgba(0, 0, 0, 0.5);
		color: #fff;
	}
	.ps-mini-pill-sheet-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 2px 6px 8px;
		font-size: 10px;
		font-weight: 700;
		letter-spacing: 0.12em;
		text-transform: uppercase;
		opacity: 0.75;
	}
	.ps-mini-pill-sheet-modes { display: flex; gap: 2px; }
	.ps-mini-pill-next {
		all: unset;
		display: flex;
		flex-direction: column;
		gap: 1px;
		width: 100%;
		box-sizing: border-box;
		padding: 7px 10px;
		border-radius: var(--r-xl);
		cursor: pointer;
		text-align: left;
	}
	.ps-mini-pill-next:hover { background: rgba(255, 255, 255, 0.08); }
	.ps-mini-pill-next-title {
		font-size: 12px;
		font-weight: 600;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.ps-mini-pill-next-artist {
		font-size: 10px;
		opacity: 0.6;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.ps-mini-pill-next-empty {
		padding: 10px;
		font-size: 12px;
		opacity: 0.6;
		text-align: center;
	}
	.ps-mini-pill-progress {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 100px;
		flex: 1;
		cursor: pointer;
	}
	.ps-mini-pill-progress-track {
		height: 4px;
		border-radius: 4px;
		background: rgba(255, 255, 255, 0.18);
		overflow: hidden;
	}
	.ps-mini-pill-progress-fill {
		height: 100%;
		background: linear-gradient(90deg, rgba(255, 255, 255, 0.95), var(--ps-pill-accent, rgba(140, 225, 240, 0.95)));
		transition: width 0.3s linear;
	}
	/* Waveform seekbar: fixed bar count, decoder-driven heights. Played bars glow white,
	   the rest sit dim; the whole strip is the slider hit area (click + drag-scrub). */
	.ps-mini-pill-wave {
		display: flex;
		align-items: center;
		gap: 1px;
		height: 28px;
		cursor: pointer;
	}
	.ps-mini-pill-bar {
		flex: 1 1 0;
		min-width: 1px;
		border-radius: 1px;
		background: rgba(255, 255, 255, 0.22);
		transition: background var(--dur-2);
	}
	.ps-mini-pill-bar.on {
		background: linear-gradient(180deg, #fff, var(--ps-pill-accent, rgba(140, 225, 240, 0.9)));
		box-shadow: 0 0 6px var(--ps-pill-accent, rgba(140, 225, 240, 0.45));
	}
	.ps-mini-pill-progress-times {
		display: flex;
		justify-content: space-between;
		font-size: 9px;
		letter-spacing: 0.08em;
		opacity: 0.65;
		font-variant-numeric: tabular-nums;
	}
	.ps-mini-pill-volume {
		width: 60px;
		flex: none;
	}
	.ps-mini-pill-volume input {
		width: 100%;
		accent-color: var(--accent);
	}
	@media (max-width: 720px) {
		.ps-mini-pill {
			min-width: 0;
			padding: 6px 8px;
			gap: 6px;
		}
		.ps-mini-pill-meta { display: none; }
		.ps-mini-pill-volume { display: none; }
		.ps-mini-pill-progress { min-width: 80px; }
	}
</style>
