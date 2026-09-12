<!--
  Theater mode: the window goes fullscreen and the app is replaced by one thing — the cover and
  the controls on the left, the lyrics on the right.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { beforeNavigate } from '$app/navigation';
	import { fade, fly, scale } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Cancel01Icon,
		FavouriteIcon,
		Mic01Icon,
		MusicNote01Icon,
		NextIcon,
		PauseIcon,
		PlayIcon,
		PreviousIcon,
		RepeatIcon,
		RepeatOne01Icon,
		ShuffleIcon,
		VolumeHighIcon,
		VolumeMute02Icon
	} from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import {
		commitVolume,
		dragVolume,
		playback,
		toggleMute,
		ui,
		toggleNowPlayingLike
	} from '$lib/player.svelte';
	import { artworkAccent } from '$lib/artcolor';
	import { hexToHsv } from '$lib/color';
	import { t } from '$lib/i18n.svelte';
	import { appearance } from '$lib/theme.svelte';
	import { thumb } from '$lib/thumb';
	import LyricsView from './LyricsView.svelte';

	const close = () => (ui.theaterOpen = false);

	beforeNavigate(close);

	onMount(() => {
		api.theaterFullscreen(true).catch((e) => console.error('theater fullscreen failed', e));
		return () => {
			api.theaterFullscreen(false).catch(() => {});
		};
	});

	function onKey(e: KeyboardEvent) {
		if (e.defaultPrevented) return;
		if (e.key === 'Escape') {
			e.preventDefault();
			close();
		}
	}

	let idle = $state(false);
	let idleTimer: ReturnType<typeof setTimeout>;
	function wake() {
		idle = false;
		clearTimeout(idleTimer);
		idleTimer = setTimeout(() => (idle = true), 3500);
	}
	onMount(() => {
		wake();
		return () => clearTimeout(idleTimer);
	});

	let attempt = $state(0);
	$effect(() => {
		playback.now?.thumbnail;
		attempt = 0;
	});
	const srcs = $derived([720, 400, 120].map((px) => thumb(playback.now?.thumbnail, px)));
	const src = $derived(srcs[attempt]);

	let accent = $state<string | null>(null);
	$effect(() => {
		const url = thumb(playback.now?.thumbnail, 120);
		if (!url) {
			accent = null;
			return;
		}
		let alive = true;
		artworkAccent(url).then((hex) => {
			if (alive) accent = hex;
		});
		return () => {
			alive = false;
		};
	});

	const MAX_WASHES = 8;
	const WASH = 160;
	const WASH_BLUR = 28;
	const washes = new Map<string, string>();
	let wash = $state<string | null>(null);
	$effect(() => {
		const url = thumb(playback.now?.thumbnail, 400);
		if (!url) {
			wash = null;
			return;
		}
		// Cache key includes the bake size: changing WASH must not serve stale thumbnails
		// baked at the old resolution.
		const key = `${url}@${WASH}`;
		const hit = washes.get(key);
		if (hit !== undefined) {
			wash = hit;
			return;
		}
		let alive = true;
		bake(url).then((data) => {
			if (!alive || !data) return;
			if (washes.size >= MAX_WASHES && !washes.has(key)) {
				const oldest = washes.keys().next().value;
				if (oldest !== undefined) washes.delete(oldest);
			}
			washes.set(key, data);
			wash = data;
		});
		return () => {
			alive = false;
		};
	});

	async function bake(url: string): Promise<string | null> {
		try {
			const img = new Image();
			img.crossOrigin = 'anonymous';
			img.src = url;
			await img.decode();
			const canvas = document.createElement('canvas');
			canvas.width = canvas.height = WASH;
			const ctx = canvas.getContext('2d');
			if (!ctx) return null;
			ctx.imageSmoothingQuality = 'high';
			const over = WASH_BLUR * 1.6;
			const canFilter = typeof ctx.filter === 'string';
			if (canFilter) {
				ctx.filter = `blur(${WASH_BLUR}px) saturate(1.5)`;
				ctx.drawImage(img, -over, -over, WASH + over * 2, WASH + over * 2);
			} else {
				const small = document.createElement('canvas');
				small.width = small.height = 20;
				small.getContext('2d')?.drawImage(img, 0, 0, 20, 20);
				ctx.drawImage(small, -over, -over, WASH + over * 2, WASH + over * 2);
			}
			return canvas.toDataURL('image/png');
		} catch {
			return null;
		}
	}

	const hue = $derived(accent ? (hexToHsv(accent)?.h ?? null) : null);
	const mesh = $derived.by(() => {
		const h = hue ?? 265;
		const a = (deg: number) => (h + deg + 360) % 360;
		return [
			`radial-gradient(70% 60% at 12% 18%, hsl(${a(0)} 72% 48% / 0.34), transparent 68%)`,
			`radial-gradient(60% 55% at 88% 82%, hsl(${a(38)} 70% 45% / 0.28), transparent 68%)`,
			`radial-gradient(55% 50% at 72% 8%, hsl(${a(-46)} 65% 52% / 0.2), transparent 70%)`
		].join(',');
	});
	const glow = $derived(`radial-gradient(closest-side, hsl(${hue ?? 265} 80% 55% / 0.5), transparent)`);

	const fmt = (secs: number) => {
		if (!secs || secs < 0) return '0:00';
		const s = Math.floor(secs);
		const h = Math.floor(s / 3600);
		const m = Math.floor((s % 3600) / 60);
		const mm = h ? String(m).padStart(2, '0') : `${m}`;
		return `${h ? `${h}:` : ''}${mm}:${String(s % 60).padStart(2, '0')}`;
	};

	let seekDrag = $state<number | null>(null);
	const shownPosition = $derived(seekDrag ?? playback.position);
	const pct = $derived(playback.duration ? (shownPosition / playback.duration) * 100 : 0);

	const shuffleOn = $derived(playback.queue.shuffle ?? false);
	const repeat = $derived(playback.queue.repeat ?? 'off');
	const local = $derived(!!playback.now && api.isLocalId(playback.now.videoId));
	const album = $derived.by(() => {
		const cur = playback.queue.items[playback.queue.currentIndex];
		return cur?.video_id === playback.now?.videoId ? cur?.album : null;
	});

	let volHover = $state(false);
	let volDragging = $state(false);
	const volOpen = $derived(volHover || volDragging);

	// Wheel over the lyrics column scrolls it (the column hides its own scrollbar). A native
	// non-passive listener: Svelte's `onwheel` can't take `{ passive: false }`, and an always-
	// preventDefault wheel would trap page scroll. Gestures starting on sliders/buttons are
	// ignored, and preventDefault only fires when the lyrics actually consume the scroll.
	let theaterRoot: HTMLElement | undefined = $state();
	$effect(() => {
		const root = theaterRoot;
		if (!root) return;
		const onWheel = (e: WheelEvent) => {
			const target = e.target as HTMLElement | null;
			if (target?.closest?.('input, select, textarea, button, [role="button"], a')) return;
			const host = root.querySelector('[data-theater-lyrics]') as HTMLElement | null;
			const scroller =
				(host?.querySelector('.lyrics-scroller') as HTMLElement | null) ?? host;
			if (!scroller) return;
			const delta = e.deltaY ?? 0;
			if (!delta) return;
			const canDown =
				scroller.scrollTop + scroller.clientHeight < scroller.scrollHeight - 1;
			const canUp = scroller.scrollTop > 0;
			if ((delta > 0 && !canDown) || (delta < 0 && !canUp)) return;
			e.preventDefault();
			scroller.scrollBy({ top: delta > 0 ? 120 : -120, behavior: 'smooth' });
		};
		root.addEventListener('wheel', onWheel, { passive: false });
		return () => root.removeEventListener('wheel', onWheel);
	});

	let showLyrics = $state(true);

	let justLiked = $state(false);
	function toggleLike() {
		if (!playback.liked) justLiked = true;
		toggleNowPlayingLike();
	}

	function toggleRepeat() {
		const next = repeat === 'off' ? 'all' : repeat === 'all' ? 'one' : 'off';
		api.setRepeat(next);
	}

	const IconMap = {
		off: RepeatIcon,
		all: RepeatIcon,
		one: RepeatOne01Icon
	};
</script>

<svelte:window onkeydown={onKey} onpointerup={() => (volDragging = false)} />

<section
	transition:fade={{ duration: 220 }}
	bind:this={theaterRoot}
	onpointermove={wake}
	class="theater fixed inset-0 z-40 flex flex-col overflow-hidden bg-background text-foreground {idle
		? 'cursor-none'
		: ''}"
>
	{#if appearance.artworkBackground && wash}
		{#key wash}
			<div
				in:fade={{ duration: 700 }}
				style="background-image:url({wash});background-size:100% 100%"
				class="pointer-events-none absolute inset-0 opacity-50 dark:opacity-60"
			></div>
		{/key}
	{/if}
	<div class="pointer-events-none absolute -inset-[15%]" style="background-image:{mesh}"></div>
	<div class="pointer-events-none absolute inset-x-0 bottom-0 h-40 bg-gradient-to-t from-background to-transparent"></div>

	<header
		class="relative z-10 flex shrink-0 items-center justify-between px-8 py-5 transition-opacity duration-500 xl:px-14 {idle
			? 'opacity-0'
			: 'opacity-100'}"
	>
		<div class="min-w-0">
			<p class="text-[var(--text-caption)] font-semibold uppercase tracking-[0.22em] text-muted-foreground">
				{playback.queue.sourceName ? 'Playing from' : 'Now playing'}
			</p>
			{#if playback.queue.sourceName}
				<p class="mt-1 truncate text-sm font-medium">{playback.queue.sourceName}</p>
			{/if}
		</div>
		<button
			onclick={close}
			class="flex h-10 w-10 cursor-pointer items-center justify-center rounded-full border border-border/50 bg-card/70 text-muted-foreground transition-colors hover:bg-card hover:text-foreground"
			title="Exit theater (Esc)"
			aria-label="Exit theater"
		>
			<HugeiconsIcon strokeWidth={2} icon={Cancel01Icon} class="h-4 w-4" />
		</button>
	</header>

	<div
		class="relative z-10 mx-auto grid min-h-0 w-full max-w-[104rem] flex-1 grid-rows-[minmax(0,1fr)] gap-10 px-8 pb-10 xl:gap-20 xl:px-14 {showLyrics
			? 'lg:grid-cols-[minmax(20rem,0.85fr)_minmax(0,1.15fr)]'
			: ''}"
	>
		<div
			class="mx-auto w-full self-center {showLyrics ? 'max-w-[30rem]' : 'max-w-[34rem]'}"
			style="--art:min(100%, 100vh - 25rem)"
		>
			<div class="relative mx-auto" style="width:var(--art);max-width:100%">
				<div
					class="pointer-events-none absolute -inset-[12%] -z-10 opacity-70"
					style="background-image:{glow}"
				></div>
				{#key playback.now?.videoId}
					<div in:scale={{ start: 0.94, duration: 420, easing: cubicOut }} class="relative">
						{#if src && attempt < srcs.length}
							<img decoding="async"
								{src}
								alt={playback.now?.title ? `${playback.now.title}${playback.now.artists ? ` by ${playback.now.artists}` : ''}` : t('player.now_playing')}
								onerror={() => attempt++}
								style={srcs[2] ? `background-image:url(${srcs[2]})` : undefined}
								class="aspect-square w-full rounded-[var(--r-2xl)] bg-cover object-cover ring-1 ring-[color-mix(in_srgb,var(--on-art)_10%,transparent)]"
							/>
						{:else}
							<div
								class="flex aspect-square w-full items-center justify-center rounded-[var(--r-2xl)] bg-muted text-muted-foreground/40 ring-1 ring-[color-mix(in_srgb,var(--on-art)_10%,transparent)]"
							>
								<HugeiconsIcon strokeWidth={2} icon={MusicNote01Icon} class="h-20 w-20" />
							</div>
						{/if}
						<div
							class="pointer-events-none absolute inset-0 rounded-[var(--r-2xl)] ring-1 ring-inset ring-[color-mix(in_srgb,var(--on-art)_10%,transparent)]"
						></div>
					</div>
				{/key}

				<div
					class="absolute left-3 top-3 z-10 flex items-center rounded-[var(--r-full)] bg-[color-mix(in_srgb,var(--scrim)_40%,transparent)] px-1.5 py-1 text-[var(--on-art)]"
					role="group"
					aria-label="Volume"
					onpointerenter={() => (volHover = true)}
					onpointerleave={() => (volHover = false)}
				>
					<button
						class="flex size-6 shrink-0 cursor-pointer items-center justify-center rounded-[var(--r-full)] text-[color-mix(in_srgb,var(--on-art)_75%,transparent)] transition-colors hover:text-[var(--on-art)]"
						onclick={() => toggleMute()}
						aria-label={playback.volume === 0 ? t('player.unmute') : t('player.mute')}
					>
						<HugeiconsIcon strokeWidth={2}
							icon={VolumeHighIcon}
							altIcon={VolumeMute02Icon}
							showAlt={playback.volume === 0}
							class="h-4 w-4"
						/>
					</button>
					<input
						type="range"
						class="range on-art min-w-0 transition-[width,opacity,margin] duration-150 {volOpen
							? 'ml-1.5 mr-1 w-24 opacity-100'
							: 'w-0 opacity-0'}"
						style="--pct:{playback.volume}%"
						min="0"
						max="100"
						value={playback.volume}
						onpointerdown={() => (volDragging = true)}
						oninput={(e) => dragVolume(Number(e.currentTarget.value))}
						onchange={(e) => commitVolume(Number(e.currentTarget.value))}
						aria-label={t('player.volume')}
					/>
				</div>
			</div>

			<div class="mt-8 flex items-start gap-4">
				<div class="min-w-0 flex-1">
					<h1
						class="truncate font-heading text-[var(--text-display)] font-bold leading-tight tracking-tight xl:text-4xl"
						title={playback.now?.title}
					>
						{playback.now?.title ?? 'Not playing'}
					</h1>
					<p class="mt-2 truncate text-base text-foreground/70">
						{playback.now?.artists ?? ''}
					</p>
					{#if album}
						<p class="mt-0.5 truncate text-[var(--text-caption)] text-muted-foreground">{album}</p>
					{/if}
				</div>
				<div class="flex shrink-0 items-center gap-1 pt-1.5">
					<button
						onclick={() => (showLyrics = !showLyrics)}
						class="hidden h-10 w-10 cursor-pointer items-center justify-center rounded-full transition-colors hover:bg-foreground/10 lg:flex {showLyrics
							? 'text-primary'
							: 'text-muted-foreground hover:text-foreground'}"
						aria-label={t('player.lyrics')}
						aria-pressed={showLyrics}
						title="Lyrics"
					>
						<HugeiconsIcon strokeWidth={2} icon={Mic01Icon} class="h-4 w-4" />
					</button>
					{#if playback.now && !local}
						<button
							onclick={toggleLike}
							class="flex h-10 w-10 cursor-pointer items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-foreground/10 hover:text-foreground"
							aria-label={playback.liked ? t('player.remove_from_liked') : t('player.save_to_liked')}
						>
							<span
								class="inline-flex"
								class:animate-heart-pop={justLiked}
								onanimationend={() => (justLiked = false)}
							>
								<HugeiconsIcon strokeWidth={2}
									icon={FavouriteIcon}
									class="h-4 w-4 {playback.liked
										? 'fill-current text-primary'
										: ''}"
								/>
							</span>
						</button>
					{/if}
				</div>
			</div>

			<div class="mt-7">
				<input
					type="range"
					class="range theater-range w-full"
					style="--pct:{pct}%"
					min="0"
					max={playback.duration || 0}
					value={shownPosition}
					oninput={(e) => (seekDrag = Number(e.currentTarget.value))}
					onchange={(e) => {
						const v = Number(e.currentTarget.value);
						playback.position = v;
						seekDrag = null;
						api.seek(v);
					}}
					aria-label={t('player.seek')}
				/>
				<div class="timer mt-2 flex justify-between text-xs font-medium text-muted-foreground" data-timer>
					<span>{fmt(shownPosition)}</span>
					<span>{fmt(playback.duration)}</span>
				</div>
			</div>

			<div class="mt-6 flex items-center justify-center gap-2 xl:gap-3">
				<button
					onclick={() => api.toggleShuffle()}
					class="flex h-10 w-10 cursor-pointer items-center justify-center rounded-full transition-colors hover:bg-foreground/10 {shuffleOn
						? 'text-primary'
						: 'text-muted-foreground hover:text-foreground'}"
					aria-label={t('player.shuffle')}
					aria-pressed={shuffleOn}
				>
					<HugeiconsIcon strokeWidth={2} icon={ShuffleIcon} class="h-4 w-4" />
				</button>
				<button
					onclick={() => api.prevTrack()}
					class="flex h-12 w-12 cursor-pointer items-center justify-center rounded-full text-foreground/90 transition-colors hover:bg-foreground/10 hover:text-foreground"
					aria-label={t('player.previous')}
				>
					<HugeiconsIcon strokeWidth={2} icon={PreviousIcon} class="h-5 w-5" />
				</button>
				<button
					onclick={() => api.togglePause()}
					class="mx-1 flex h-16 w-16 cursor-pointer items-center justify-center rounded-[var(--r-full)] bg-primary text-primary-foreground transition-transform duration-150 hover:scale-[1.06] active:scale-95"
					aria-label={playback.paused ? t('player.play') : t('player.pause')}
				>
					<HugeiconsIcon strokeWidth={2}
						icon={PauseIcon}
						altIcon={PlayIcon}
						showAlt={playback.paused}
						class="h-5 w-5"
					/>
				</button>
				<button
					onclick={() => api.nextTrack()}
					class="flex h-12 w-12 cursor-pointer items-center justify-center rounded-full text-foreground/90 transition-colors hover:bg-foreground/10 hover:text-foreground"
					aria-label={t('player.next')}
				>
					<HugeiconsIcon strokeWidth={2} icon={NextIcon} class="h-5 w-5" />
				</button>
				<button
					onclick={toggleRepeat}
					class="flex h-10 w-10 cursor-pointer items-center justify-center rounded-full transition-colors hover:bg-foreground/10 {repeat !== 'off'
						? 'text-primary'
						: 'text-muted-foreground hover:text-foreground'}"
					aria-label="Repeat"
					aria-pressed={repeat !== 'off'}
				>
					<HugeiconsIcon strokeWidth={2}
						icon={repeat === 'one' ? RepeatOne01Icon : RepeatIcon}
						class="h-4 w-4"
					/>
				</button>
			</div>
		</div>

		{#if showLyrics}
			<div
				in:fly={{ y: 24, duration: 400, easing: cubicOut }}
				class="hidden h-full min-h-0 flex-col lg:flex"
				data-theater-lyrics
			>
				<LyricsView expanded />
			</div>
		{/if}
	</div>
</section>

<style>
	.theater :global(.lyrics-scroller) {
		scrollbar-width: none;
	}
	.theater :global(.lyrics-scroller::-webkit-scrollbar) {
		display: none;
	}
	.range.on-art {
		--pct: 0%;
		background: linear-gradient(
			to right,
			color-mix(in srgb, var(--on-art) 85%, transparent) 0%,
			color-mix(in srgb, var(--on-art) 85%, transparent) var(--pct),
			color-mix(in srgb, var(--on-art) 25%, transparent) var(--pct),
			color-mix(in srgb, var(--on-art) 25%, transparent) 100%
		);
	}
	.theater-range {
		--pct: 0%;
		background: linear-gradient(
			to right,
			var(--primary) 0%,
			var(--primary) var(--pct),
			color-mix(in srgb, var(--on-art) 15%, transparent) var(--pct),
			color-mix(in srgb, var(--on-art) 15%, transparent) 100%
		);
	}
</style>
