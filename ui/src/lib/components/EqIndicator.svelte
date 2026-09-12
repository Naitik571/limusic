<!-- Equalizer indicator: aria-hidden 3-span visualizer-playing mark. Rendered on the
     currently-playing row (TrackRow's `active`); the bars animate via .eq-indicator in
     layout.css and rest at 8px under reduced motion. -->
<script lang="ts">
	let { class: cls = '' }: { class?: string } = $props();
</script>

<span class="eq-indicator {cls}" aria-hidden="true">
	<span></span><span></span><span></span>
</span>
