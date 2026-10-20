<script lang="ts">
	import type { TmdbEpisode } from '$lib/ipc';
	import { knownEpisodes, type Show } from '$lib/shows';

	interface Props {
		show: Show;
		selected: boolean;
		onselect: () => void;
	}

	let { show, selected, onselect }: Props = $props();

	const title = $derived(show.work.original_title || show.work.title);
	const english = $derived(title === show.work.title ? null : show.work.title);
	const regular = $derived(show.seasons.filter((s) => s.number !== 0));
	const opened = $derived(regular.reduce((sum, s) => sum + s.episodes.size, 0));

	let known = $state<Record<number, TmdbEpisode[]>>({});
	$effect(() => {
		if (show.work.kind !== 'tv') return;
		for (const { number } of regular) {
			if (number === null) continue;
			void knownEpisodes(show.work.tmdb_id, number).then((episodes) => {
				if (episodes) known[number] = episodes;
			});
		}
	});
	const aired = $derived(
		regular.length > 0 && regular.every((s) => s.number !== null && known[s.number])
			? regular.reduce((sum, s) => sum + known[s.number as number].length, 0)
			: null
	);

	const progress = $derived.by(() => {
		if (show.work.kind !== 'tv') return 'Film';
		const count = aired === null ? `${opened}` : `${opened}/${aired}`;
		return `${count} opened`;
	});
</script>

<button class="tile" class:on={selected} aria-pressed={selected} onclick={onselect}>
	<span class="title" lang="ja">{title}</span>
	{#if english}<span class="english">{english}</span>{/if}
	<span class="progress">{progress}</span>
</button>

<style>
	.tile {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 0.1rem;
		min-width: 0;
		padding: 0.4rem 0.55rem;
		text-align: left;
		background: color-mix(in srgb, var(--bg-raised) 55%, transparent);
		border: 1px solid color-mix(in srgb, var(--border) 35%, transparent);
		border-radius: var(--radius);
	}
	.tile:hover,
	.tile:focus-visible {
		border-color: var(--accent);
	}
	.tile.on {
		border-color: var(--accent);
		box-shadow: inset 0 0 0 1px var(--accent);
	}
	.title,
	.english {
		max-width: 100%;
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
	}
	.title {
		font-size: 0.85rem;
		color: var(--text);
	}
	.english,
	.progress {
		font-size: 0.68rem;
		color: var(--text-muted);
	}
</style>
