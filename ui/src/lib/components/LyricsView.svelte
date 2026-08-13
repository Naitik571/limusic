<script lang="ts">
	import * as api from '$lib/api';
	import { playback } from '$lib/player.svelte';

	// `expanded` only sizes the type and centres the column. The owner of the extra room (the side
	// panel, or the now-playing view) decides how much there is. Toggling it must not remount this
	// component, or the lyrics refetch and the scroll position is lost.
	let { expanded = false }: { expanded?: boolean } = $props();

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
		const posMs = playback.position * 1000;
		let i = -1;
		for (let j = 0; j < lyrics.lines.length; j++) {
			const t = lyrics.lines[j].time_ms;
			if (t === undefined) continue;
			if (t > posMs) break;
			i = j;
		}
		return i;
	});

	// Raw playback position in ms (from the throttled ~250ms position tick).
	const posMs = $derived(playback.position * 1000);

	// Smooth playback position in ms, interpolated every animation frame. The position tick only
	// arrives every ~250ms, so feeding `posMs` straight into the word sweep makes the gradient
	// jump in visible steps. `smoothMs` eases toward the latest tick continuously, so the sweep
	// reads as a fluid highlight even between ticks.
	let smoothMs = $state(0);
	{
		// Read `playback.position` only inside the RAF closure (never synchronously) so this
		// effect has no reactive deps and runs exactly once.
		let raf = 0;
		const tick = () => {
			const target = playback.position * 1000;
			if (smoothMs === 0) smoothMs = target;
			else smoothMs += (target - smoothMs) * 0.22;
			raf = requestAnimationFrame(tick);
		};
		$effect(() => {
			raf = requestAnimationFrame(tick);
			return () => cancelAnimationFrame(raf);
		});
	}

	// 0..1 progress through a single word, clamped. Drives the gradient sweep on the active line.
	function getWordProgress(word: api.LyricWord, currentMs: number): number {
		if (currentMs <= word.start_ms) return 0;
		if (currentMs >= word.end_ms) return 1;
		const dur = word.end_ms - word.start_ms;
		if (dur <= 0) return 1;
		return (currentMs - word.start_ms) / dur;
	}

	// Auto-scroll pauses while the user is scrolling (wheel/touch/scrollbar), resumes after 3s.
	// Tracked via input events, not `scroll`, so our own smooth scrolls don't trip it.
	let userScrollUntil = 0;
	let hasScrolled = false;
	function onUserScroll() {
		userScrollUntil = Date.now() + 3000;
		cancelAnimationFrame(scrollRaf); // the user took over mid-glide
	}

	// Auto-scroll glides with a hand-tweened ease (native smooth scroll's duration isn't
	// controllable and its clamp at the container edges feels like a snap). EaseInOutQuint,
	// ~650ms: a long, soft start and finish so a line change reads as a glide, not a jump.
	let scrollRaf = 0;
	function tweenScrollTo(el: HTMLElement, to: number, dur: number) {
		cancelAnimationFrame(scrollRaf);
		const from = el.scrollTop;
		if (dur <= 0 || Math.abs(to - from) < 2) {
			el.scrollTop = to;
			return;
		}
		const t0 = performance.now();
		const ease = (t: number) => (t < 0.5 ? 16 * t * t * t * t * t : 1 - Math.pow(-2 * t + 2, 5) / 2);
		const step = (now: number) => {
			const p = Math.min(1, (now - t0) / dur);
			el.scrollTop = from + (to - from) * ease(p);
			if (p < 1) scrollRaf = requestAnimationFrame(step);
		};
		scrollRaf = requestAnimationFrame(step);
	}

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
		const line = scroller.querySelector(`[data-line="${i}"]`) as HTMLElement | null;
		if (!line) return;
		// Centre the line in the viewport, in scroller coordinates.
		const target =
			line.getBoundingClientRect().top -
			scroller.getBoundingClientRect().top +
			scroller.scrollTop -
			(scroller.clientHeight - line.offsetHeight) / 2;
		// Opening mid-song jumps straight to the line; after that, glide.
		tweenScrollTo(scroller, target, hasScrolled ? 650 : 0);
		hasScrolled = true;
	});

	function seekTo(line: api.LyricLine) {
		if (line.time_ms === undefined) return;
		const secs = line.time_ms / 1000;
		playback.position = secs; // optimistic — the mpv tick confirms
		userScrollUntil = 0; // jump the view along with the seek
		api.seek(secs);
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -- handlers only detect scroll intent -->
<div
	bind:this={scroller}
	onwheel={onUserScroll}
	ontouchmove={onUserScroll}
	onpointerdown={onUserScroll}
	class="min-h-0 flex-1 overflow-y-auto py-6 {expanded ? 'px-10' : 'px-5'}"
>
	{#if loading}
		<div class="space-y-3">
			{#each { length: 8 } as _, i (i)}
				<div class="h-5 animate-pulse rounded bg-muted" style="width:{55 + ((i * 17) % 40)}%"></div>
			{/each}
		</div>
	{:else if lyrics?.instrumental}
		<!-- Just the music: a live equalizer + breathing copy instead of a dead text line. -->
		<div class="flex min-h-full flex-col items-center justify-center gap-4 py-12">
			{@render eqBars(48, 4)}
			<div class="text-center">
				<p class="font-heading text-lg font-semibold">Instrumental</p>
				<p class="lv-breathe mt-1 text-sm text-muted-foreground">
					Just the music — nothing to sing along to.
				</p>
			</div>
		</div>
	{:else if lyrics && lyrics.synced}
		<!-- Padding lets the first/last lines center-scroll. -->
		<div class="py-[35vh] {expanded ? 'mx-auto max-w-3xl' : ''}">
			{#each lyrics.lines as line, i (i)}
				<button
					data-line={i}
					onclick={() => seekTo(line)}
					class="flex w-full items-center gap-2.5 text-left font-heading font-semibold leading-snug transition-[color,transform,text-shadow,opacity] duration-500 ease-out hover:text-foreground
						{expanded ? 'py-3.5 text-4xl' : 'py-2 text-xl'}
						{i === activeIndex
							? 'lv-active lv-glow scale-[1.04]'
							: i < activeIndex
								? 'text-muted-foreground/35'
								: 'text-muted-foreground'}"
				>
					{#if i === activeIndex}
						<span class="flex h-[1.1em] shrink-0 items-end gap-[3px]" aria-hidden="true">
							<span class="lv-eq h-full w-[3px] rounded-full bg-primary" style="animation-delay:0s"></span>
							<span class="lv-eq h-full w-[3px] rounded-full bg-primary" style="animation-delay:0.15s"></span>
							<span class="lv-eq h-full w-[3px] rounded-full bg-primary" style="animation-delay:0.3s"></span>
						</span>
					{/if}
					{#if line.words && line.words.length > 0}
						<!-- Word-by-word karaoke sweep (Better-Lyrics style, highly optimized):
						     only the gradient stop moves per frame, so the clip/fill stay static. -->
						<span class="min-w-0 flex-1 inline-flex flex-wrap items-baseline">
							{#each line.words as word, wIdx (wIdx)}
								{@const isWordEnd = word.text.endsWith(' ') || word.text.endsWith('\n')}
								{@const cleanText = word.text.trimEnd()}
								{#if i === activeIndex}
									{@const progress = getWordProgress(word, smoothMs)}
									{@const pct = Math.round(Math.min(1, Math.max(0, progress)) * 100)}
									{@const isCurrentWord = progress > 0 && progress < 1}
									<span
										class="inline-block bg-clip-text text-transparent [-webkit-text-fill-color:transparent] {isWordEnd ? 'mr-[0.26em]' : ''} {isCurrentWord ? 'scale-[1.03]' : ''}"
										style="background-image: linear-gradient(90deg, var(--foreground) {pct}%, var(--muted-foreground) {pct}%)"
									>{cleanText}</span>
								{:else}
									<span class="inline-block {isWordEnd ? 'mr-[0.26em]' : ''} {i < activeIndex ? 'text-muted-foreground/35' : 'text-muted-foreground'}">{cleanText}</span>
								{/if}
							{/each}
						</span>
					{:else}
						<span class="min-w-0 flex-1">{line.text || '♪'}</span>
					{/if}
				</button>
			{/each}
		</div>
	{:else if lyrics}
		<div
			class="space-y-1 leading-relaxed text-foreground/90 {expanded
				? 'mx-auto max-w-3xl text-2xl'
				: 'text-base'}"
		>
			{#each lyrics.lines as line, i (i)}
				{#if line.text}<p>{line.text}</p>{:else}<div class="h-4"></div>{/if}
			{/each}
		</div>
	{:else}
		<!-- No lyrics (or nothing playing): floating notes + breathing copy so the empty state
		     feels alive rather than dead-ended. -->
		<div class="flex min-h-full flex-col items-center justify-center gap-4 py-12">
			<div class="relative flex h-24 w-24 items-center justify-center">
				<span class="lv-float absolute text-6xl text-primary/55">♪</span>
				<span class="lv-float-slow absolute -top-1 left-2 text-2xl text-muted-foreground/40">♫</span>
				<span class="lv-float-slower absolute -bottom-1 right-1 text-xl text-muted-foreground/30">♩</span>
			</div>
			<div class="text-center">
				<p class="font-heading text-base font-semibold">
					{playback.now ? 'No lyrics found for this track' : 'Nothing playing'}
				</p>
				<p class="lv-breathe mt-1 text-sm text-muted-foreground">
					{playback.now
						? 'Enjoy the music — the mood is the message.'
						: 'Pick a song and the lyrics will appear here.'}
				</p>
			</div>
		</div>
	{/if}
</div>
{#if lyrics && !loading}
	<p class="border-t px-4 py-2 text-xs text-muted-foreground">
		{lyrics.source.startsWith('Source:') ? lyrics.source : `Lyrics from ${lyrics.source}`}
	</p>
{/if}

{#snippet eqBars(heightPx: number, bars: number)}
	<span class="flex items-end gap-1" aria-hidden="true">
		{#each Array(bars) as _, n (n)}
			<span
				class="lv-eq w-1 rounded-full bg-primary"
				style="height:{heightPx}px; animation-delay:{n * 0.12}s"
			></span>
		{/each}
	</span>
{/snippet}

<style>
	/* The active line: full-brightness text with a soft primary glow so it reads as "now". */
	.lv-active {
		color: var(--foreground);
		text-shadow: 0 0 26px color-mix(in oklab, var(--primary) 55%, transparent);
	}

	/* Aurora: the active synced line reads as accent→violet gradient text. */
	.lv-glow {
		background-image: linear-gradient(100deg, var(--primary) 10%, oklch(0.7 0.2 285) 60%, oklch(0.65 0.19 335) 100%);
		-webkit-background-clip: text;
		background-clip: text;
		color: transparent;
		text-shadow: none;
	}

	/* Equalizer bars: scale from the bottom, staggered via inline animation-delay. */
	.lv-eq {
		transform-origin: bottom;
		animation: lv-eq 1.1s ease-in-out infinite;
	}

	/* Gentle bob for the empty-state notes, staggered durations/delays. */
	.lv-float {
		animation: lv-float 3.2s ease-in-out infinite;
	}
	.lv-float-slow {
		animation: lv-float 4.2s ease-in-out 0.7s infinite;
	}
	.lv-float-slower {
		animation: lv-float 5s ease-in-out 1.3s infinite;
	}

	/* Soft text pulse for the empty/instrumental copy. */
	.lv-breathe {
		animation: lv-breathe 2.4s ease-in-out infinite;
	}

	@keyframes lv-eq {
		0%,
		100% {
			transform: scaleY(0.3);
		}
		50% {
			transform: scaleY(1);
		}
	}

	@keyframes lv-float {
		0%,
		100% {
			transform: translateY(0);
		}
		50% {
			transform: translateY(-9px);
		}
	}

	@keyframes lv-breathe {
		0%,
		100% {
			opacity: 0.55;
		}
		50% {
			opacity: 1;
		}
	}
</style>
