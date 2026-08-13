<script lang="ts">
	// The whole UI of the floating-player window (Rust `floating.rs`). It is the same SPA as the
	// main window — the root layout picks this instead of the app chrome when the window label is
	// `floating` — so it reads the same `playback` store and calls the same commands. Nothing here
	// is floating-specific state: the window is a second pair of eyes on the same stream.
	//
	// The window is undecorated and transparent, so this component *is* the window: it paints the
	// rounded glass card, and `data-tauri-drag-region="deep"` makes every part of it a drag handle
	// except the controls (Tauri's drag script stops at buttons and inputs on its own).
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		PreviousIcon,
		NextIcon,
		PlayIcon,
		PauseIcon,
		ShuffleIcon,
		RepeatIcon,
		RepeatOne01Icon,
		FavouriteIcon,
		VolumeHighIcon,
		VolumeMute02Icon,
		Cancel01Icon
	} from '@hugeicons/core-free-icons';
	import { fade } from 'svelte/transition';
	import * as api from '$lib/api';
	import {
		playback,
		commitVolume,
		cycleRepeat,
		dragVolume,
		toggleMute,
		toggleNowPlayingLike
	} from '$lib/player.svelte';
	import { thumb } from '$lib/thumb';

	const now = $derived(playback.now);
	const shuffleOn = $derived(playback.queue.shuffle ?? false);
	const repeat = $derived(playback.queue.repeat ?? 'off');
	// A local file has no YouTube identity, so there is nothing to like (see api.isLocalId).
	const likeable = $derived(!!now && !api.isLocalId(now.videoId));

	// Apple-style remaining-time label: "1:23" elapsed, "-2:47" remaining.
	const fmt = (s: number) => {
		const t = Math.max(0, Math.floor(s));
		return `${Math.floor(t / 60)}:${String(t % 60).padStart(2, '0')}`;
	};
	const elapsed = $derived(fmt(playback.position));
	const remaining = $derived(fmt((playback.duration || 0) - playback.position));

	// Pop the heart once when favouriting (not when un-favouriting), same as the player bar.
	let justLiked = $state(false);
	function toggleLike() {
		if (!playback.liked) justLiked = true;
		toggleNowPlayingLike();
	}

	// Seek: hold the dragged value locally so incoming position ticks can't yank the thumb out
	// from under the pointer; only invoke the seek on release.
	let seekDrag = $state<number | null>(null);
	const shownPosition = $derived(seekDrag ?? playback.position);
	function onSeekInput(e: Event) {
		seekDrag = Number((e.target as HTMLInputElement).value);
	}
	function onSeekCommit(e: Event) {
		const v = Number((e.target as HTMLInputElement).value);
		playback.position = v;
		seekDrag = null;
		api.seek(v);
	}

	// Volume slider, revealed by hovering the icon (same hover/drag discipline as the mini player).
	let volHover = $state(false);
	let volDragging = $state(false);
	const volOpen = $derived(volHover || volDragging);

	const artBtn =
		'flex size-8 shrink-0 cursor-pointer items-center justify-center rounded-full text-white/80 transition hover:bg-white/15 hover:text-white';
	const panelBtn =
		'flex size-8 shrink-0 cursor-pointer items-center justify-center rounded-full transition-colors hover:bg-muted';
</script>

<svelte:window onpointerup={() => (volDragging = false)} />

<!-- h-screen/w-screen: the window has no chrome, so this fills it exactly and rounds its corners
     (the compositor can't round an undecorated window for us — same trick as the mini player). -->
<div
	data-tauri-drag-region="deep"
	class="relative flex h-screen w-screen select-none flex-col overflow-hidden rounded-[1.75rem] border-transparent glass-strong text-foreground"
>
	<!-- Ambient backdrop: the cover art, blown up and blurred, with a dark wash on top so the
	     foreground always reads. Keyed so a track change cross-fades. -->
	<div class="pointer-events-none absolute inset-0 overflow-hidden">
		{#key now?.videoId}
			{#if now?.thumbnail}
				<img
					src={thumb(now.thumbnail, 960)}
					alt=""
					in:fade={{ duration: 400 }}
					class="absolute inset-0 h-full w-full scale-125 object-cover opacity-50 blur-2xl"
				/>
			{/if}
		{/key}
		<div class="absolute inset-0 bg-gradient-to-b from-black/45 via-black/25 to-black/75"></div>
	</div>

	<!-- Dismiss. Hidden until the pointer is over the card: it is not part of the design, it is the
	     way out of it. The command destroys this very window, so its reply lands nowhere — the
	     rejection is swallowed rather than left dangling. -->
	<button
		class="absolute right-3 top-3 z-10 flex size-7 cursor-pointer items-center justify-center rounded-full bg-black/40 text-white/70 opacity-0 transition hover:bg-black/60 hover:text-white focus-visible:opacity-100 group-hover:opacity-100"
		onclick={() => api.closeFloating().catch(() => {})}
		title="Close floating player"
		aria-label="Close floating player"
	>
		<HugeiconsIcon icon={Cancel01Icon} class="h-3.5 w-3.5" />
	</button>

	<div class="group relative flex min-h-0 flex-1 flex-col items-center justify-center gap-5 px-6 pt-10">
		<!-- The cover, big and centered. -->
		<div class="w-full max-w-[15rem]">
			{#key now?.videoId}
				{#if now?.thumbnail}
					<img
						src={thumb(now.thumbnail, 720)}
						alt=""
						in:fade={{ duration: 300 }}
						class="aspect-square w-full rounded-[1.75rem] object-cover shadow-[0_20px_50px_-12px_rgb(0_0_0/0.8)]"
					/>
				{:else}
					<div
						class="aspect-square w-full rounded-[1.75rem] bg-white/10 shadow-[0_20px_50px_-12px_rgb(0_0_0/0.8)]"
					></div>
				{/if}
			{/key}
		</div>

		<!-- Title / artist. -->
		<div class="w-full text-center [text-shadow:0_1px_6px_rgb(0_0_0/0.8)]">
			<div class="truncate font-heading text-lg font-semibold leading-tight text-white">
				{now?.title ?? 'Nothing playing'}
			</div>
			<div class="mt-1 truncate text-sm text-white/80">{now?.artists ?? ''}</div>
		</div>

		<!-- Seek: slider with elapsed/remaining labels. -->
		<div class="w-full">
			<input
				type="range"
				class="range on-art w-full"
				style="--pct:{playback.duration ? (shownPosition / playback.duration) * 100 : 0}%"
				min="0"
				max={playback.duration || 0}
				value={shownPosition}
				oninput={onSeekInput}
				onchange={onSeekCommit}
				aria-label="Seek"
			/>
			<div class="mt-1 flex justify-between text-[0.7rem] font-medium tabular-nums text-white/60">
				<span>{elapsed}</span>
				<span>-{remaining}</span>
			</div>
		</div>

		<!-- Transport: shuffle / prev / play / next / repeat. -->
		<div class="flex items-center justify-center gap-2.5">
			<button
				class="{panelBtn} {shuffleOn ? 'text-primary' : 'text-white/70'}"
				onclick={() => api.toggleShuffle()}
				aria-label="Shuffle"
				aria-pressed={shuffleOn}
			>
				<HugeiconsIcon icon={ShuffleIcon} class="h-4.5 w-4.5" />
			</button>
			<button class={artBtn} onclick={() => api.prevTrack()} aria-label="Previous">
				<HugeiconsIcon icon={PreviousIcon} class="h-5 w-5" />
			</button>
			<button
				class="flex size-13 shrink-0 cursor-pointer items-center justify-center rounded-full bg-white text-black transition-transform hover:scale-105"
				onclick={() => api.togglePause()}
				aria-label="Play/pause"
			>
				<!-- HugeiconsIcon freezes `icon` at mount, so the swap has to go through
				     altIcon/showAlt — a ternary on `icon` would never repaint. -->
				<HugeiconsIcon icon={PauseIcon} altIcon={PlayIcon} showAlt={playback.paused} class="h-5 w-5" />
			</button>
			<button class={artBtn} onclick={() => api.nextTrack()} aria-label="Next">
				<HugeiconsIcon icon={NextIcon} class="h-5 w-5" />
			</button>
			<button
				class="{panelBtn} {repeat !== 'off' ? 'text-primary' : 'text-white/70'}"
				onclick={cycleRepeat}
				aria-label="Repeat: {repeat}"
				aria-pressed={repeat !== 'off'}
			>
				<HugeiconsIcon
					icon={RepeatIcon}
					altIcon={RepeatOne01Icon}
					showAlt={repeat === 'one'}
					class="h-4.5 w-4.5"
				/>
			</button>
		</div>

		<!-- Volume + like. -->
		<div class="flex items-center justify-center gap-2">
			<div
				class="flex items-center"
				role="group"
				aria-label="Volume"
				onpointerenter={() => (volHover = true)}
				onpointerleave={() => (volHover = false)}
			>
				<input
					type="range"
					class="range on-art min-w-0 transition-[width,opacity] duration-150 {volOpen
						? 'w-24 opacity-100'
						: 'w-0 opacity-0'}"
					style="--pct:{playback.volume}%"
					min="0"
					max="100"
					value={playback.volume}
					onpointerdown={() => (volDragging = true)}
					oninput={(e) => dragVolume(Number(e.currentTarget.value))}
					onchange={(e) => commitVolume(Number(e.currentTarget.value))}
					aria-label="Volume"
				/>
				<button class={artBtn} onclick={toggleMute} aria-label={playback.volume === 0 ? 'Unmute' : 'Mute'}>
					<HugeiconsIcon
						icon={VolumeHighIcon}
						altIcon={VolumeMute02Icon}
						showAlt={playback.volume === 0}
						class="h-4 w-4"
					/>
				</button>
			</div>
			{#if likeable}
				<button class={artBtn} onclick={toggleLike} aria-label={playback.liked ? 'Remove from liked songs' : 'Add to liked songs'}>
					<span
						class="flex"
						class:animate-heart-pop={justLiked}
						onanimationend={() => (justLiked = false)}
					>
						<HugeiconsIcon
							icon={FavouriteIcon}
							class="h-4 w-4 {playback.liked ? 'fill-current text-primary' : ''}"
						/>
					</span>
				</button>
			{/if}
		</div>
	</div>
</div>
