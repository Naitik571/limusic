// Up Next mood chips (`All`, `Chill`, `Discover`, …) — YouTube's radio moods, wired to
// `set_radio_mood`. Shown only on radio queues (moods empty otherwise). While one applies,
// the row locks so a second tap can't stack two mood swaps.
<script lang="ts">
	import type { MoodChip } from '$lib/api';
	import * as api from '$lib/api';
	import { toast } from '$lib/player.svelte';

	let { moods, pool = false }: { moods?: MoodChip[]; pool?: boolean } = $props();

	let busy = $state<string | null>(null);

	async function pick(m: MoodChip) {
		if (m.selected || busy) return;
		busy = m.title;
		try {
			await api.setRadioMood(m.title);
		} catch (e) {
			toast.error(String(e));
		} finally {
			busy = null;
		}
	}
</script>

{#if moods?.length}
	<div class={pool ? 'ps-mood-row' : 'flex flex-wrap gap-1.5 px-2 pt-1 pb-2'} role="group" aria-label="Radio mood">
		{#each moods as m (m.title)}
			{#if pool}
				<button
					class="ps-mood-chip {m.selected ? 'on' : ''}"
					aria-pressed={m.selected}
					disabled={busy !== null}
					onclick={() => pick(m)}
				>
					{m.title}
				</button>
			{:else}
				<button
					class="cursor-pointer rounded-full border px-2.5 py-1 text-xs font-medium transition-colors disabled:opacity-50 {m.selected
						? 'border-transparent bg-primary text-primary-foreground'
						: 'border-border text-muted-foreground hover:bg-accent/10 hover:text-foreground'}"
					aria-pressed={m.selected}
					disabled={busy !== null}
					onclick={() => pick(m)}
				>
					{m.title}
				</button>
			{/if}
		{/each}
	</div>
{/if}
