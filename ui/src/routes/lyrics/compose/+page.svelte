<script lang="ts">
	// Lyrics Composer: tap-along timing editor. Paste LRC or plain text (or load the fetched
	// lyrics as a starting point), press TAP (or Space) on each line as it plays, then save —
	// stored as that track's custom lyrics. Line-level timing only: word timings can't be
	// tapped reliably, so they're never fabricated. Drafts autosave to localStorage every 2s.
	import { onDestroy, onMount } from 'svelte';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { PlayIcon, PauseIcon, Delete01Icon, CheckmarkCircle02Icon } from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import type { SongItem } from '$lib/api';
	import { playback, playSong, toast } from '$lib/player.svelte';
	import { t } from '$lib/i18n.svelte';

	type Line = { text: string; ms: number | null };

	const SPEEDS = [0.5, 0.75, 1, 1.25, 1.5];
	const DRAFT_KEY = (id: string) => `lrc-draft:${id}`;

	let videoId = $derived(page.url.searchParams.get('videoId') ?? '');
	let track = $state<SongItem | null>(null);
	let lines = $state<Line[]>([]);
	let paste = $state('');
	let speed = $state(1);
	let dirty = $state(false);
	let saving = $state(false);
	let draftTimer: ReturnType<typeof setInterval> | null = null;

	// Live clock (rAF-interpolated like LyricsView) for tap precision. Rebased from every
	// position tick — never dead-reckoned — so seeks and pauses can't leave it drifting, plus a
	// visibility resync for background-throttled tabs.
	let nowMs = $state(0);
	$effect(() => {
		const pos = playback.position * 1000;
		if (playback.paused) {
			nowMs = pos;
			return;
		}
		const base = pos;
		const baseAt = performance.now();
		nowMs = pos;
		let id = requestAnimationFrame(function tick() {
			nowMs = base + (performance.now() - baseAt);
			id = requestAnimationFrame(tick);
		});
		return () => cancelAnimationFrame(id);
	});
	function resyncClock() {
		if (document.visibilityState === 'visible') nowMs = playback.position * 1000;
	}

	// Exits back where the composer was opened from. history.back() strands the user on a blank
	// view when there is no entry (deep link / fresh window), so fall back to the queue, then /.
	function exitCompose() {
		if (window.history.length > 1) history.back();
		else goto('/queue').catch(() => goto('/'));
	}

	function fmt(ms: number | null): string {
		if (ms === null || ms < 0) return '--:--.--';
		const cs = Math.floor(ms / 10);
		return `${String(Math.floor(cs / 6000)).padStart(2, '0')}:${String(Math.floor((cs / 100) % 60)).padStart(2, '0')}.${String(cs % 100).padStart(2, '0')}`;
	}
	function toLrc(): string {
		return lines
			.filter((l) => l.text.trim())
			.map((l) => (l.ms === null ? l.text.trim() : `[${fmt(l.ms)}]${l.text.trim()}`))
			.join('\n');
	}
	function parseInput(text: string): Line[] {
		const out: Line[] = [];
		for (const raw of text.split('\n')) {
			const line = raw.trim();
			if (!line) continue;
			const m = line.match(/^\[(\d+):(\d+(?:\.\d+)?)\]\s*(.*)$/);
			if (m && m[3]) {
				const ms = Number(m[1]) * 60000 + Number(m[2]) * 1000;
				if (Number.isFinite(ms)) {
					out.push({ text: m[3], ms: Math.round(ms) });
					continue;
				}
			}
			if (/^\[\w+:/.test(line)) continue; // LRC tags ([ar:], [ti:], …)
			out.push({ text: line, ms: null });
		}
		return out;
	}

	function loadText() {
		const parsed = parseInput(paste);
		if (!parsed.length) {
			toast.error(t('lyrics.compose.nothing_usable'));
			return;
		}
		lines = parsed;
		dirty = true;
		paste = '';
	}
	async function loadFetched() {
		if (!track) return;
		try {
			const l = await api.getLyrics({
				videoId: track.video_id,
				title: track.title,
				artists: track.artists
			});
			if (!l || !l.lines.length) {
				toast.info(t('lyrics.compose.no_fetched'));
				return;
			}
			lines = l.lines.map((x) => ({ text: x.text, ms: null }));
			dirty = true;
			toast.success(t('lyrics.compose.loaded_fetched'));
		} catch (e) {
			toast.error(String(e));
		}
	}

	const nextUntimed = $derived(lines.findIndex((l) => l.ms === null));
	const timedCount = $derived(lines.filter((l) => l.ms !== null).length);
	const isCurrent = $derived(playback.now?.videoId === videoId);

	function tap() {
		if (!isCurrent) return;
		const i = lines.findIndex((l) => l.ms === null);
		if (i < 0) return;
		lines[i].ms = Math.round(nowMs);
		dirty = true;
	}
	function restamp(i: number) {
		lines[i].ms = Math.round(nowMs);
		dirty = true;
	}
	function nudge(i: number, delta: number) {
		if (lines[i].ms === null) return;
		lines[i].ms = Math.max(0, (lines[i].ms as number) + delta);
		dirty = true;
	}
	function removeLine(i: number) {
		lines.splice(i, 1);
		dirty = true;
	}
	function seekTo(ms: number | null) {
		if (ms === null) return;
		api.seek(ms / 1000).catch(() => {});
	}

	async function setSpeed(v: number) {
		speed = v;
		try {
			await api.setPlaybackRate(v);
		} catch (e) {
			toast.error(String(e));
		}
	}

	async function save() {
		if (!track || saving) return;
		if (!lines.some((l) => l.text.trim() && l.ms !== null)) {
			toast.error(t('lyrics.compose.time_first'));
			return;
		}
		saving = true;
		try {
			await api.setCustomLyrics(track.video_id, toLrc());
			try {
				localStorage.removeItem(DRAFT_KEY(track.video_id));
			} catch {
				/* quota */
			}
			await api.setPlaybackRate(1).catch(() => {});
			toast.success(t('lyrics.compose.saved'));
			exitCompose();
		} catch (e) {
			toast.error(String(e));
		} finally {
			saving = false;
		}
	}

	function onKey(e: KeyboardEvent) {
		const t = e.target as HTMLElement | null;
		if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.tagName === 'SELECT')) return;
		if (e.code === 'Space') {
			e.preventDefault();
			tap();
		}
	}

	onMount(() => {
		if (!videoId) {
			toast.error(t('lyrics.compose.no_track'));
			exitCompose();
			return;
		}
		const fromQueue = playback.queue.items.find((s) => s.video_id === videoId);
		track =
			fromQueue ??
			(playback.now?.videoId === videoId
				? {
						video_id: playback.now.videoId,
						title: playback.now.title,
						artists: playback.now.artists,
						thumbnail: playback.now.thumbnail
					}
				: null);
		if (!track) {
			toast.error(t('lyrics.compose.track_gone'));
			exitCompose();
			return;
		}
		// Restore an autosaved draft, if any.
		try {
			const raw = localStorage.getItem(DRAFT_KEY(videoId));
			if (raw) {
				const d = JSON.parse(raw) as Line[];
				if (Array.isArray(d) && d.length) lines = d;
			}
		} catch {
			/* corrupt draft — start clean */
		}
		window.addEventListener('keydown', onKey);
		document.addEventListener('visibilitychange', resyncClock);
		draftTimer = setInterval(() => {
			if (!dirty || !track) return;
			try {
				localStorage.setItem(DRAFT_KEY(track.video_id), JSON.stringify(lines));
				dirty = false;
			} catch {
				/* quota */
			}
		}, 2000);
	});

	onDestroy(() => {
		window.removeEventListener('keydown', onKey);
		document.removeEventListener('visibilitychange', resyncClock);
		if (draftTimer) clearInterval(draftTimer);
		// Never leak a slowed rate into normal listening.
		void api.setPlaybackRate(1).catch(() => {});
	});
</script>

<div class="mx-auto flex min-h-0 w-full max-w-2xl flex-1 flex-col gap-4 px-4 py-6">
	<div class="flex items-center gap-3">
		<div class="min-w-0 flex-1">
			<h1 class="font-heading text-xl font-bold">{t('lyrics.compose.title')}</h1>
			<p class="truncate text-xs text-muted-foreground">
				{track ? `${track.title} — ${track.artists}` : '…'}
			</p>
		</div>
		<label class="flex shrink-0 items-center gap-1.5 text-xs text-muted-foreground">
			{t('lyrics.compose.speed')}
			<select
				class="cursor-pointer rounded-lg border bg-transparent px-2 py-1"
				value={speed}
				aria-label={t('lyrics.compose.speed_label')}
				onchange={(e) => setSpeed(Number(e.currentTarget.value))}
			>
				{#each SPEEDS as s}
					<option value={s}>{s}×</option>
				{/each}
			</select>
		</label>
		<button
			class="flex h-10 w-10 shrink-0 cursor-pointer items-center justify-center rounded-full bg-primary text-primary-foreground"
			onclick={() => api.togglePause().catch(() => {})}
			aria-label={playback.paused ? t('player.play') : t('player.pause')}
		>
			<HugeiconsIcon icon={playback.paused ? PlayIcon : PauseIcon} class="h-5 w-5" />
		</button>
	</div>

	<div class="flex items-center gap-2 rounded-xl border border-border bg-card px-3 py-2">
		<span class="text-xs tabular-nums text-muted-foreground">{fmt(Math.round(nowMs))}</span>
		{#if track && !isCurrent}
			<button
				class="flex-1 cursor-pointer rounded-lg border border-primary/50 px-3 py-2.5 text-sm font-bold text-primary transition-transform active:scale-[0.98]"
				onclick={() => playSong(track!)}
			>
				{t('lyrics.compose.play_to_start', { title: track.title })}
			</button>
		{:else}
			<button
				class="flex-1 cursor-pointer rounded-lg bg-primary py-2.5 text-sm font-bold text-primary-foreground transition-transform active:scale-[0.98] disabled:opacity-40"
				disabled={nextUntimed < 0}
				onclick={tap}
				title={t('lyrics.compose.tap_hint')}
			>
				{nextUntimed < 0
					? t('lyrics.compose.all_timed')
					: t('lyrics.compose.tap', { current: nextUntimed + 1, total: lines.length })}
			</button>
		{/if}
		<span class="text-xs tabular-nums text-muted-foreground">{timedCount}/{lines.length}</span>
	</div>

	{#if !lines.length}
		<div class="flex flex-col gap-2">
			<textarea
				bind:value={paste}
				rows={6}
				placeholder={t('lyrics.compose.paste_placeholder')}
				class="w-full rounded-xl border bg-transparent p-3 text-sm outline-none placeholder:text-muted-foreground/60 focus:border-primary/50"
			></textarea>
			<div class="flex gap-2">
				<button
					class="flex-1 cursor-pointer rounded-lg border px-3 py-2 text-sm font-medium transition-colors hover:bg-accent/10"
					onclick={loadText}
				>
					{t('lyrics.compose.load_text')}
				</button>
				<button
					class="flex-1 cursor-pointer rounded-lg border px-3 py-2 text-sm font-medium transition-colors hover:bg-accent/10"
					onclick={loadFetched}
				>
					{t('lyrics.compose.load_fetched')}
				</button>
			</div>
		</div>
	{:else}
		<div class="flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto pb-2">
			{#each lines as line, i (i)}
				<div
					class="flex items-center gap-2 rounded-lg px-2 py-1.5 transition-colors {line.ms === null
						? 'bg-muted/40'
						: 'hover:bg-accent/10'}"
				>
					<button
						class="w-20 shrink-0 cursor-pointer rounded-md px-1 py-1 text-left text-xs tabular-nums {line.ms === null
							? 'text-muted-foreground/50'
							: 'text-primary'}"
						title={line.ms === null ? t('lyrics.compose.untimed_hint') : t('lyrics.compose.seek_hint')}
						onclick={() => (line.ms === null ? restamp(i) : seekTo(line.ms))}
					>
						{fmt(line.ms)}
					</button>
					<input
						bind:value={line.text}
						oninput={() => (dirty = true)}
						class="min-w-0 flex-1 bg-transparent text-sm outline-none"
						aria-label={t('lyrics.compose.line_text', { n: i + 1 })}
					/>
					<button
						class="shrink-0 cursor-pointer rounded px-1 text-[11px] text-muted-foreground hover:text-foreground"
						title={t('lyrics.compose.nudge_down')}
						onclick={() => nudge(i, -500)}
					>
						−0.5
					</button>
					<button
						class="shrink-0 cursor-pointer rounded px-1 text-[11px] text-muted-foreground hover:text-foreground"
						title={t('lyrics.compose.nudge_up')}
						onclick={() => nudge(i, 500)}
					>
						+0.5
					</button>
					<button
						class="flex h-6 w-6 shrink-0 cursor-pointer items-center justify-center rounded text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
						onclick={() => removeLine(i)}
						aria-label={t('lyrics.compose.delete_line', { n: i + 1 })}
					>
						<HugeiconsIcon icon={Delete01Icon} class="h-3.5 w-3.5" />
					</button>
				</div>
			{/each}
		</div>
		<div class="flex gap-2">
			<button
				class="cursor-pointer rounded-lg border px-3 py-2 text-sm font-medium text-muted-foreground transition-colors hover:text-foreground"
				onclick={() => {
					paste = '';
					lines = [];
				}}
			>
				{t('lyrics.compose.start_over')}
			</button>
			<button
				class="flex flex-1 cursor-pointer items-center justify-center gap-2 rounded-lg bg-primary py-2.5 text-sm font-bold text-primary-foreground transition-transform active:scale-[0.98] disabled:opacity-40"
				disabled={saving || timedCount === 0}
				onclick={save}
			>
				<HugeiconsIcon icon={CheckmarkCircle02Icon} class="h-4 w-4" />
				{saving
					? t('lyrics.compose.saving')
					: timedCount === 1
						? t('lyrics.compose.save_one')
						: t('lyrics.compose.save_many', { count: timedCount })}
			</button>
		</div>
	{/if}
</div>
