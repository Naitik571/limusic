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
		playback,
		ui,
		toggleNowPlayingLike
	} from '$lib/player.svelte';
	import { artworkAccent } from '$lib/artcolor';
	import { hexToHsv } from '$lib/color';
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
	const washes = new Map<string, string>();
	let wash = $state<string | null>(null);
	$effect(() => {
		const url = thumb(playback.now?.thumbnail, 400);
		if (!url) {
			wash = null;
			return;
		}
		const hit = washes.get(url);
		if (hit !== undefined) {
			wash = hit;
			return;
		}
		let alive = true;
		bake(url).then((data) => {
			if (!alive || !data) return;
			if (washes.size >= MAX_WASHES && !washes.has(url)) {
				const oldest = washes.keys().next().value;
				if (oldest !== undefined) washes.delete(oldest);
			}
			washes.set(url, data);
			wash = data;
		});
		return () => {
			alive = false;
		};
	});

	const WASH = 160;
	const WASH_BLUR = 28;
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
	onwheel={(e) => {
		const delta = e.deltaY ?? 0;
		if (delta > 0) {
			const el = document.querySelector('[data-theater-lyrics]') as HTMLElement | null;
			el?.scrollBy({ top: 120, behavior: 'smooth' });
		} else if (delta < 0) {
			const el = document.querySelector('[data-theater-lyrics]') as HTMLElement | null;
			el?.scrollBy({ top: -120, behavior: 'smooth' });
		}
	}}
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
			<p class="text-[10px] font-semibold uppercase tracking-[0.22em] text-muted-foreground">
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
			<HugeiconsIcon icon={Cancel01Icon} class="h-4 w-4" />
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
							<img
								{src}
								alt=""
								onerror={() => attempt++}
								style={srcs[2] ? `background-image:url(${srcs[2]})` : undefined}
								class="aspect-square w-full rounded-2xl bg-cover object-cover ring-1 ring-white/10"
							/>
						{:else}
							<div
								class="flex aspect-square w-full items-center justify-center rounded-2xl bg-muted text-muted-foreground/40 ring-1 ring-white/10"
							>
								<HugeiconsIcon icon={MusicNote01Icon} class="h-20 w-20" />
							</div>
						{/if}
						<div
							class="pointer-events-none absolute inset-0 rounded-2xl ring-1 ring-inset ring-white/10"
						></div>
					</div>
				{/key}

				<div
					class="absolute left-3 top-3 z-10 flex items-center rounded-full bg-black/40 px-1.5 py-1 text-white"
					role="group"
					aria-label="Volume"
					onpointerenter={() => (volHover = true)}
					onpointerleave={() => (volHover = false)}
				>
					<button
						class="flex size-6 shrink-0 cursor-pointer items-center justify-center rounded-full text-white/75 transition-colors hover:text-white"
						onclick={() => api.toggleMute()}
						aria-label={playback.volume === 0 ? 'Unmute' : 'Mute'}
					>
						<HugeiconsIcon
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
						oninput={(e) => playback.volume = Number(e.currentTarget.value)}
						onchange={(e) => api.setVolume(Number(e.currentTarget.value))}
						aria-label="Volume"
					/>
				</div>
			</div>

			<div class="mt-8 flex items-start gap-4">
				<div class="min-w-0 flex-1">
					<h1
						class="truncate font-heading text-[1.75rem] font-bold leading-tight tracking-tight xl:text-4xl"
						title={playback.now?.title}
					>
						{playback.now?.title ?? 'Not playing'}
					</h1>
					<p class="mt-2 truncate text-base text-foreground/70">
						{playback.now?.artists ?? ''}
					</p>
					{#if album}
						<p class="mt-0.5 truncate text-[13px] text-muted-foreground">{album}</p>
					{/if}
				</div>
				<div class="flex shrink-0 items-center gap-1 pt-1.5">
					<button
						onclick={() => (showLyrics = !showLyrics)}
						class="hidden h-10 w-10 cursor-pointer items-center justify-center rounded-full transition-colors hover:bg-foreground/10 lg:flex {showLyrics
							? 'text-primary'
							: 'text-muted-foreground hover:text-foreground'}"
						aria-label="Lyrics"
						aria-pressed={showLyrics}
						title="Lyrics"
					>
						<HugeiconsIcon icon={Mic01Icon} class="h-[18px] w-[18px]" />
					</button>
					{#if playback.now && !local}
						<button
							onclick={toggleLike}
							class="flex h-10 w-10 cursor-pointer items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-foreground/10 hover:text-foreground"
							aria-label="Like"
						>
							<span
								class="inline-flex"
								class:animate-heart-pop={justLiked}
								onanimationend={() => (justLiked = false)}
							>
								<HugeiconsIcon
									icon={FavouriteIcon}
									class="h-[18px] w-[18px] {playback.liked
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
					aria-label="Seek"
				/>
				<div class="mt-2 flex justify-between text-xs font-medium tabular-nums text-muted-foreground">
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
					aria-label="Shuffle"
					aria-pressed={shuffleOn}
				>
					<HugeiconsIcon icon={ShuffleIcon} class="h-[18px] w-[18px]" />
				</button>
				<button
					onclick={() => api.prevTrack()}
					class="flex h-12 w-12 cursor-pointer items-center justify-center rounded-full text-foreground/90 transition-colors hover:bg-foreground/10 hover:text-foreground"
					aria-label="Previous"
				>
					<HugeiconsIcon icon={PreviousIcon} class="h-6 w-6" />
				</button>
				<button
					onclick={() => api.togglePause()}
					class="mx-1 flex h-[68px] w-[68px] cursor-pointer items-center justify-center rounded-full bg-primary text-primary-foreground transition-transform duration-150 hover:scale-[1.06] active:scale-95"
					aria-label={playback.paused ? 'Play' : 'Pause'}
				>
					<HugeiconsIcon
						icon={PauseIcon}
						altIcon={PlayIcon}
						showAlt={playback.paused}
						class="h-7 w-7"
					/>
				</button>
				<button
					onclick={() => api.nextTrack()}
					class="flex h-12 w-12 cursor-pointer items-center justify-center rounded-full text-foreground/90 transition-colors hover:bg-foreground/10 hover:text-foreground"
					aria-label="Next"
				>
					<HugeiconsIcon icon={NextIcon} class="h-6 w-6" />
				</button>
				<button
					onclick={toggleRepeat}
					class="flex h-10 w-10 cursor-pointer items-center justify-center rounded-full transition-colors hover:bg-foreground/10 {repeat !== 'off'
						? 'text-primary'
						: 'text-muted-foreground hover:text-foreground'}"
					aria-label="Repeat"
					aria-pressed={repeat !== 'off'}
				>
					<HugeiconsIcon
						icon={repeat === 'one' ? RepeatOne01Icon : RepeatIcon}
						class="h-[18px] w-[18px]"
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
			rgba(255, 255, 255, 0.85) 0%,
			rgba(255, 255, 255, 0.85) var(--pct),
			rgba(255, 255, 255, 0.25) var(--pct),
			rgba(255, 255, 255, 0.25) 100%
		);
	}
	.theater-range {
		--pct: 0%;
		background: linear-gradient(
			to right,
			var(--primary) 0%,
			var(--primary) var(--pct),
			rgba(255, 255, 255, 0.15) var(--pct),
			rgba(255, 255, 255, 0.15) 100%
		);
	}
</style>
