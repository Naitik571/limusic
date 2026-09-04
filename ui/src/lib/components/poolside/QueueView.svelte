<script lang="ts">
	// Poolside Queue — full queue management with reorder, remove, play next.
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { TrashIcon, PlayIcon, ArrowUp02Icon, ArrowDown02Icon } from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import { playback, toast } from '$lib/player.svelte';

	let {} = $props();

	const q = $derived(playback.queue);
	const items = $derived(q.items);
	const currentIdx = $derived(q.currentIndex);

	function playAt(i: number) {
		api.playIndex(i).catch((e) => toast.error(String(e)));
	}
	function removeAt(i: number) {
		api.removeFromQueue(i).catch((e) => toast.error(String(e)));
	}
	function moveUp(i: number) {
		if (i <= currentIdx) return;
		api.moveQueueItem(i, i - 1).catch((e) => toast.error(String(e)));
	}
	function moveDown(i: number) {
		if (i <= currentIdx) return;
		api.moveQueueItem(i, i + 1).catch((e) => toast.error(String(e)));
	}
	function clearUpcoming() {
		api.clearQueued().catch((e) => toast.error(String(e)));
		toast.info('Upcoming tracks cleared');
	}
</script>

<div class="ps-queue">
	<div class="ps-queue-head ps-anim-fade-up">
		<h2 class="ps-page-title">QUEUE</h2>
		<div class="ps-queue-info">
			<span>{items.length} tracks</span>
			{#if items.length > currentIdx + 1}
				<button class="ps-ghost" onclick={clearUpcoming}>Clear upcoming</button>
			{/if}
		</div>
	</div>

	<!-- now playing -->
	{#if items[currentIdx]}
		<div class="ps-queue-current ps-anim-fade-up" style="animation-delay:.05s">
			<span class="ps-queue-badge">NOW</span>
			{#if items[currentIdx].thumbnail}
				<img decoding="async" src={items[currentIdx].thumbnail} alt="" class="ps-queue-thumb" />
			{/if}
			<div class="ps-queue-meta">
				<span class="ps-queue-title">{items[currentIdx].title}</span>
				<span class="ps-queue-artist">{items[currentIdx].artists}</span>
			</div>
		</div>
	{/if}

	<!-- upcoming -->
	{#if items.length > currentIdx + 1}
		<h4 class="ps-section-title ps-anim-fade-up" style="animation-delay:.1s">UP NEXT</h4>
		<div class="ps-songlist">
			{#each items.slice(currentIdx + 1) as item, ri (item.video_id + ri)}
				{@const i = currentIdx + 1 + ri}
				<div class="ps-songrow ps-queue-row ps-anim-slide-in" style="animation-delay:{ri * 0.03}s">
					{#if item.thumbnail}
						<img decoding="async" loading="lazy" src={item.thumbnail} alt="" class="ps-queue-thumb-sm" />
					{/if}
					<button class="st" style="cursor:pointer;background:none;border:none;color:inherit;font:inherit;text-align:left" onclick={() => playAt(i)}>{item.title.toUpperCase()}</button>
					<span class="sa">{item.artists}</span>
					<div class="ps-queue-actions">
						<button class="ps-qbtn" onclick={() => moveUp(i)} title="Move up" aria-label="Move up">
							<HugeiconsIcon icon={ArrowUp02Icon} />
						</button>
						<button class="ps-qbtn" onclick={() => moveDown(i)} title="Move down" aria-label="Move down">
							<HugeiconsIcon icon={ArrowDown02Icon} />
						</button>
						<button class="ps-qbtn" onclick={() => removeAt(i)} title="Remove" aria-label="Remove from queue">
							<HugeiconsIcon icon={TrashIcon} />
						</button>
					</div>
				</div>
			{/each}
		</div>
	{:else}
		<div class="ps-empty ps-anim-fade-up" style="animation-delay:.15s">No upcoming tracks.</div>
	{/if}
</div>
