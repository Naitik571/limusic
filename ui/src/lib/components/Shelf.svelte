<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { ArrowLeft01Icon, ArrowRight01Icon } from '@hugeicons/core-free-icons';
	import MediaCard from './MediaCard.svelte';
	import CommunityCard from './CommunityCard.svelte';
	import type { BrowseItem } from '$lib/api';

	let {
		title,
		items,
		onMore,
		community = false,
		headingClass = 'font-heading text-lg font-semibold'
	}: {
		title?: string;
		items: BrowseItem[];
		/** Renders a "More" button in the header when provided. */
		onMore?: () => void;
		/**
		 * Community playlist cards: three per row at most, stretching to fill the width instead of
		 * fitting more cards as the window grows. The arrows page through the rest.
		 */
		community?: boolean;
		/** Artist pages use text-xl font-bold; home uses the default. */
		headingClass?: string;
	} = $props();

	let row = $state<HTMLDivElement | null>(null);
	let canLeft = $state(false);
	let canRight = $state(false);

	function update() {
		if (!row) return;
		canLeft = row.scrollLeft > 4;
		canRight = row.scrollLeft + row.clientWidth < row.scrollWidth - 4;
	}

	const measureOnEnter = (el: HTMLElement) => {
		el.addEventListener('pointerenter', update);
		return () => el.removeEventListener('pointerenter', update);
	};

	function page(dir: 1 | -1) {
		row?.scrollBy({ left: dir * Math.round(row.clientWidth * 0.9), behavior: 'smooth' });
	}

	$effect(() => {
		items; // re-measure when content changes
		update();
	});
</script>

<svelte:window onresize={update} />

<!-- content-visibility: a home feed grows to hundreds of cards and WebKit keeps every one of them in
     style, layout and paint. Because it rasterizes in tiles, one card's hover repaint re-rasterizes
     the images around it, so hovering gets slower the further you scroll. Skipping off-screen
     shelves caps that at a screenful. `auto 17.5rem` is a shelf's height (heading + w-40 row); the
     `auto` keyword swaps in the real size once measured, so the scrollbar stays put. -->
<section class="[content-visibility:auto] [contain-intrinsic-size:auto_17.5rem]">
	{#if title || onMore}
		<div class="mb-3 flex items-baseline justify-between gap-3">
			<!-- The title is the same navigation as "See all": a shelf header is a big, obvious click
			     target and every music app treats it as one. "See all" stays visible so the affordance
			     doesn't depend on hovering to discover it. -->
			{#if title && onMore}
				<button
					class="min-w-0 cursor-pointer text-left hover:underline"
					onclick={onMore}
					title="See all {title}"
				>
					<h2 class="{headingClass} truncate">{title}</h2>
				</button>
			{:else if title}
				<h2 class="{headingClass} truncate">{title}</h2>
			{/if}
			{#if onMore}
				<button
					class="flex shrink-0 cursor-pointer items-center gap-0.5 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground"
					onclick={onMore}
				>
					See all
					<HugeiconsIcon icon={ArrowRight01Icon} class="h-3.5 w-3.5" />
				</button>
			{/if}
		</div>
	{/if}
	<!-- Measure on pointer enter, because a shelf skipped by content-visibility has no layout at
	     mount: scrollWidth reads 0 and the arrows never appear. They only show on hover, so measuring
	     as the pointer arrives is both correct and later than the mount-time forced layout.
	     An attachment rather than onpointerenter: the handler doesn't make this div interactive. -->
	<div class="group/shelf relative" {@attach measureOnEnter}>
		<div
			class="flex snap-x overflow-x-auto pb-2 {community ? 'gap-3' : 'gap-2'}"
			bind:this={row}
			onscroll={update}
		>
			{#each items as item, i (item.id + ':' + i)}
				<!-- A community shelf is playlists, but don't stretch a stray song/album card to a third
				     of the row if one ever shows up — it keeps the plain card and its width. -->
				{@const rich = community && item.kind === 'playlist'}
				<div
					class="shrink-0 snap-start {rich
						? 'basis-full sm:basis-[calc((100%-0.75rem)/2)] lg:basis-[calc((100%-1.5rem)/3)]'
						: 'w-40'}"
				>
					{#if rich}
						<CommunityCard {item} />
					{:else}
						<MediaCard {item} />
					{/if}
				</div>
			{/each}
		</div>
		<!-- Fades, not just arrows: a card sliced by the edge should read as "the row continues", which
		     is also what makes the arrow legible sitting on top of artwork. Both are pointer-transparent
		     so they never eat a click meant for the card underneath. -->
		{#if canLeft}
			<div
				class="pointer-events-none absolute inset-y-0 left-0 w-16 bg-gradient-to-r from-background to-transparent"
			></div>
			<button
				aria-label="Scroll left"
				onclick={() => page(-1)}
				class="absolute left-1 top-1/2 flex h-9 w-9 -translate-y-1/2 cursor-pointer items-center justify-center rounded-full border bg-background text-foreground opacity-0 shadow-lg transition hover:scale-105 focus-visible:opacity-100 group-hover/shelf:opacity-100"
			>
				<HugeiconsIcon icon={ArrowLeft01Icon} class="h-4 w-4" />
			</button>
		{/if}
		{#if canRight}
			<div
				class="pointer-events-none absolute inset-y-0 right-0 w-16 bg-gradient-to-l from-background to-transparent"
			></div>
			<button
				aria-label="Scroll right"
				onclick={() => page(1)}
				class="absolute right-1 top-1/2 flex h-9 w-9 -translate-y-1/2 cursor-pointer items-center justify-center rounded-full border bg-background text-foreground opacity-0 shadow-lg transition hover:scale-105 focus-visible:opacity-100 group-hover/shelf:opacity-100"
			>
				<HugeiconsIcon icon={ArrowRight01Icon} class="h-4 w-4" />
			</button>
		{/if}
	</div>
</section>
