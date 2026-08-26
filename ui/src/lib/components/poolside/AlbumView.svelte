<script lang="ts">
	// Poolside album detail: 3D coverflow fan of the library's albums, the selected album's
	// sleeve+vinyl deck, play action, and the custom-cover entry point.
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { ArrowLeft01Icon, PlayIcon } from '@hugeicons/core-free-icons';
	import type { BrowseItem } from '$lib/api';
	import { toast } from '$lib/player.svelte';
	import Vinyl from './Vinyl.svelte';

	let {
		albums,
		album,
		artFor,
		onBack,
		onPlayAlbum,
		onOpenCustom
	}: {
		albums: BrowseItem[];
		album: BrowseItem;
		artFor: (item: BrowseItem) => string;
		onBack: () => void;
		onPlayAlbum: (item: BrowseItem) => void;
		onOpenCustom: () => void;
	} = $props();

	let activeIdx = $state(0);
	let tip = $state('');
	let tipX = $state(0);
	let tipY = $state(0);

	$effect(() => {
		const i = albums.findIndex((a) => a.id === album.id);
		activeIdx = i >= 0 ? i : 0;
	});

	// relative offset with wrap-around so the fan never empties
	function off(i: number, n: number, active: number): number {
		let o = i - active;
		if (n > 5) {
			if (o < -3) o += n;
			if (o > 3) o -= n;
		}
		return o;
	}

	function select(i: number) {
		activeIdx = i;
	}

	function playActive() {
		const a = albums[activeIdx];
		if (!a) return;
		if (a.id.startsWith('LOCALALBUM:')) {
			toast.info('Local albums play from the library grid for now');
			return;
		}
		onPlayAlbum(a);
	}
</script>

<div class="ps-view ps-albumview">
	<button class="ps-edge-btn absolute left-8 top-7" onclick={onBack} title="Back to library" aria-label="Back to library">
		<HugeiconsIcon icon={ArrowLeft01Icon} />
	</button>

	<div class="ps-fan">
		{#each albums as a, i (a.id)}
			{@const o = off(i, albums.length, activeIdx)}
			{#if Math.abs(o) <= 3}
				<div
					class="ps-fcard {o !== 0 ? 'dim' : ''}"
					style="transform:translateX({o * 118}px) translateZ({-Math.abs(o) * 130}px) rotateY({o *
						-32}deg);z-index:{10 - Math.abs(o)};transition-delay:{Math.abs(o) * 40}ms"
					role="button"
					tabindex="0"
					onclick={() => (o === 0 ? playActive() : select(i))}
					onkeydown={(e) => e.key === 'Enter' && (o === 0 ? playActive() : select(i))}
					onmousemove={(e) => {
						tip = o === 0 ? `${a.title} / ${a.subtitle ?? ''}` : a.title;
						tipX = e.clientX + 14;
						tipY = e.clientY - 10;
					}}
					onmouseleave={() => (tip = '')}
				>
					<img src={artFor(a)} alt={a.title} />
				</div>
			{/if}
		{/each}
	</div>

	{#if tip}
		<div class="ps-fan-tip" style="left:{tipX}px;top:{tipY}px;opacity:1">{tip}</div>
	{/if}

	<div class="text-center">
		<h3 class="text-[14px] font-bold tracking-[0.1em] ps-title-glow">{album.title.toUpperCase()}</h3>
		<p class="mt-1.5 text-[10px] tracking-[0.14em] uppercase text-cyan-100">{album.subtitle ?? ''}</p>
	</div>

	<div class="flex items-center gap-3">
		<button class="ps-aqua px-4.5 py-2.5 text-[9px] flex items-center gap-1.5" onclick={playActive}>
			<HugeiconsIcon icon={PlayIcon} class="w-3.5 h-3.5" />
			Play album
		</button>
		<button class="ps-ghost" onclick={onOpenCustom}>Add custom CD cover</button>
	</div>

	<div class="ps-alb-deck relative" style="width:min(20vh,180px);margin-top:-6px">
		<div class="ps-sleeve" style="aspect-ratio:1;width:100%"><div class="mouth"></div></div>
		<Vinyl src={artFor(album)} playing={true} style="width:88%;position:absolute;top:6%;left:6%;transform:translateX(24%)" />
	</div>
</div>
