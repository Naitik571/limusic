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
	let hasScrolled = false;
	function onUserScroll() {
		userScrollUntil = Date.now() + 3000;
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
			<p class="py-8 text-center text-lg text-muted-foreground">Instrumental ♪</p>
		{:else if lyrics && lyrics.synced}
			<!-- Padding lets the first/last lines center-scroll. -->
			<div class="py-[35vh] {expanded ? 'mx-auto max-w-3xl' : ''}">
				{#each lyrics.lines as line, i (i)}
					{@const isActive = i === activeIndex}
					{@const isPast = i < activeIndex}
					<!-- The active line reads as the focal point: gradient + a soft primary glow for
					     whole-line lyrics, a scale-up for karaoke lines (their per-word sweep is the
					     colour). Everything else recedes in muted steps, waking up on hover. -->
					<button
						data-line={i}
						onclick={() => seekTo(line)}
						class="block w-full origin-left cursor-pointer text-left font-heading font-bold leading-snug transition-[color,transform,opacity,filter] duration-300 ease-out
							{expanded ? 'py-3 text-3xl' : 'py-2 text-xl'}
							{isActive
								? line.words?.length
									? 'scale-[1.04]'
									: 'text-gradient scale-[1.04] [filter:drop-shadow(0_2px_14px_color-mix(in_srgb,var(--primary)_30%,transparent))]'
								: isPast
									? 'text-muted-foreground/35 hover:text-muted-foreground/75'
									: 'text-muted-foreground/65 hover:text-foreground/90'}"
					>
						{#if line.words && line.words.length > 0}
							<!-- Word-by-Word Karaoke — theme-tinted sweep + bouncing ball -->
							<span class="inline-flex flex-wrap items-baseline {isActive ? 'drop-shadow-[0_1px_10px_color-mix(in_srgb,var(--primary)_40%,transparent)]' : ''}">
								{#each line.words as word, wIdx (wIdx)}
									{@const isWordEnd = word.text.endsWith(' ')}
									{@const cleanText = word.text.trimEnd()}
									{#if isActive}
										{@const progress = getWordProgress(word, posMs)}
										{@const pct = Math.round(Math.min(1, Math.max(0, progress)) * 100)}
										{@const isCurrentWord = progress > 0 && progress < 1}
										{@const isSung = progress >= 1}
										<span class="relative inline-block {isWordEnd ? 'mr-[0.26em]' : ''}">
											<!-- Text: bluish-purplish karaoke sweep using primary->accent gradient for sung portion, muted for unsung. -->
											<span
												class="inline-block bg-clip-text text-transparent [-webkit-text-fill-color:transparent] transition-transform duration-100 ease-out {isCurrentWord ? 'scale-[1.04]' : ''} {isSung ? 'scale-[1.02]' : ''}"
												style="background-image: linear-gradient(90deg, var(--primary) {pct}%, color-mix(in oklab, var(--primary) 55%, var(--accent) 45%) {pct}%, var(--muted-foreground) {pct}%)"
											>
												{cleanText}
											</span>
											<!-- Bouncing ball: sits on the baseline center of the current word, hops with an ease. Only on the word actively sweeping. -->
											{#if isCurrentWord}
												<span
													class="pointer-events-none absolute left-1/2 -top-2 h-1.5 w-1.5 -translate-x-1/2 rounded-full bg-primary shadow-[0_0_6px_var(--primary)]"
													style="animation: karaoke-ball 0.5s ease-in-out infinite alternate"
												></span>
											{/if}
										</span>
									{:else}
										<span class="inline-block {isWordEnd ? 'mr-[0.26em]' : ''} {isPast ? 'text-muted-foreground/35' : 'text-muted-foreground/65'}">{cleanText}</span>
									{/if}
								{/each}
							</span>
						{:else}
							<span>{line.text || '♪'}</span>
						{/if}

						<!-- Translation line rendering -->
						{#if line.translation}
							<p
								class="mt-1 font-sans text-sm font-normal tracking-wide transition-colors {isActive
									? 'text-muted-foreground/90'
									: 'text-muted-foreground/50'}"
							>
								{line.translation}
							</p>
						{/if}
					</button>
				{/each}
			</div>
		{:else if lyrics}
			<div
				class="space-y-2 leading-loose text-foreground/90 {expanded
					? 'mx-auto max-w-3xl text-xl'
					: 'text-[15px]'}"
			>
				{#each lyrics.lines as line, i (i)}
					{#if line.text}
						<div class="transition-colors hover:text-foreground">
							<p>{line.text}</p>
							{#if line.translation}
								<p class="text-sm italic tracking-wide text-muted-foreground/70">
									{line.translation}
								</p>
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
{#if lyrics && !loading}
	<p class="border-t px-4 py-2 text-xs text-muted-foreground">
		{lyrics.source.startsWith('Source:') ? lyrics.source : `Lyrics from ${lyrics.source}`}
	</p>
{/if}

<style>
@keyframes karaoke-ball {
	from { transform: translateX(-50%) translateY(0); }
	to { transform: translateX(-50%) translateY(-4px); }
}
</style>
