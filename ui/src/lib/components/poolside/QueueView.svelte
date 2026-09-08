<script lang="ts">
	// Poolside Queue — full queue management with reorder, remove, play next.
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { TrashIcon, PlayIcon, ArrowUp02Icon, ArrowDown02Icon } from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import { playback, toast } from '$lib/player.svelte';
	import { isSwipe, shouldRemove } from '$lib/swipe';

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
	// Horizontal swipe-to-remove on upcoming rows (playing row excluded — the backend
	// guards it too). Window-level listeners + no pointer capture, same lesson as the
	// classic QueueList: capture would eat the row's own clicks.
	let swipeIdx: number | null = $state(null);
	let swipedX = $state(0);
	let swiping = $state(false);
	let removingIdx: number | null = $state(null);
	let removingDir = $state(1);
	let pressX = 0;
	let pressY = 0;
	let pressW = 0;
	let swipeAte = false;

	function onRowDown(e: PointerEvent, i: number) {
		if (e.button !== 0) return;
		pressX = e.clientX;
		pressY = e.clientY;
		pressW = (e.currentTarget as HTMLElement).clientWidth || 300;
		const idx = i;
		const move = (ev: PointerEvent) => {
			const dx = ev.clientX - pressX;
			const dy = ev.clientY - pressY;
			if (!swiping && isSwipe(dx, dy)) {
				swiping = true;
				swipeIdx = idx;
			}
			if (swiping) {
				swipedX = dx;
				ev.preventDefault();
			}
		};
		const up = (ev: PointerEvent) => {
			window.removeEventListener('pointermove', move, true);
			window.removeEventListener('pointerup', up, true);
			window.removeEventListener('pointercancel', off, true);
			if (!swiping) return;
			const dx = ev.clientX - pressX;
			swiping = false;
			swipeIdx = null;
			swipedX = 0;
			swipeAte = true;
			setTimeout(() => (swipeAte = false), 300);
			if (shouldRemove(dx, pressW)) {
				removingIdx = idx;
				removingDir = dx < 0 ? -1 : 1;
				setTimeout(() => {
					removingIdx = null;
					removeAt(idx);
				}, 200);
			}
		};
		const off = () => {
			window.removeEventListener('pointermove', move, true);
			window.removeEventListener('pointerup', up, true);
			window.removeEventListener('pointercancel', off, true);
			swiping = false;
			swipeIdx = null;
			swipedX = 0;
		};
		window.addEventListener('pointermove', move, true);
		window.addEventListener('pointerup', up, true);
		window.addEventListener('pointercancel', off, true);
	}
	function swallowSwipeClick(e: MouseEvent) {
		if (!swipeAte) return;
		e.preventDefault();
		e.stopPropagation();
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
				{@const isSwiping = swiping && swipeIdx === i}
				{@const isRemoving = removingIdx === i}
				<!-- svelte-ignore a11y_no_static_element_interactions: pointerdown starts the
				     swipe gesture; every action stays a real button for keyboard users. -->
				<div
					class="ps-songrow ps-queue-row ps-anim-slide-in"
					style="animation-delay:{ri * 0.03}s; touch-action: pan-y;"
					onpointerdown={(e) => onRowDown(e, i)}
					onclickcapture={swallowSwipeClick}
				>
					{#if isSwiping}
						<div
							class="ps-swipe-under {swipedX < 0 ? 'right' : 'left'}"
							aria-hidden="true"
						>
							<HugeiconsIcon icon={TrashIcon} class="h-4 w-4" />
						</div>
					{/if}
					<div
						class="ps-swipe-inner"
						style={isSwiping
							? `transform: translateX(${swipedX}px); transition: none;`
							: isRemoving
								? `transform: translateX(${removingDir * 120}%); transition: transform .2s ease-in, opacity .2s; opacity: 0;`
								: 'transition: transform .25s ease-out;'}
					>
					{#if item.thumbnail}
						<img decoding="async" loading="lazy" src={item.thumbnail} alt="" draggable={false} class="ps-queue-thumb-sm" />
					{/if}
					<button class="st" style="cursor:pointer;background:none;border:none;color:inherit;font:inherit;text-align:left" onclick={() => playAt(i)}>{item.title.toUpperCase()}</button>
					<span class="sa">{item.artists}</span>
					{#if item.duration}<span class="sd">{item.duration}</span>{/if}
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
				</div>
			{/each}
		</div>
	{:else}
		<div class="ps-empty ps-anim-fade-up" style="animation-delay:.15s">No upcoming tracks.</div>
	{/if}
</div>
