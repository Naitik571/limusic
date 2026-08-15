<script lang="ts">
	// The single, extendable mini-player. The `expanded` prop morphs the design:
	//   collapsed -> slim bar: masked cover on the left, title/meta, transport + a queue peek.
	//   expanded  -> full now-playing card: big centered cover, transport, volume, like, queue.
	// The expand button inside toggles `expanded` and asks Rust to resize the window to match,
	// so "how extended it is" drives both the layout and the window size.
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
		MusicNote01Icon,
		MaximizeScreenIcon,
		MinimizeScreenIcon,
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

	let { expanded = false }: { expanded?: boolean } = $props();

	const now = $derived(playback.now);
	const shuffleOn = $derived(playback.queue.shuffle ?? false);
	const repeat = $derived(playback.queue.repeat ?? 'off');
	const likeable = $derived(!!now && !api.isLocalId(now.videoId));

	const fmt = (s: number) => {
		const t = Math.max(0, Math.floor(s));
		return `${Math.floor(t / 60)}:${String(t % 60).padStart(2, '0')}`;
	};
	const elapsed = $derived(fmt(playback.position));
	const remaining = $derived(fmt((playback.duration || 0) - playback.position));

	// Three fit in the queue peek; the fourth is clipped by the mask so it reads as continuing.
	const upcoming = $derived.by(() => {
		const { items, currentIndex } = playback.queue;
		return items
			.slice(currentIndex + 1, currentIndex + 5)
			.map((item, k) => ({ item, index: currentIndex + 1 + k }));
	});

	const artBtn =
		'flex size-7 shrink-0 cursor-pointer items-center justify-center rounded-md transition-colors hover:bg-muted';
	const panelBtn =
		'flex size-8 shrink-0 cursor-pointer items-center justify-center rounded-full transition-colors hover:bg-muted';

	let volHover = $state(false);
	let volDragging = $state(false);
	const volOpen = $derived(volHover || volDragging);

	let justLiked = $state(false);
	function toggleLike() {
		if (!playback.liked) justLiked = true;
		toggleNowPlayingLike();
	}

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

	// Expand/collapse: flip the prop and ask Rust to resize the window to the matching size so the
		// layout and the window grow/shrink together. The restore/close buttons are window-aware via
		// the label we read at mount.
		import { getCurrentWindow } from '@tauri-apps/api/window';
		const label = getCurrentWindow().label;
		// Only one mini window now (no floating); closeMini works for both compact/expanded.
		function toggleExpand() {
			expanded = !expanded;
			api.setMiniExpanded(expanded).catch(() => {});
		}
		function closeOrRestore() {
			api.closeMini().catch(() => {});
		}
</script>

<svelte:window onpointerup={() => (volDragging = false)} />

<div
	data-tauri-drag-region="deep"
	class="group relative flex h-screen w-screen select-none overflow-hidden rounded-2xl border-transparent glass-strong text-foreground {expanded
		? 'flex-col'
		: 'flex-row'}"
>
	<!-- Cover art, masked into the card. In collapsed mode it sits under the left half; expanded it
	     is the big centered hero. Keyed so a track change cross-fades. -->
	{#key now?.videoId}
		{#if now?.thumbnail}
			{#if expanded}
				<div class="pointer-events-none absolute inset-0 overflow-hidden">
					<img
						src={thumb(now.thumbnail, 960)}
						alt=""
						in:fade={{ duration: 400 }}
						class="absolute inset-0 h-full w-full scale-125 object-cover opacity-50 blur-2xl"
					/>
					<div class="absolute inset-0 bg-gradient-to-b from-black/45 via-black/25 to-black/75"></div>
				</div>
				<div class="relative z-10 mx-auto mt-10 w-full max-w-[15rem] px-6">
					<img
						src={thumb(now.thumbnail, 720)}
						alt=""
						in:fade={{ duration: 300 }}
						class="aspect-square w-full rounded-[1.75rem] object-cover shadow-[0_20px_50px_-12px_rgb(0_0_0/0.8)]"
					/>
				</div>
			{:else}
				<img
					src={thumb(now.thumbnail, 480)}
					alt=""
					in:fade={{ duration: 300 }}
					class="pointer-events-none absolute inset-y-0 left-0 h-full w-[62%] object-cover"
					style="mask-image:linear-gradient(to right,#000 0,#000 70%,transparent 100%);-webkit-mask-image:linear-gradient(to right,#000 0,#000 70%,transparent 100%)"
				/>
			{/if}
		{/if}
	{/key}
	{#if !expanded && now?.thumbnail}
		<div
			class="pointer-events-none absolute inset-y-0 left-0 w-[62%]"
			style="background:linear-gradient(to right,rgb(0 0 0/0.72) 0%,rgb(0 0 0/0.58) 70%,rgb(0 0 0/0) 100%)"
		></div>
	{/if}

	<!-- Restore / close. Hidden until hover (it's the way out, not part of the design). -->
		<button
			class="absolute {expanded ? 'right-3 top-3' : 'left-2 top-2'} z-20 flex size-6 cursor-pointer items-center justify-center rounded-md text-white/60 opacity-0 transition hover:bg-white/15 hover:text-white focus-visible:opacity-100 group-hover:opacity-100"
			onclick={closeOrRestore}
			title="Back to Limusic"
			aria-label="Back to Limusic"
		>
			<HugeiconsIcon icon={MaximizeScreenIcon} class="h-3.5 w-3.5" />
		</button>

	<!-- Expand / collapse. -->
	<button
		class="absolute {expanded ? 'right-3 top-3' : 'right-2 top-2'} z-20 flex size-6 translate-y-7 cursor-pointer items-center justify-center rounded-md text-white/60 opacity-0 transition hover:bg-white/15 hover:text-white focus-visible:opacity-100 group-hover:opacity-100"
		onclick={toggleExpand}
		title={expanded ? 'Collapse' : 'Expand'}
		aria-label={expanded ? 'Collapse' : 'Expand'}
	>
		<HugeiconsIcon icon={expanded ? MinimizeScreenIcon : MaximizeScreenIcon} class="h-3.5 w-3.5" />
	</button>

	{#if expanded}
		<!-- Expanded: centered hero + title + transport + volume/like. -->
		<div class="group relative z-10 flex min-h-0 flex-1 flex-col items-center justify-center gap-5 px-6 pb-8">
			<div class="w-full text-center [text-shadow:0_1px_6px_rgb(0_0_0/0.8)]">
				<div class="truncate font-heading text-lg font-semibold leading-tight text-white">
					{now?.title ?? 'Nothing playing'}
				</div>
				<div class="mt-1 truncate text-sm text-white/80">{now?.artists ?? ''}</div>
			</div>

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
					<HugeiconsIcon icon={RepeatIcon} altIcon={RepeatOne01Icon} showAlt={repeat === 'one'} class="h-4.5 w-4.5" />
				</button>
			</div>

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
						class="range on-art min-w-0 transition-[width,opacity] duration-150 {volOpen ? 'w-24 opacity-100' : 'w-0 opacity-0'}"
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
					<button
						class={artBtn}
						onclick={toggleLike}
						aria-label={playback.liked ? 'Remove from liked songs' : 'Add to liked songs'}
					>
						<span class="flex" class:animate-heart-pop={justLiked} onanimationend={() => (justLiked = false)}>
							<HugeiconsIcon
								icon={FavouriteIcon}
								class="h-4 w-4 {playback.liked ? 'fill-current text-primary' : ''}"
							/>
						</span>
					</button>
				{/if}
			</div>
		</div>
	{:else}
		<!-- Collapsed: slim bar with the masked cover, title, transport, and a queue peek. -->
		<div class="relative z-10 flex min-w-0 flex-1 flex-col justify-between p-3.5 pl-4">
			<div class="flex items-center justify-end gap-0.5">
				<div
					class="flex items-center"
					role="group"
					aria-label="Volume"
					onpointerenter={() => (volHover = true)}
					onpointerleave={() => (volHover = false)}
				>
					<input
						type="range"
						class="range on-art min-w-0 transition-[width,opacity] duration-150 {volOpen ? 'w-20 opacity-100' : 'w-0 opacity-0'}"
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
						<span class="flex" class:animate-heart-pop={justLiked} onanimationend={() => (justLiked = false)}>
							<HugeiconsIcon icon={FavouriteIcon} class="h-4 w-4 {playback.liked ? 'fill-current text-primary' : ''}" />
						</span>
					</button>
				{/if}
			</div>

			<div class="min-w-0 [text-shadow:0_1px_4px_rgb(0_0_0/0.7)]">
				<div class="truncate font-heading text-[0.95rem] font-semibold leading-tight text-white">
					{now?.title ?? 'Nothing playing'}
				</div>
				<div class="truncate text-xs leading-snug text-white/75">{now?.artists ?? ''}</div>
			</div>

			<div class="flex items-center gap-2">
				<button class={artBtn} onclick={() => api.prevTrack()} aria-label="Previous">
					<HugeiconsIcon icon={PreviousIcon} class="h-4 w-4" />
				</button>
				<input
					type="range"
					class="range on-art min-w-0 flex-1"
					style="--pct:{playback.duration ? (shownPosition / playback.duration) * 100 : 0}%"
					min="0"
					max={playback.duration || 0}
					value={shownPosition}
					oninput={onSeekInput}
					onchange={onSeekCommit}
					aria-label="Seek"
				/>
				<button class={artBtn} onclick={() => api.nextTrack()} aria-label="Next">
					<HugeiconsIcon icon={NextIcon} class="h-4 w-4" />
				</button>
			</div>
		</div>

		<div class="relative z-10 flex w-56 shrink-0 flex-col gap-2 py-3 pl-1 pr-3">
			<div
				class="flex min-h-0 flex-1 flex-col gap-0.5 overflow-hidden"
				style="mask-image:linear-gradient(to bottom,#000 0,#000 78%,transparent 100%);-webkit-mask-image:linear-gradient(to bottom,#000 0,#000 78%,transparent 100%)"
			>
				{#each upcoming as { item, index } (item.video_id + index)}
					<button
						class="flex shrink-0 cursor-pointer items-center gap-2 rounded-md px-1.5 py-0.5 text-left transition-colors hover:bg-muted"
						onclick={() => api.playIndex(index)}
						title={item.title}
					>
						{#if item.thumbnail}
							<img
								src={thumb(item.thumbnail, 64)}
								alt=""
								style="max-width:none"
								class="h-6 w-6 shrink-0 rounded object-cover"
							/>
						{:else}
							<div class="flex h-6 w-6 shrink-0 items-center justify-center rounded bg-muted text-muted-foreground/50">
								<HugeiconsIcon icon={MusicNote01Icon} class="h-3 w-3" />
							</div>
						{/if}
						<span class="truncate text-xs">{item.title}</span>
					</button>
				{:else}
					<p class="px-1.5 py-0.5 text-xs text-muted-foreground">Nothing up next</p>
				{/each}
			</div>

			<div class="flex shrink-0 items-center justify-center gap-2.5">
				<button
					class="{panelBtn} {shuffleOn ? 'text-primary' : 'text-muted-foreground'}"
					onclick={() => api.toggleShuffle()}
					aria-label="Shuffle"
					aria-pressed={shuffleOn}
				>
					<HugeiconsIcon icon={ShuffleIcon} class="h-4 w-4" />
				</button>
				<button
					class="flex size-9 shrink-0 cursor-pointer items-center justify-center rounded-full bg-primary text-primary-foreground transition-colors hover:bg-primary/80"
					onclick={() => api.togglePause()}
					aria-label="Play/pause"
				>
					<HugeiconsIcon icon={PauseIcon} altIcon={PlayIcon} showAlt={playback.paused} class="h-4 w-4" />
				</button>
				<button
					class="{panelBtn} {repeat !== 'off' ? 'text-primary' : 'text-muted-foreground'}"
					onclick={cycleRepeat}
					aria-label="Repeat: {repeat}"
					aria-pressed={repeat !== 'off'}
				>
					<HugeiconsIcon icon={RepeatIcon} altIcon={RepeatOne01Icon} showAlt={repeat === 'one'} class="h-4 w-4" />
				</button>
			</div>
		</div>
	{/if}
</div>
