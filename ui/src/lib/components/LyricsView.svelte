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
	import { t } from '$lib/i18n.svelte';

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

	// NOTE (Bug 1): the server already applies the persisted per-song offset to the cues
	// it returns, so there is deliberately NO client-side shift here (a shiftCues pass used to
	// double-shift every line). seekTo below uses the stored cue times verbatim; the offset
	// pill (follow-up batch) will adjust display/seek in one place only.

	// videoId of the fetch whose result is (or will be) shown — guards stale responses.
	let requested = '';
	// Monotonic fetch generation: incremented on every track/reload run so in-flight
	// getLyrics/romanize/translate continuations from a previous videoId can be ignored even
	// when the videoId string itself is slow to update. Checked alongside `requested`.
	let fetchSeq = 0;
	// Bumped after attaching/removing custom lyrics so the effect below refetches.
	// ($state: the effect tracks it — a plain let would make every bump a silent no-op.)
	let reloadKey = $state(0);
	// Set before an offset-pill-triggered refetch: the lines are identical, only the server-side
	// cues shift, so romaji/translation caches stay valid and are kept instead of cleared.
	let keepOverlays = false;
	// Romaji view (kana→romaji via the `romanize_lyrics` command): one round-trip for the
	// whole song — every word and line text joined by \n (romanization never touches
	// newlines), mapped back positionally. Timings and the karaoke sweep are untouched.
	let romanOn = $state(false);
	let romanSeg = $state<string[] | null>(null);
	let romanFor = '';
	let romanBusy = $state(false);
	// Per-line partial-transliteration flags, built alongside romanSeg: true when the line mixed
	// converted kana with passthrough kanji (output still holds CJK although the input had kana).
	// Pure-kanji lines (nothing converted) and fully converted lines stay unflagged.
	let romanPartial = $state<boolean[]>([]);
	const KANA_RE = /[\u3040-\u30ff]/;
	const CJK_RE = /[\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff]/;
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
		const seq = fetchSeq;
		try {
			const l = lyrics;
			const segs = segList(l);
			const origs = segs.map((s) =>
				s.word >= 0 ? (l.lines[s.line].words?.[s.word].text ?? '') : l.lines[s.line].text
			);
			const src = origs.join('\n');
			const out = await api.romanizeLyrics(src);
			const parts = out.split('\n');
			if (requested !== id || seq !== fetchSeq) return; // stale (track changed)
			if (parts.length !== segs.length) {
				toast.error(t('lyrics.romanize_mismatch'));
				return;
			}
			romanSeg = parts;
			romanFor = id;
			romanPartial = l.lines.map(() => false);
			parts.forEach((p, k) => {
				if (KANA_RE.test(origs[k] ?? '') && CJK_RE.test(p)) romanPartial[segs[k].line] = true;
			});
			romanOn = true;
		} catch {
			toast.error(t('lyrics.romanize_failed'));
		} finally {
			if (requested === id && seq === fetchSeq) romanBusy = false;
		}
	}
	/** Display text for a line or word segment (romaji when toggled, original otherwise). */
	function segText(line: number, word: number, fallback: string): string {
		if (!romanOn || !romanSeg) return fallback;
		const p = segIndex.get(`${line}:${word}`);
		return p === undefined ? fallback : (romanSeg[p] ?? fallback);
	}

	// Translation (translate_lyrics command, one round-trip for the whole song, joined by \n).
	// Line-level only: word timings stay on the original script, and the translated line
	// renders under it (Both) or in its place (Translated). Cached per song + language.
	const TRANS_LANGS = [
		['en', 'English'],
		['ja', '日本語'],
		['ko', '한국어'],
		['zh', '中文'],
		['es', 'Español'],
		['fr', 'Français'],
		['de', 'Deutsch'],
		['pt', 'Português'],
		['it', 'Italiano'],
		['hi', 'हिन्दी'],
		['tr', 'Türkçe'],
		['ro', 'Română']
	] as const;
	let transMode = $state<'off' | 'both' | 'only'>('off');
	let transLang = $state<string>('');
	let transSeg = $state<string[] | null>(null);
	let transFor = '';
	let transBusy = $state(false);
	function defaultTransLang(): string {
		const sys = navigator.language?.toLowerCase().split('-')[0] ?? 'en';
		return (TRANS_LANGS as readonly (readonly [string, string])[]).some(([c]) => c === sys)
			? sys
			: 'en';
	}
	async function setTransMode(mode: 'off' | 'both' | 'only') {
		if (!lyrics || mode === 'off') {
			transMode = mode;
			return;
		}
		if (!transLang) transLang = defaultTransLang();
		const key = `${requested}:${transLang}`;
		if (transFor === key && transSeg) {
			transMode = mode;
			return;
		}
		transBusy = true;
		const id = requested;
		const seq = fetchSeq;
		const lang = transLang;
		try {
			const src = lyrics.lines.map((l) => l.text).join('\n');
			const out = await api.translateLyrics(src, lang);
			const parts = out.split('\n');
			// Stale (track changed mid-flight) → drop silently. Reshaped (line count moved
			// under us) → toast, never silent: the user asked for a translation and got none.
			if (requested !== id || seq !== fetchSeq) return;
			if (parts.length !== lyrics.lines.length) {
				toast.error(t('lyrics.translate_mismatch'));
				return;
			}
			transSeg = parts;
			transFor = key;
			transMode = mode;
		} catch {
			toast.error(t('lyrics.translate_failed'));
		} finally {
			if (requested === id && seq === fetchSeq) transBusy = false;
		}
	}
	function cycleTrans() {
		void setTransMode(transMode === 'off' ? 'both' : transMode === 'both' ? 'only' : 'off');
	}
	/**
	 * Translated line text, or null when translation is off/empty/identical for this line.
	 * Identical lines are suppressed so Both mode never renders the same string twice and
	 * Only mode falls back to the original instead of a redundant copy.
	 */
	function transText(i: number): string | null {
		if (transMode === 'off' || !transSeg) return null;
		const t = transSeg[i]?.trim();
		if (!t) return null;
		if (lyrics?.lines[i]?.text?.trim() === t) return null;
		return t;
	}

	// Per-song timing offset pill (footer −/value/+). The server owns shifting: it applies the
	// persisted offset to the cues it returns, so this only reads the setting (getLyricOffset on
	// track load) and writes it (setLyricOffset, debounced ~300ms), then refetches so the shifted
	// cues reflect. Deliberately NO client-side cue math — see the Bug 1 note above.
	let lyricOffsetMs = $state(0);
	let offsetFor = '';
	let offsetTimer: ReturnType<typeof setTimeout> | null = null;
	onDestroy(() => {
		if (offsetTimer) clearTimeout(offsetTimer);
	});
	/** 0 → "±0ms", |v|<1000 → "+300ms"/"−300ms", else "+1.2s". */
	function fmtOffset(ms: number): string {
		if (ms === 0) return '±0ms';
		const sign = ms > 0 ? '+' : '−';
		const a = Math.abs(ms);
		return a >= 1000 ? `${sign}${(a / 1000).toFixed(1)}s` : `${sign}${a}ms`;
	}
	function scheduleOffsetWrite() {
		const vid = offsetFor || playback.now?.videoId;
		if (!vid) return;
		if (offsetTimer) clearTimeout(offsetTimer);
		offsetTimer = setTimeout(() => {
			const v = lyricOffsetMs;
			api
				.setLyricOffset(vid, v)
				.then(() => {
					// Same lines, shifted cues: keep romaji/translation and force the fetch
					// effect past its same-track early return so the new cues load.
					keepOverlays = true;
					requested = '';
					reloadKey++;
				})
				.catch((e) => toast.error(String(e)));
		}, 300);
	}
	function nudgeOffset(d: number) {
		// Mirror the server-side ±5000ms clamp so the pill never displays an unpersistable value.
		lyricOffsetMs = Math.max(-5000, Math.min(5000, lyricOffsetMs + d));
		scheduleOffsetWrite();
	}
	function resetOffset() {
		if (lyricOffsetMs === 0) return;
		lyricOffsetMs = 0;
		scheduleOffsetWrite();
	}

	// Segment positions (`line:word` → index into romanSeg), built once per lyrics instead of
	// per word per frame — segText runs for every visible word on every karaoke frame.
	const segIndex = $derived.by(() => {
		const m = new Map<string, number>();
		const l = lyrics;
		if (!l) return m;
		let k = 0;
		l.lines.forEach((ln, i) => {
			if (ln.words?.length) ln.words.forEach((_, w) => m.set(`${i}:${w}`, k++));
			else m.set(`${i}:-1`, k++);
		});
		return m;
	});

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
			romanPartial = [];
			transMode = 'off';
			transSeg = null;
			transFor = '';
			offsetFor = '';
			lyricOffsetMs = 0;
			return;
		}
		if (now.videoId === requested) return;
		const id = (requested = now.videoId);
		const seq = ++fetchSeq; // invalidates every in-flight continuation from the old track
		loading = true;
		lyrics = null;
		romanBusy = false;
		transBusy = false;
		// Persisted per-song offset for the pill (display/adjust only — the server already
		// applied it to the cues below). Skipped when this run is an offset-triggered refetch
		// of the same song: the pill already holds the just-written value.
		if (offsetFor !== id) {
			lyricOffsetMs = 0;
			api
				.getLyricOffset(id)
				.then((v) => {
					if (requested !== id || seq !== fetchSeq) return;
					lyricOffsetMs = v ?? 0;
					offsetFor = id;
				})
				.catch(() => {});
		}
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
				// Abort-on-track-change: a newer run already claimed `requested`/`fetchSeq`.
				if (requested !== id || seq !== fetchSeq) return;
				lyrics = l;
				loading = false;
				hasScrolled = false; // first positioning on a new track is an instant jump
				autoPaused = false; // a new track always resumes autoscroll
				// An offset-pill refetch carries the same lines with shifted cues, so the romaji
				// and translation caches (positional over those lines) stay valid and are kept.
				if (!keepOverlays) {
					romanOn = false; // new song, new script — romaji starts off
					romanSeg = null;
					romanFor = '';
					romanPartial = [];
					transMode = 'off'; // same for translation
					transSeg = null;
					transFor = '';
				}
				keepOverlays = false;
				// NOTE (Bug 1): no client-side offset shift — the server already returns cues
				// shifted by the persisted per-song offset. (The follow-up offset pill will
				// read api.getLyricOffset for display/adjustment only.)
			})
			.catch(() => {
				if (requested !== id || seq !== fetchSeq) return;
				loading = false;
				lyrics = null;
				// Same reset as the success path: a failed fetch must not leave romaji or a
				// translation from the previous song armed for this one.
				romanOn = false;
				romanSeg = null;
				romanFor = '';
				romanPartial = [];
				transMode = 'off';
				transSeg = null;
				transFor = '';
				keepOverlays = false;
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

	// Trailer: the line just before the active one while its end cue hasn't passed (line ends
	// often overlap the next start). Rendered sung but never zoomed — only the active line
	// owns the scale.
	const trailerIndex = $derived.by(() => {
		if (!lyrics || activeIndex <= 0) return -1;
		const prev = activeIndex - 1;
		const end = lineEndMs(lyrics.lines[prev], lyrics.lines, prev);
		return end !== undefined && end > posMs ? prev : -1;
	});

	/** End cue for trailer detection: explicit end, else last word end, else next line start. */
	function lineEndMs(
		line: api.LyricLine | undefined,
		lines: api.LyricLine[],
		i: number
	): number | undefined {
		if (!line) return undefined;
		if (line.end_time_ms !== undefined) return line.end_time_ms;
		const words = line.words;
		if (words?.length) return words[words.length - 1].end_ms;
		for (let j = i + 1; j < lines.length; j++) {
			if (lines[j].time_ms !== undefined) return lines[j].time_ms;
		}
		return undefined;
	}

	// Auto-scroll pauses the moment the user takes the wheel/touch/scrollbar and stays
	// paused until the Resume pill is tapped (no timed auto-resume: a slow reader must never
	// have the view yanked away). Tracked via input events, not `scroll`, so our own tweens
	// never trip it. Jumps (track change / seek) clear the pause and snap; sequential line
	// advances glide while unpaused.
	let autoPaused = $state(false);
	let hasScrolled = false;
	function onUserScroll() {
		autoPaused = true;
		cancelScrollTween();
	}
	function resumeAuto() {
		autoPaused = false;
		hasScrolled = true; // resume glides from here; the next advance is sequential, not a jump
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
			autoPaused = false;
		}
		if (i < 0 || !scroller || autoPaused) return;
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

	/** Seek to a stored cue verbatim — no offset math (server already shifted the cues). */
	function seekTo(line: api.LyricLine) {
		if (line.time_ms === undefined) return;
		const secs = line.time_ms / 1000;
		playback.position = secs; // optimistic — the mpv tick confirms
		autoPaused = false; // a seek is a jump: clear the pause so the view follows it
		hasScrolled = false; // …and snap to it: the scroll effect jumps until positioned once
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
				title: t('lyrics.attach_title', { title: now.title }),
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
			toast.success(t('lyrics.attached_ok'));
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
			toast.success(t('lyrics.detached_ok'));
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

</script>

{#snippet breathe()}
	<!-- Single empty-cue treatment, shared by the synced and unsynced branches. -->
	<span class="inline-flex items-center gap-1.5 py-2" aria-hidden="true">
		{#each [0, 1, 2] as d (d)}
			<span
				class="h-1.5 w-1.5 rounded-full bg-primary/70"
				style="animation: karaoke-breathe 1.6s ease-in-out infinite; animation-delay: {d * 0.25}s"
			></span>
		{/each}
	</span>
{/snippet}

{#snippet lineBody(line: api.LyricLine, i: number, isActive: boolean, isPast: boolean, isTrailer: boolean)}
	{#if transMode === 'only' && transText(i)}
		<span>{transText(i)}</span>
	{:else}
		{#if line.words && line.words.length > 0}
			<!-- Word karaoke: dim baseline everywhere; the CURRENT word only gets a bright
			     overlay wiped per-frame via clip-path. Sung/trailer words are a static primary,
			     past words a static dim — no per-frame work, no word-level transitions. -->
			<span class="inline-flex flex-wrap items-baseline {isActive ? 'drop-shadow-[0_2px_12px_color-mix(in_srgb,var(--primary)_40%,transparent)]' : ''}">
				{#each line.words as word, wIdx (wIdx)}
					{@const isWordEnd = word.text.endsWith(' ')}
					{@const cleanText = word.text.trimEnd()}
					{@const progress = isActive ? getWordProgress(word, posMs) : 0}
					{@const pct = Math.round(Math.min(1, Math.max(0, progress)) * 100)}
					{@const label = segText(i, wIdx, cleanText).trimEnd()}
					{#if isActive && progress > 0 && progress < 1}
						<span class="relative inline-block {isWordEnd ? 'mr-[0.26em]' : ''}">
							<span class="lyric-unsung {romanOn ? 'lyric-romaji' : ''}">{label}</span>
							<span
								aria-hidden="true"
								class="lyric-sung absolute inset-0 overflow-hidden whitespace-nowrap {romanOn ? 'lyric-romaji' : ''}"
								style="clip-path: inset(0 {(100 - pct).toFixed(1)}% 0 0)"
							>{label}</span>
						</span>
					{:else if (isActive && progress >= 1) || isTrailer}
						<span class="lyric-sung inline-block {romanOn ? 'lyric-romaji' : ''} {isWordEnd ? 'mr-[0.26em]' : ''}">{label}</span>
					{:else if isPast}
						<span class="lyric-past inline-block {romanOn ? 'lyric-romaji' : ''} {isWordEnd ? 'mr-[0.26em]' : ''}">{label}</span>
					{:else}
						<span class="lyric-unsung inline-block {romanOn ? 'lyric-romaji' : ''} {isWordEnd ? 'mr-[0.26em]' : ''}">{label}</span>
					{/if}
				{/each}
			</span>
		{:else if !line.text?.trim()}
			{@render breathe()}
		{:else}
			<span class="{romanOn ? 'lyric-romaji' : ''} {isTrailer ? 'lyric-sung' : isPast ? 'lyric-past' : isActive ? '' : 'lyric-unsung'}">{segText(i, -1, line.text)}</span>
		{/if}
		{#if romanOn && romanPartial[i]}
			<!-- Partial transliteration: converted kana mixed with passthrough kanji. One subtle
			     mark per line, details in the tooltip — no layout or color change. -->
			<span class="roman-partial" title={t('lyrics.partial_romaji')}>※</span>
		{/if}
		{#if transMode === 'both' && transText(i)}
			<span class="lyric-trans mt-1 block font-medium normal-case tracking-normal opacity-60">{transText(i)}</span>
		{/if}
	{/if}
{/snippet}

<!-- Autoscroll pause + Resume pill live on this wrapper (not the scroller): an absolute pill
     inside the scroll container would scroll away with the lyrics. -->
<div class="relative flex min-h-0 flex-1 flex-col">
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
		<!-- Instrumental: a dedicated calm presentation — breathing dots + label, no seek
		     buttons (fake or otherwise). No auto-switch behavior is changed elsewhere. -->
		<div class="flex flex-col items-center gap-3 py-12 text-center">
			{@render breathe()}
			<p class="text-lg font-semibold text-[var(--text-2)]">{t('lyrics.instrumental')}</p>
			<p class="text-xs text-[var(--text-4)]">{t('lyrics.instrumental_hint')}</p>
		</div>
	{:else if lyrics && lyrics.synced}
		<!-- No top/bottom gap: lyrics start at the top like any list. Centering still applies
		     mid-song; the first/last lines simply clamp to the edges. -->
		<div class={expanded ? 'mx-auto max-w-3xl' : ''}>
			{#each lyrics.lines as line, i (i)}
				{@const isActive = i === activeIndex}
				{@const isTrailer = i === trailerIndex}
				{@const isPast = i < activeIndex && !isTrailer}
				<!-- One motion owner: the line owns scale/opacity/color. Words never transition
				     or animate (only the current word's overlay clip-path updates per frame). -->
				{@const sizeCls = sing
					? `py-4 ${isActive ? 'text-4xl md:text-6xl' : isTrailer ? 'text-3xl md:text-4xl' : 'text-2xl md:text-3xl'}`
					: expanded
						? 'py-3 text-3xl'
						: compact
							? 'py-1 text-sm'
							: 'py-2 text-xl'}
				{@const stateCls = isActive
					? line.words?.length
						? 'scale-[1.04] text-[var(--text-1)]'
						: 'text-gradient scale-[1.04] [filter:drop-shadow(0_2px_14px_color-mix(in_srgb,var(--primary)_30%,transparent))]'
					: isTrailer
						? 'text-[var(--text-1)]'
						: isPast
							? 'text-[var(--text-1)] opacity-60 hover:opacity-100'
							: expanded
								? 'scale-[0.97] text-[var(--text-1)] opacity-90'
								: 'text-[var(--text-1)]'}
				{#if line.time_ms !== undefined}
					<button
						data-line={i}
						onclick={() => seekTo(line)}
						title={t('lyrics.seek_to_line')}
						class="lyric-line block w-full origin-left cursor-pointer text-left font-heading font-bold leading-snug transition-[color,opacity,transform] {sizeCls} {stateCls}"
						style="transition-duration:var(--dur-3);transition-timing-function:var(--ease-out)"
					>
						{@render lineBody(line, i, isActive, isPast, isTrailer)}
					</button>
				{:else}
					<!-- Untimed line: not a button — no pointer cursor, no seek. -->
					<div
						data-line={i}
						class="lyric-line block w-full origin-left text-left font-heading font-bold leading-snug transition-[color,opacity,transform] {sizeCls} {stateCls}"
						style="transition-duration:var(--dur-3);transition-timing-function:var(--ease-out)"
					>
						{@render lineBody(line, i, false, false, false)}
					</div>
				{/if}
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
				{#if line.text?.trim()}
					<div class={sing ? 'text-2xl md:text-3xl' : ''}>
						<p class={romanOn ? 'lyric-romaji' : ''}>{transMode === 'only' ? (transText(i) ?? segText(i, -1, line.text)) : segText(i, -1, line.text)}{#if romanOn && transMode !== 'only' && romanPartial[i]}<span class="roman-partial" title={t('lyrics.partial_romaji')}>※</span>{/if}</p>
						{#if transMode === 'both' && transText(i)}
							<p class="lyric-trans mt-0.5 opacity-60">{transText(i)}</p>
						{/if}
					</div>
				{:else}
					<div class="py-1">{@render breathe()}</div>
				{/if}
			{/each}
			</div>
		{:else}
			<div class="flex flex-col items-center gap-3 py-8 text-center">
				<p class="text-sm text-muted-foreground">{t('lyrics.none_found')}</p>
				{#if playback.now && !api.isLocalId(playback.now.videoId)}
					<button
						class="flex cursor-pointer items-center gap-1.5 rounded-lg border px-2.5 py-1.5 text-xs font-medium text-muted-foreground transition-colors hover:border-foreground/20 hover:bg-muted hover:text-foreground"
						onclick={attachLyricsFile}
						title={t('lyrics.import_lrc_title')}
					>
						<HugeiconsIcon icon={Attachment01Icon} class="h-3.5 w-3.5" /> {t('lyrics.import_lrc')}
					</button>
					<a
						class="text-xs font-medium text-primary hover:underline"
						href={`/lyrics/compose?videoId=${encodeURIComponent(playback.now.videoId)}`}
					>
						{t('lyrics.time_yourself')}
					</a>
				{/if}
			</div>
		{/if}
</div>
	{#if autoPaused && lyrics?.synced && !loading}
		<!-- Resume-autoscroll pill: a manual wheel/touch pauses autoscroll until tapped (no
		     timed auto-resume). Jumps (track change/seek) clear the pause on their own. -->
		<button
			onclick={resumeAuto}
			class="absolute bottom-3 left-1/2 z-10 -translate-x-1/2 cursor-pointer rounded-[var(--r-full)] px-3 py-1.5 text-xs font-semibold shadow-lg transition-transform hover:scale-105 active:scale-95"
			style="background:var(--surface-1);color:var(--text-1);border:1px solid var(--border);transition-duration:var(--dur-2);transition-timing-function:var(--ease-out)"
		>
			{t('lyrics.resume_autoscroll')}
		</button>
	{/if}
</div>
{#if !sing}
	<!-- Footer row: suppressed in sing mode. Source left, controls right; compact/mini shows
	     the source only. Controls stay disabled-with-tooltip when N/A. -->
	{@const canTranslate = !!lyrics && !loading}
	<div class="flex items-center gap-2 border-t px-4 py-2 text-xs text-muted-foreground">
		<span
			class="min-w-0 flex-1 truncate"
			title={loading ? t('lyrics.loading') : lyrics ? lyrics.source : t('lyrics.no_lyrics_short')}
		>
			{loading
				? t('lyrics.loading')
				: lyrics
					? lyrics.source.startsWith('Source:')
						? lyrics.source
						: t('lyrics.from_source', { source: lyrics.source })
					: t('lyrics.no_lyrics_short')}
		</span>
		{#if !compact}
		<div class="flex shrink-0 items-center gap-1.5">
		{#if playback.now}
			<!-- Timing offset: −/+ nudge ±100ms per tap, value click resets to 0. Display updates
			     immediately; the write is debounced and the shifted cues refetch from the server. -->
			<div
				class="flex shrink-0 items-center gap-0.5 rounded-full border px-1 py-0.5 tabular-nums"
				title={t('lyrics.offset_title')}
			>
				<button
					class="cursor-pointer rounded-full px-1.5 font-semibold transition-colors hover:text-foreground disabled:cursor-not-allowed disabled:opacity-40"
					onclick={() => nudgeOffset(-100)}
					disabled={loading || !playback.now}
					title={t('lyrics.offset_down')}
					aria-label={t('lyrics.offset_down')}
				>
					−
				</button>
				<button
					class="min-w-14 cursor-pointer text-center font-semibold transition-colors hover:text-foreground disabled:cursor-not-allowed disabled:opacity-40"
					onclick={resetOffset}
					disabled={loading || !playback.now}
					title={t('lyrics.offset_reset')}
					aria-label={t('lyrics.offset_reset')}
				>
					{fmtOffset(lyricOffsetMs)}
				</button>
				<button
					class="cursor-pointer rounded-full px-1.5 font-semibold transition-colors hover:text-foreground disabled:cursor-not-allowed disabled:opacity-40"
					onclick={() => nudgeOffset(100)}
					disabled={loading || !playback.now}
					title={t('lyrics.offset_up')}
					aria-label={t('lyrics.offset_up')}
				>
					+
				</button>
			</div>
		{/if}
		{#if hasKana}
			<button
				class="shrink-0 cursor-pointer rounded-full border px-2 py-0.5 font-semibold tracking-wide uppercase transition-colors disabled:cursor-not-allowed disabled:opacity-40 {romanOn
					? 'border-primary/60 text-primary'
					: 'hover:border-foreground/20 hover:text-foreground'}"
				onclick={toggleRoman}
				disabled={romanBusy || !lyrics}
				title={lyrics ? t('lyrics.romaji_show') : t('lyrics.romaji_needs_lyrics')}
				aria-pressed={romanOn}
			>
				{romanBusy ? '…' : romanOn ? 'かな' : t('lyrics.romaji')}
			</button>
		{:else}
			<button
				class="shrink-0 cursor-not-allowed rounded-full border px-2 py-0.5 font-semibold tracking-wide uppercase opacity-40"
				disabled
				title={t('lyrics.romaji_no_kana')}
			>
				{t('lyrics.romaji')}
			</button>
		{/if}
		<button
			class="shrink-0 cursor-pointer rounded-full border px-2 py-0.5 font-semibold tracking-wide uppercase transition-colors disabled:cursor-not-allowed disabled:opacity-40 {transMode !== 'off'
				? 'border-primary/60 text-primary'
				: 'hover:border-foreground/20 hover:text-foreground'}"
			onclick={cycleTrans}
			disabled={transBusy || !canTranslate}
			title={canTranslate ? t('lyrics.translate_cycle') : t('lyrics.translate_needs_lyrics')}
			aria-pressed={transMode !== 'off'}
		>
			{transBusy ? '…' : transMode === 'off' ? t('lyrics.translate') : transMode === 'both' ? t('lyrics.both') : t('lyrics.translated')}
		</button>
		{#if transMode !== 'off'}
			<select
				class="shrink-0 cursor-pointer rounded-full border bg-transparent px-1.5 py-0.5 text-[11px] disabled:cursor-not-allowed disabled:opacity-40"
				value={transLang || defaultTransLang()}
				aria-label={t('lyrics.translate_lang')}
				disabled={!canTranslate}
				title={canTranslate ? t('lyrics.translate_lang') : t('lyrics.translate_needs_lyrics')}
				onchange={(e) => {
					transLang = e.currentTarget.value;
					transSeg = null;
					transFor = '';
					void setTransMode(transMode);
				}}
			>
				{#each TRANS_LANGS as [code, label]}
					<option value={code}>{label}</option>
				{/each}
			</select>
		{/if}
		{#if lyrics?.source === 'Custom file'}
			<button
				class="ml-auto shrink-0 cursor-pointer underline-offset-2 hover:underline"
				onclick={removeAttachedLyrics}
				title={t('lyrics.remove_attached_title')}
			>
				{t('common.remove')}
			</button>
		{/if}
		</div>
		{/if}
	</div>
{/if}

{#if sing}
	<div class="absolute top-4 left-6 z-20 sm:left-14"></div>
{/if}

<style>
@keyframes karaoke-breathe {
	0%, 100% { opacity: 0.35; transform: translateY(0); }
	50% { opacity: 1; transform: translateY(-2px); }
}
/* Karaoke + translation type, all on concrete tokens (never var(--hue) without a fallback,
   which left the unsung half unpainted on themes without it). Translation and romaji sizes
   are independent vars so each can be tuned without moving the other. */
.lyrics-scroller {
	--lyric-trans-size: 0.62em;
	--lyric-romaji-size: 0.96em;
}
.lyric-trans {
	font-size: var(--lyric-trans-size);
}
.lyric-romaji {
	font-size: var(--lyric-romaji-size);
}
.lyric-sung {
	color: var(--primary);
}
.lyric-unsung {
	color: color-mix(in srgb, var(--primary) 26%, var(--text-3));
}
.lyric-past {
	color: var(--text-4);
}
.lyric-line {
	border-radius: var(--r-md);
}
/* Partial-transliteration mark: one faint reference glyph per mixed line, nothing else. */
.roman-partial {
	opacity: 0.35;
	font-size: 0.72em;
	margin-inline-start: 0.35em;
}
</style>
