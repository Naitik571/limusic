<script module lang="ts">
	import * as api from '$lib/api';
	import { browser } from '$app/environment';
</script>

<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { Attachment01Icon } from '@hugeicons/core-free-icons';
	import { open as pickFile } from '@tauri-apps/plugin-dialog';
	import { playback, toast } from '$lib/player.svelte';
	import { appearance, applyLyricsFont } from '$lib/theme.svelte';

	// `expanded` only sizes the type and centres the column. The owner of the extra room (the side
	// panel, or the now-playing view) decides how much there is. Toggling it must not remount this
	// component, or the lyrics refetch and the scroll position is lost.
	// `compact` is the mini-player: a ~220px column with no room for the source footer or a
	// scrollbar. It only shrinks the type and chrome; the sync/auto-scroll logic is identical.
	// `sing` is the full-view karaoke takeover: giant active line, roomier spacing, no footer.
	let {
		expanded = false,
		compact = false,
		sing = false
	}: { expanded?: boolean; compact?: boolean; sing?: boolean } = $props();

	/** "3:21" / "1:02:03" → seconds. */
	function durationSecs(d?: string): number | undefined {
		if (!d) return undefined;
		const parts = d.split(':').map(Number);
		if (!parts.length || parts.some(Number.isNaN)) return undefined;
		return parts.reduce((a, b) => a * 60 + b, 0);
	}

	let lyrics = $state<api.Lyrics | null>(null);
	let loading = $state(true);
	let scroller: HTMLElement | undefined = $state();

	/** Shift every cue by `delta` ms (the persisted per-song offset), clamped at 0. */
	function shiftCues(l: api.Lyrics, delta: number) {
		if (!delta) return;
		for (const line of l.lines) {
			if (line.time_ms !== undefined) line.time_ms = Math.max(0, line.time_ms + delta);
			if (line.end_time_ms !== undefined) line.end_time_ms = Math.max(0, line.end_time_ms + delta);
			if (line.words)
				for (const w of line.words) {
					w.start_ms = Math.max(0, w.start_ms + delta);
					w.end_ms = Math.max(0, w.end_ms + delta);
				}
		}
	}

	// videoId of the fetch whose result is (or will be) shown — guards stale responses.
	let requested = '';
	// Bumped after attaching/removing custom lyrics so the effect below refetches.
	let reloadKey = 0;
	// Romaji view (kana→romaji via the `romanize_lyrics` command): one round-trip for the
	// whole song — every word and line text joined by \n (romanization never touches
	// newlines), mapped back positionally. Timings and the karaoke sweep are untouched.
	let romanOn = $state(false);
	let romanSeg = $state<string[] | null>(null);
	let romanFor = '';
	let romanBusy = $state(false);
	function segList(l: api.Lyrics): { line: number; word: number }[] {
		const out: { line: number; word: number }[] = [];
		l.lines.forEach((ln, i) => {
			if (ln.words?.length) ln.words.forEach((_, w) => out.push({ line: i, word: w }));
			else out.push({ line: i, word: -1 });
		});
		return out;
	}
	const hasKana = $derived(
		!!lyrics && lyrics.lines.some((l) => /[\u3040-\u30ff]/.test(l.text))
	);
	async function toggleRoman() {
		if (!lyrics) return;
		if (romanOn) {
			romanOn = false;
			return;
		}
		if (romanFor === requested && romanSeg) {
			romanOn = true;
			return;
		}
		romanBusy = true;
		const id = requested;
		try {
			const l = lyrics;
			const segs = segList(l);
			const src = segs
				.map((s) => (s.word >= 0 ? (l.lines[s.line].words?.[s.word].text ?? '') : l.lines[s.line].text))
				.join('\n');
			const out = await api.romanizeLyrics(src);
			const parts = out.split('\n');
			if (requested !== id || parts.length !== segs.length) return; // stale or reshaped
			romanSeg = parts;
			romanFor = id;
			romanOn = true;
		} catch {
			toast.error('Romanization failed');
		} finally {
			romanBusy = false;
		}
	}
	/** Display text for a line or word segment (romaji when toggled, original otherwise). */
	function segText(line: number, word: number, fallback: string): string {
		if (!romanOn || !romanSeg) return fallback;
		const l = lyrics;
		if (!l) return fallback;
		const segs = segList(l);
		const idx = segs.findIndex((s) => s.line === line && s.word === word);
		return idx >= 0 ? (romanSeg[idx] ?? fallback) : fallback;
	}

	$effect(() => {
		reloadKey;
		const now = playback.now;
		if (!now) {
			requested = '';
			lyrics = null;
			loading = false;
			romanOn = false;
			romanSeg = null;
			romanFor = '';
			return;
		}
		if (now.videoId === requested) return;
		const id = (requested = now.videoId);
		loading = true;
		lyrics = null;
		// Album isn't in now-playing, but the queue item usually has it — better LRCLIB matching.
		const album = playback.queue.items[playback.queue.currentIndex]?.album;
		api.getLyrics({
			videoId: id,
			title: now.title,
			artists: now.artists,
			album: album ?? undefined,
			// The track's own length — NOT playback.duration, which still holds the previous
			// track's value for a moment after a track change.
			duration: durationSecs(now.duration)
		})
			.then((l) => {
				if (requested !== id) return;
				lyrics = l;
				loading = false;
				hasScrolled = false; // first positioning on a new track is an instant jump
				romanOn = false; // new song, new script — romaji starts off
				romanSeg = null;
				romanFor = '';
				// Kodama per-song offset: apply the persisted shift to this song's cues on load.
				api.getLyricOffset(id)
					.then((o) => {
						if (requested !== id || !lyrics || !o) return;
						shiftCues(lyrics, o);
						lyrics = { ...lyrics };
					})
					.catch(() => {});
			})
			.catch(() => {
				if (requested !== id) return;
				loading = false;
			});
	});

	// Last synced line whose cue has passed. Binary search over the timed-line index
	// (built once per song): the old linear scan re-walked from the top on every tick, O(n)
	// per frame on 200-line lyrics; this is O(log n) for the same answer. Untimed lines
	// stay out of the index, so they can never read as active — same as before.
	const timedIdx = $derived(
		!lyrics?.synced
			? []
			: lyrics.lines
					.map((l, i) => (l.time_ms === undefined ? -1 : i))
					.filter((i) => i >= 0)
	);
	const activeIndex = $derived.by(() => {
		if (!timedIdx.length || !lyrics) return -1;
		const currentMs = posMs;
		const lines = lyrics.lines;
		let lo = 0;
		let hi = timedIdx.length - 1;
		let i = -1;
		while (lo <= hi) {
			const mid = (lo + hi) >> 1;
			if ((lines[timedIdx[mid]].time_ms as number) <= currentMs) {
				i = timedIdx[mid];
				lo = mid + 1;
			} else {
				hi = mid - 1;
			}
		}
		return i;
	});

	// Auto-scroll pauses while the user is scrolling (wheel/touch/scrollbar), resumes after 3s.
	// Tracked via input events, not `scroll`, so our own smooth scrolls don't trip it.
	let userScrollUntil = 0;
	let hasScrolled = false;
	function onUserScroll() {
		userScrollUntil = Date.now() + 3000;
		cancelScrollTween();
	}
	let wasMode: string | undefined;

	// rAF-driven scroll tween. `scrollIntoView({ behavior: 'smooth' })` delegates to the browser,
	// whose curve and duration are unsteerable and visibly different per platform (WebView2 glides,
	// WebKitGTK snaps); a fixed-duration easeInOutCubic reads the same everywhere and can be
	// cancelled mid-flight by a new line or a user scroll, so lines never queue up animations.
	let scrollTweenId: number | undefined;
	function cancelScrollTween() {
		if (scrollTweenId !== undefined) {
			cancelAnimationFrame(scrollTweenId);
			scrollTweenId = undefined;
		}
	}
	onDestroy(cancelScrollTween);
	function glideTo(target: number, ms: number) {
		const scrollerEl = scroller;
		if (!scrollerEl) return;
		cancelScrollTween();
		const from = scrollerEl.scrollTop;
		const delta = target - from;
		if (Math.abs(delta) < 0.5) return; // already there — don't fight sub-pixel jitter
		if (ms <= 0) {
			scrollerEl.scrollTop = target;
			return;
		}
		const t0 = performance.now();
		// easeInOutCubic: gentle start (the previous line just released focus), gentle settle
		// (the next line is about to take it). Linear feels mechanical at 60fps; this reads calm.
		const ease = (t: number) => (t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2);
		const tick = (now: number) => {
			const p = Math.min(1, (now - t0) / ms);
			scrollerEl.scrollTop = from + delta * ease(p);
			scrollTweenId = p < 1 ? requestAnimationFrame(tick) : undefined;
		};
		scrollTweenId = requestAnimationFrame(tick);
	}

	$effect(() => {
		const i = activeIndex;
		// Re-centre after the layout width/font changes, and jump rather than glide across it.
		// (Also fires on the first run, where both values are already at their defaults.)
		const mode = `${expanded ? 'e' : ''}${sing ? 's' : ''}`;
		if (mode !== wasMode) {
			wasMode = mode;
			hasScrolled = false;
			userScrollUntil = 0;
		}
		if (i < 0 || !scroller || Date.now() < userScrollUntil) return;
		const line = scroller.querySelector(`[data-line="${i}"]`);
		if (!line) return;
		// Centre the active line in the scroller's viewport (matches block:'center').
		const target =
			line.getBoundingClientRect().top -
			scroller.getBoundingClientRect().top -
			scroller.clientHeight / 2 +
			line.getBoundingClientRect().height / 2 +
			scroller.scrollTop;
		if (!hasScrolled) {
			// Opening mid-song jumps straight to the line.
			glideTo(target, 0);
		} else if (Math.abs(target - scroller.scrollTop) > scroller.clientHeight) {
			// A seek landed far away: jump instead of sweeping past every line in between.
			glideTo(target, 0);
		} else {
			// Duration scales slightly with distance so short hops feel snappy and long ones
			// never rush — clamped to 320–640ms, the range that reads as "glide" not "slide".
			const dist = Math.abs(target - scroller.scrollTop);
			glideTo(target, Math.max(320, Math.min(640, 320 + dist)));
		}
		hasScrolled = true;
	});

	function seekTo(line: api.LyricLine) {
		if (line.time_ms === undefined) return;
		const secs = line.time_ms / 1000;
		playback.position = secs; // optimistic — the mpv tick confirms
		userScrollUntil = 0; // jump the view along with the seek
		api.seek(secs);
	}

	// Import a .lrc/.txt file for a track no provider covers. Only offered in the empty
	// state (see below) — attached lyrics outrank every provider on the next fetch.
	async function attachLyricsFile() {
		const now = playback.now;
		if (!now) return;
		let picked: string | string[] | null = null;
		try {
			picked = await pickFile({
				multiple: false,
				title: `Lyrics for ${now.title}`,
				filters: [{ name: 'Lyrics', extensions: ['lrc', 'txt'] }]
			});
		} catch (e) {
			toast.error(String(e));
			return;
		}
		const path = Array.isArray(picked) ? picked[0] : picked;
		if (!path) return;
		try {
			const text = await api.readLyricsFile(path);
			await api.setCustomLyrics(now.videoId, text);
			toast.success('Lyrics attached to this song');
			reloadKey++; // refetch through the normal path
		} catch (e) {
			toast.error(String(e));
		}
	}

	async function removeAttachedLyrics() {
		const now = playback.now;
		if (!now) return;
		try {
			await api.deleteCustomLyrics(now.videoId);
			toast.success('Attached lyrics removed');
			reloadKey++;
		} catch (e) {
			toast.error(String(e));
		}
	}

	// mpv's position arrives ~4x a second. Run a local clock forward from each one so the karaoke
	// sweep moves every frame instead of stepping four times a second.
	let interpolatedPosSecs = $state(playback.position);

	$effect(() => {
		const pos = playback.position;
		if (playback.paused) {
			interpolatedPosSecs = pos;
			return;
		}
		// Rebase on every run. Rebasing only when the value moved kept the base timestamp from
		// before a pause, so resuming after N seconds paused ran the clock N seconds fast until
		// the next tick corrected it.
		const base = pos;
		const baseAt = performance.now();
		interpolatedPosSecs = pos;
		let frameId = requestAnimationFrame(function tick() {
			interpolatedPosSecs = base + (performance.now() - baseAt) / 1000;
			frameId = requestAnimationFrame(tick);
		});
		return () => cancelAnimationFrame(frameId);
	});

	const posMs = $derived(interpolatedPosSecs * 1000);

	function getWordProgress(word: api.LyricWord, currentMs: number): number {
		if (currentMs <= word.start_ms) return 0;
		if (currentMs >= word.end_ms) return 1;
		const dur = word.end_ms - word.start_ms;
		if (dur <= 0) return 1;
		return (currentMs - word.start_ms) / dur;
	}

	// The Aurora headline ramp from .text-gradient — the exact colours line-mode karaoke shows,
	// sampled at t so the word sweep and the line gradient read as one continuous system.
	function sung(t: number): string {
		const cs = getComputedStyle(document.documentElement);
		const primary = parseOkLCH(cs.getPropertyValue('--primary').trim());
		const stops: [number, number, number][] = [primary, [0.68, 0.19, 285], [0.65, 0.17, 335]];
		const x = Math.min(1, Math.max(0, t)) * (stops.length - 1);
		const i = Math.min(stops.length - 2, Math.floor(x));
		const f = x - i;
		const a = stops[i];
		const b = stops[i + 1];
		return `oklch(${(a[0] + (b[0] - a[0]) * f).toFixed(3)} ${(a[1] + (b[1] - a[1]) * f).toFixed(3)} ${(a[2] + (b[2] - a[2]) * f).toFixed(1)})`;
	}
	function parseOkLCH(v: string): [number, number, number] {
		const m = v.match(/oklch\(\s*([\d.]+)\s+([\d.]+)\s+([\d.]+)/);
		return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : [0.585, 0.233, 15.458];
	}

</script>

<!-- svelte-ignore a11y_no_static_element_interactions -- handlers only detect scroll intent -->
<div
	bind:this={scroller}
	onwheel={onUserScroll}
	ontouchmove={onUserScroll}
	onpointerdown={onUserScroll}
	class="lyrics-scroller min-h-0 flex-1 overflow-y-auto {compact
		? 'px-2 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden'
		: expanded
			? 'px-10 py-6'
			: 'px-5 py-6'}"
>
	{#if loading}
		<div class="space-y-3">
			{#each { length: 8 } as _, i (i)}
				<div class="h-5 animate-pulse rounded bg-muted" style="width:{55 + ((i * 17) % 40)}%"></div>
			{/each}
		</div>
	{:else if lyrics?.instrumental}
		<p class="py-8 text-center text-lg text-muted-foreground">Instrumental ♪</p>
	{:else if lyrics && lyrics.synced}
		<!-- No top/bottom gap: lyrics start at the top like any list. Centering still applies
		     mid-song; the first/last lines simply clamp to the edges. -->
		<div class={expanded ? 'mx-auto max-w-3xl' : ''}>
			{#each lyrics.lines as line, i (i)}
				{@const isActive = i === activeIndex}
				{@const isPast = i < activeIndex}
				<button
					data-line={i}
					onclick={() => seekTo(line)}
					class="block w-full origin-left cursor-pointer text-left font-heading font-bold leading-snug transition-[color,transform,opacity,filter] duration-500 ease-[cubic-bezier(0.22,1,0.36,1)]
						{sing
						? `py-4 ${isActive ? 'text-4xl md:text-6xl' : 'text-2xl md:text-3xl'}`
						: expanded
							? 'py-3 text-3xl'
							: compact
								? 'py-1 text-sm'
								: 'py-2 text-xl'}
						{isActive
						? line.words?.length
								? 'scale-[1.04]'
								: 'text-gradient scale-[1.04] [filter:drop-shadow(0_2px_14px_color-mix(in_srgb,var(--primary)_30%,transparent))]'
						: isPast
							? (expanded ? 'text-muted-foreground/15 blur-[1.5px] opacity-80 hover:blur-0 hover:text-muted-foreground/50 hover:blur-none transition-[filter]' : 'text-muted-foreground/40 hover:text-muted-foreground/75')
							: (expanded ? 'text-muted-foreground/25 blur-[1px] opacity-90 hover:blur-0 hover:text-muted-foreground/60 transition-[filter] scale-[0.97]' : 'text-muted-foreground/70 hover:text-foreground/90')}"
				>
					{#if line.words && line.words.length > 0}
						<!-- Word-by-word karaoke — the Aurora gradient sweeps across each word, with a gentle vertical float -->
						{@const wordCount = Math.max(1, line.words.length)}
						<span class="inline-flex flex-wrap items-baseline {isActive ? 'drop-shadow-[0_2px_12px_color-mix(in_srgb,var(--primary)_40%,transparent)] [animation:karaoke-float_3s_ease-in-out_infinite_alternate]' : ''}">
							{#each line.words as word, wIdx (wIdx)}
								{@const isWordEnd = word.text.endsWith(' ')}
								{@const cleanText = word.text.trimEnd()}
								{#if isActive}
									{@const progress = getWordProgress(word, posMs)}
									{@const pct = Math.round(Math.min(1, Math.max(0, progress)) * 100)}
									{@const isCurrentWord = progress > 0 && progress < 1}
									<!-- Only the gradient stop moves per frame; the clip/fill are static, so they
									     live in the class and aren't re-serialised 60 times a second. Both
									     colours are theme tokens: the sung half was hardcoded white, which is
									     invisible on every light theme. -->
									<span
										class="inline-block bg-clip-text text-transparent [-webkit-text-fill-color:transparent] transition-transform will-change-transform [transition-timing-function:cubic-bezier(0.34,1.56,0.64,1)] duration-200 {isWordEnd ? 'mr-[0.26em]' : ''} {isCurrentWord
											? 'scale-[1.08] -translate-y-[3px]'
											: progress >= 1
												? 'scale-[1.03] -translate-y-[1px]'
												: ''}"
										style="background-image: linear-gradient(90deg, {sung(pct / 100)} {pct}%, color-mix(in srgb, {sung(pct / 100)} 22%, oklch(0.55 0.02 var(--hue)) {pct}%) {pct}%)"
									>
										{segText(i, wIdx, cleanText).trimEnd()}
									</span>
								{:else}
									<span class="inline-block {isWordEnd ? 'mr-[0.26em]' : ''} {isPast
										? (expanded ? 'text-muted-foreground/15' : 'text-muted-foreground/40')
										: (expanded ? 'text-muted-foreground/25' : 'text-muted-foreground/70')}">
										{segText(i, wIdx, cleanText).trimEnd()}
									</span>
								{/if}
							{/each}
						</span>
					{:else if isActive && !line.text}
						<!-- Instrumental break: three breathing dots instead of blank space. -->
						<span class="inline-flex items-center gap-1.5 py-2" aria-hidden="true">
							{#each [0, 1, 2] as d (d)}
								<span
									class="h-1.5 w-1.5 rounded-full bg-primary/70"
									style="animation: karaoke-breathe 1.6s ease-in-out infinite; animation-delay: {d * 0.25}s"
								></span>
							{/each}
						</span>
					{:else}
						<span>{segText(i, -1, line.text) || '♪'}</span>
					{/if}
					</button>
			{/each}
		</div>
	{:else if lyrics}
		<div
			class="space-y-2 leading-relaxed text-foreground/90 {expanded
				? 'mx-auto max-w-3xl text-xl'
				: compact
					? 'text-xs'
					: 'text-[15px]'}"
		>
			{#each lyrics.lines as line, i (i)}
				{#if line.text}
					<div class={sing ? 'text-2xl md:text-3xl' : ''}>
						<p>{segText(i, -1, line.text)}</p>
					</div>
				{:else}
					<div class="h-4"></div>
				{/if}
			{/each}
			</div>
		{:else}
			<div class="flex flex-col items-center gap-3 py-8 text-center">
				<p class="text-sm text-muted-foreground">No lyrics found for this track.</p>
				{#if playback.now && !api.isLocalId(playback.now.videoId)}
					<button
						class="flex cursor-pointer items-center gap-1.5 rounded-lg border px-2.5 py-1.5 text-xs font-medium text-muted-foreground transition-colors hover:border-foreground/20 hover:bg-muted hover:text-foreground"
						onclick={attachLyricsFile}
						title="Import a .lrc file for this song"
					>
						<HugeiconsIcon icon={Attachment01Icon} class="h-3.5 w-3.5" /> Import .lrc
					</button>
				{/if}
			</div>
		{/if}
</div>
{#if lyrics && !loading && !compact && !sing}
	<p class="flex items-center gap-2 border-t px-4 py-2 text-xs text-muted-foreground">
		<span class="truncate">
			{lyrics.source.startsWith('Source:') ? lyrics.source : `Lyrics from ${lyrics.source}`}
		</span>
		{#if hasKana}
			<button
				class="shrink-0 cursor-pointer rounded-full border px-2 py-0.5 font-semibold tracking-wide uppercase transition-colors {romanOn
					? 'border-primary/60 text-primary'
					: 'hover:border-foreground/20 hover:text-foreground'}"
				onclick={toggleRoman}
				disabled={romanBusy}
				title="Show kana lyrics in romaji"
				aria-pressed={romanOn}
			>
				{romanBusy ? '…' : romanOn ? 'かな' : 'Romaji'}
			</button>
		{/if}
		{#if lyrics.source === 'Custom file'}
			<button
				class="ml-auto shrink-0 cursor-pointer underline-offset-2 hover:underline"
				onclick={removeAttachedLyrics}
			>
				Remove
			</button>
		{/if}
	</p>
{/if}

{#if sing}
	<div class="absolute top-4 left-6 z-20 sm:left-14"></div>
{/if}

<style>
@keyframes karaoke-breathe {
	0%, 100% { opacity: 0.35; transform: translateY(0); }
	50% { opacity: 1; transform: translateY(-2px); }
}
@keyframes karaoke-float {
	from { transform: translateY(-2.5px); }
	to { transform: translateY(2.5px); }
}
</style>
