<script lang="ts">
	import * as api from '$lib/api';
	import { playback } from '$lib/player.svelte';

	// `expanded` only sizes the type and centres the column. The owner of the extra room (the side
	// panel, or the now-playing view) decides how much there is. Toggling it must not remount this
	// component, or the lyrics refetch and the scroll position is lost.
	// `compact` is the mini-player: a ~220px column with no room for the source footer or a
	// scrollbar. It only shrinks the type and chrome; the sync/auto-scroll logic is identical.
	let { expanded = false, compact = false }: { expanded?: boolean; compact?: boolean } =
		$props();

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

	// videoId of the fetch whose result is (or will be) shown — guards stale responses.
	let requested = '';

	$effect(() => {
		const now = playback.now;
		if (!now) {
			requested = '';
			lyrics = null;
			loading = false;
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
			})
			.catch(() => {
				if (requested !== id) return;
				loading = false;
			});
	});

	// Last synced line whose cue has passed (lines arrive sorted by time).
	const activeIndex = $derived.by(() => {
		if (!lyrics?.synced) return -1;
		const currentMs = posMs;
		let i = -1;
		for (let j = 0; j < lyrics.lines.length; j++) {
			const t = lyrics.lines[j].time_ms;
			if (t === undefined) continue;
			if (t > currentMs) break;
			i = j;
		}
		return i;
	});

	// Auto-scroll pauses while the user is scrolling (wheel/touch/scrollbar), resumes after 3s.
	// Tracked via input events, not `scroll`, so our own smooth scrolls don't trip it.
	let userScrollUntil = 0;
	// Idea 8: gentle idle dim in fullscreen — after 30s without interaction the panel breathes
	// down to 60% opacity so it doubles as ambient art rather than a bright wall.
	let idleDim = $state(false);
	let idleTimer: ReturnType<typeof setTimeout> | undefined;
	function resetIdle() {
		idleDim = false;
		clearTimeout(idleTimer);
		if (expanded) idleTimer = setTimeout(() => (idleDim = true), 30000);
	}
	let hasScrolled = false;
	function onUserScroll() {
		userScrollUntil = Date.now() + 3000;
		resetIdle();
	}

	$effect(() => {
		expanded; // track — restarting the idle dim on enter/leave fullscreen
		resetIdle();
	});
	let wasExpanded: boolean | undefined;

	$effect(() => {
		const i = activeIndex;
		// Re-centre after the layout width/font changes, and jump rather than glide across it.
		// (Also fires on the first run, where both values are already at their defaults.)
		if (expanded !== wasExpanded) {
			wasExpanded = expanded;
			hasScrolled = false;
			userScrollUntil = 0;
		}
		if (i < 0 || !scroller || Date.now() < userScrollUntil) return;
		scroller.querySelector(`[data-line="${i}"]`)?.scrollIntoView({
			// Opening mid-song jumps straight to the line; after that, glide.
			behavior: hasScrolled ? 'smooth' : 'instant',
			block: 'center'
		});
		hasScrolled = true;
	});

	function seekTo(line: api.LyricLine) {
		if (line.time_ms === undefined) return;
		const secs = line.time_ms / 1000;
		playback.position = secs; // optimistic — the mpv tick confirms
		userScrollUntil = 0; // jump the view along with the seek
		api.seek(secs);
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
	class="min-h-0 flex-1 overflow-y-auto {compact
		? 'px-2 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden'
		: expanded
			? 'px-10 py-6'
			: 'px-5 py-6'} {idleDim ? 'opacity-60 transition-opacity duration-1000' : ''}"
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
		<!-- Padding lets the first/last lines center-scroll. -->
		<div class="py-[35vh] {expanded ? 'mx-auto max-w-3xl' : ''}">
			{#each lyrics.lines as line, i (i)}
				{@const isActive = i === activeIndex}
				{@const isPast = i < activeIndex}
				<button
					data-line={i}
					onclick={() => seekTo(line)}
					class="block w-full origin-left cursor-pointer text-left font-heading font-bold leading-snug transition-[color,transform,opacity,filter] duration-300 ease-out
						{expanded ? 'py-3 text-3xl' : compact ? 'py-1 text-sm' : 'py-2 text-xl'}
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
										{cleanText}
									</span>
								{:else}
									<span class="inline-block {isWordEnd ? 'mr-[0.26em]' : ''} {isPast
										? (expanded ? 'text-muted-foreground/15' : 'text-muted-foreground/40')
										: (expanded ? 'text-muted-foreground/25' : 'text-muted-foreground/70')}">
										{cleanText}
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
						<span>{line.text || '♪'}</span>
					{/if}

					<!-- Translation line rendering -->
					{#if line.translation}
						<p class="mt-1 text-sm font-normal italic tracking-wide opacity-80 transition-opacity">
							{line.translation}
						</p>
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
					<div>
						<p>{line.text}</p>
							{#if line.translation}
								<p class="text-xs italic text-muted-foreground">{line.translation}</p>
							{/if}
						</div>
					{:else}
						<div class="h-4"></div>
					{/if}
				{/each}
			</div>
		{:else}
			<p class="py-8 text-center text-sm text-muted-foreground">No lyrics found for this track.</p>
		{/if}
</div>
{#if lyrics && !loading && !compact}
	<p class="border-t px-4 py-2 text-xs text-muted-foreground">
		{lyrics.source.startsWith('Source:') ? lyrics.source : `Lyrics from ${lyrics.source}`}
	</p>
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
