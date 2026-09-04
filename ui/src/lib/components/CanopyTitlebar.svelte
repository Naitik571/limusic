<script lang="ts">
	// Canopy's top bar — the layout's transport lives HERE, not in a bottom bar. Orchard's
	// mechanism, ported: switching to canopy unmounts PlayerBar entirely and mounts this, which
	// owns now-playing readout, the seek scrubber along the bar's bottom edge, transport, volume
	// and the queue/lyrics toggles. The stylesheet it carries (layout-canopy.css) is inert unless
	// the `data-layout-preset` attribute matches, exactly like the component-carried original.
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		PlayIcon,
		PauseIcon,
		PreviousIcon,
		NextIcon,
		ShuffleIcon,
		RepeatIcon,
		RepeatOne01Icon,
		Queue01Icon,
		Mic01Icon,
		VolumeHighIcon,
		VolumeMute02Icon,
		FavouriteIcon,
		MinusSignIcon,
		SquareIcon,
		Cancel01Icon
	} from '@hugeicons/core-free-icons';
	import logo from '$lib/assets/favicon.svg';
	import * as api from '$lib/api';
	import {
		commitVolume,
		dragVolume,
		playback,
		np,
		toggleNowPlayingLike
	} from '$lib/player.svelte';
	import { cycleRepeat } from '$lib/player.svelte';
	import '../../routes/layout-canopy.css';

	const win = getCurrentWindow();

	const paused = $derived(playback.paused);
	const shuffle = $derived(playback.queue.shuffle ?? false);
	const repeat = $derived(playback.queue.repeat ?? 'off');
	const dur = $derived(playback.duration || 0);
	const pos = $derived(Math.min(playback.position, dur || playback.position));
	const pct = $derived(dur > 0 ? (pos / dur) * 100 : 0);

	function seekFromEvent(e: PointerEvent) {
		const el = e.currentTarget as HTMLElement;
		const rect = el.getBoundingClientRect();
		if (!dur) return;
		const ratio = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width));
		api.seek(ratio * dur).catch(() => {});
	}
</script>

<!-- 68px chrome bar. Everything is a drag region except the controls; the scrubber strip along
     the bottom edge is the bar's LCD line — click anywhere on it to seek. -->
<header
	data-tauri-drag-region
	class="relative z-50 flex h-[68px] shrink-0 select-none items-center gap-4 border-b px-4 canopy-bar"
>
	<!-- Brand -->
	<div class="flex items-center gap-2.5" data-tauri-drag-region>
		<img decoding="async" src={logo} alt="" class="pointer-events-none h-5 w-5" />
		<span class="hidden font-heading text-sm font-bold tracking-tight sm:inline">Limusic</span>
	</div>

	<!-- Now playing readout — click opens the full player view. A div with a click handler so the
	     like button can be a real sibling (no button-in-button). -->
	<div
		class="group flex min-w-0 flex-1 cursor-pointer items-center gap-3 rounded-lg px-3 py-1.5 text-left transition-colors hover:bg-accent/10"
		role="button"
		tabindex="0"
		onclick={() => {
			if (playback.now) np.open = true;
		}}
		onkeydown={(e) => {
			if (e.key === 'Enter' && playback.now) np.open = true;
		}}
		title="Open player"
	>
		{#if playback.now?.thumbnail}
			<img decoding="async"
				src={playback.now.thumbnail}
				alt=""
				class="h-10 w-10 shrink-0 rounded-md object-cover"
			/>
		{:else}
			<div class="h-10 w-10 shrink-0 rounded-md bg-muted"></div>
		{/if}
		<div class="min-w-0 flex-1">
			<div class="truncate text-sm font-medium {playback.now ? '' : 'text-muted-foreground'}">
				{playback.now?.title ?? 'Nothing playing'}
			</div>
			<div class="truncate text-xs text-muted-foreground">
				{playback.now?.artists ?? 'Pick something to play'}
			</div>
		</div>
	</div>
	<!-- Like, inline with the readout — the bar is the only transport in this layout. -->
	{#if playback.now}
		<button
			class="shrink-0 p-1 text-muted-foreground transition-colors hover:text-foreground {playback.liked
				? 'text-primary'
				: ''}"
			onclick={() => toggleNowPlayingLike()}
			aria-label={playback.liked ? 'Remove from liked songs' : 'Add to liked songs'}
		>
			<HugeiconsIcon icon={FavouriteIcon} class="h-4 w-4" />
		</button>
	{/if}

	<!-- Transport -->
	<div class="flex items-center gap-1">
		<button
			class="hidden h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent/10 hover:text-foreground {shuffle
				? 'text-primary'
				: ''} sm:flex"
			onclick={() => api.toggleShuffle().catch(() => {})}
			aria-label="Shuffle"
			title="Shuffle"
		>
			<HugeiconsIcon icon={ShuffleIcon} class="h-4 w-4" />
		</button>
		<button
			class="flex h-9 w-9 items-center justify-center rounded-md text-foreground/90 transition-colors hover:bg-accent/10"
			onclick={() => api.prevTrack().catch(() => {})}
			aria-label="Previous"
		>
			<HugeiconsIcon icon={PreviousIcon} class="h-5 w-5" />
		</button>
		<button
			class="flex h-10 w-10 items-center justify-center rounded-full bg-primary text-primary-foreground shadow transition-transform hover:scale-105"
			onclick={() => api.togglePause().catch(() => {})}
			aria-label={paused ? 'Play' : 'Pause'}
		>
			<!-- Two branches: HugeiconsIcon freezes `icon` at mount. -->
			{#if paused}
				<HugeiconsIcon icon={PlayIcon} class="h-5 w-5" />
			{:else}
				<HugeiconsIcon icon={PauseIcon} class="h-5 w-5" />
			{/if}
		</button>
		<button
			class="flex h-9 w-9 items-center justify-center rounded-md text-foreground/90 transition-colors hover:bg-accent/10"
			onclick={() => api.nextTrack().catch(() => {})}
			aria-label="Next"
		>
			<HugeiconsIcon icon={NextIcon} class="h-5 w-5" />
		</button>
		<button
			class="hidden h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent/10 hover:text-foreground {repeat !==
			'off'
				? 'text-primary'
				: ''} sm:flex"
			onclick={() => cycleRepeat().catch(() => {})}
			aria-label="Repeat"
			title={repeat === 'one' ? 'Repeat one' : repeat === 'all' ? 'Repeat all' : 'Repeat off'}
		>
			<HugeiconsIcon icon={repeat === 'one' ? RepeatOne01Icon : RepeatIcon} class="h-4 w-4" />
		</button>
	</div>

	<!-- Volume -->
	<div class="hidden items-center gap-2 md:flex">
		<button
			class="p-1 text-muted-foreground transition-colors hover:text-foreground"
			onclick={() => commitVolume(playback.volume === 0 ? 100 : 0)}
			aria-label="Mute"
		>
			<HugeiconsIcon
				icon={playback.volume === 0 ? VolumeMute02Icon : VolumeHighIcon}
				class="h-4 w-4"
			/>
		</button>
		<input
			type="range"
			min="0"
			max="100"
			value={playback.volume}
			oninput={(e) => dragVolume(Number(e.currentTarget.value))}
			onchange={(e) => commitVolume(Number(e.currentTarget.value))}
			class="w-24 accent-primary"
			aria-label="Volume"
		/>
	</div>

	<!-- Queue / lyrics toggles -->
	<div class="flex items-center gap-1">
		<button
			class="flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent/10 hover:text-foreground {np.open &&
			np.tab === 'queue'
				? 'text-primary'
				: ''}"
			onclick={() => {
				np.tab = 'queue';
				np.open = true;
			}}
			aria-label="Queue"
			title="Queue"
		>
			<HugeiconsIcon icon={Queue01Icon} class="h-4 w-4" />
		</button>
		<button
			class="flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent/10 hover:text-foreground {np.open &&
			np.tab === 'lyrics'
				? 'text-primary'
				: ''}"
			onclick={() => {
				np.tab = 'lyrics';
				np.open = true;
			}}
			aria-label="Lyrics"
			title="Lyrics"
		>
			<HugeiconsIcon icon={Mic01Icon} class="h-4 w-4" />
		</button>
	</div>

	<!-- Window controls -->
	<div class="flex items-center">
		<div class="mx-1 h-4 w-px bg-border"></div>
		<button
			class="flex h-9 w-10 items-center justify-center text-muted-foreground transition-colors hover:bg-accent/10 hover:text-foreground"
			onclick={() => win.minimize()}
			aria-label="Minimize"
		>
			<HugeiconsIcon icon={MinusSignIcon} class="h-4 w-4" />
		</button>
		<button
			class="flex h-9 w-10 items-center justify-center text-muted-foreground transition-colors hover:bg-accent/10 hover:text-foreground"
			onclick={() => win.toggleMaximize()}
			aria-label="Maximize"
		>
			<HugeiconsIcon icon={SquareIcon} class="h-3.5 w-3.5" />
		</button>
		<button
			class="flex h-9 w-10 items-center justify-center text-muted-foreground transition-colors hover:text-destructive"
			onclick={() => win.close()}
			aria-label="Close"
		>
			<HugeiconsIcon icon={Cancel01Icon} class="h-4 w-4" />
		</button>
	</div>

	<!-- LCD scrubber: the bar's bottom edge IS the seek bar. -->
	{#if playback.now}
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div
			class="group absolute inset-x-0 bottom-0 h-1.5 cursor-pointer"
			onpointerdown={seekFromEvent}
			role="slider"
			aria-label="Seek"
			aria-valuenow={Math.round(pos)}
			aria-valuemin={0}
			aria-valuemax={Math.round(dur)}
			tabindex="0"
		>
			<div class="absolute inset-x-0 top-1/2 h-px -translate-y-1/2 bg-border"></div>
			<div
				class="absolute left-0 top-1/2 h-[3px] -translate-y-1/2 rounded-full bg-primary transition-[width] duration-200"
				style="width:{pct}%"
			></div>
		</div>
	{/if}
</header>
