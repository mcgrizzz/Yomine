<script lang="ts">
	import { findEpisodes, getMinedEpisodes, type FoundEpisode, type MinedEpisode } from '$lib/ipc';
	import { formatOpened, inMode } from '$lib/recents';
	import { firstUnmined, groupShows, minedIn, openedEpisodes } from '$lib/shows';
	import { openEpisode, recentFiles } from '$lib/stores';

	/** The show of the episode opened last in Local mode. */
	const show = $derived.by(() => {
		const local = $recentFiles.filter((entry) => inMode(entry, 'local'));
		const media = local[0]?.media;
		if (media?.work.kind !== 'tv' || media.episode === null) return null;
		return groupShows(local).shows.find((s) => s.work.tmdb_id === media.work.tmdb_id) ?? null;
	});

	let found = $state<FoundEpisode[]>([]);
	let mined = $state<MinedEpisode[]>([]);
	let checking = $state(true);
	let scans = 0;
	$effect(() => {
		if (!show) return;
		const scanId = ++scans;
		const episodes: FoundEpisode[] = [];
		checking = true;
		void Promise.all([
			findEpisodes(openedEpisodes(show.files), false, (episode) => {
				const at = episodes.findIndex((e) => e.path === episode.path);
				if (at < 0) episodes.push(episode);
				else episodes[at] = episode;
			}),
			getMinedEpisodes().then((episodes) => (mined = episodes))
		])
			.catch(() => {})
			.finally(() => {
				if (scanId !== scans) return;
				found = episodes;
				checking = false;
			});
	});

	const next = $derived(
		show && !checking ? firstUnmined(show, found, minedIn(mined, show.work.tmdb_id)) : null
	);
	const label = (season: number | null, number: number | null) =>
		season === null ? `E${number}` : `S${season}E${number}`;
</script>

{#if show && (checking || next)}
	{@const latest = show.files[0]}
	{@const title = show.work.original_title || show.work.title}
	{@const last = `Last opened ${label(latest.media?.season ?? null, latest.media?.episode ?? null)} · ${formatOpened(latest.last_opened)}`}
	{#if next}
		{@const target = next}
		{@const name = target.files?.[0].media?.episode_title ?? target.found?.episode_title}
		<button
			class="continue"
			title={target.files?.[0].file_path ?? target.found?.path}
			onclick={() => openEpisode(target)}
		>
			<span class="heading">▶ Continue where you left off</span>
			<span class="title" lang="ja">{title} · {label(target.season, target.number)}</span>
			{#if name}<span class="secondary">{name}</span>{/if}
			<span class="meta">First unmined · {last}</span>
		</button>
	{:else}
		<div class="continue waiting">
			<span class="heading">▶ Continue where you left off</span>
			<span class="title" lang="ja">{title}</span>
			<span class="meta">Checking folder… · {last}</span>
		</div>
	{/if}
{/if}

<style>
	.continue {
		display: flex;
		flex-direction: column;
		gap: 0.1rem;
		width: 100%;
		margin-bottom: 0.9rem;
		padding: 0.45rem 0.6rem 0.45rem 0.75rem;
		text-align: left;
		background: color-mix(in srgb, var(--accent) 8%, var(--bg-raised));
		border: 1px solid color-mix(in srgb, var(--accent) 30%, transparent);
		border-left: 3px solid var(--accent);
		border-radius: var(--radius);
	}
	button.continue:hover,
	button.continue:focus-visible {
		background: color-mix(in srgb, var(--accent) 16%, var(--bg-raised));
		border-color: var(--accent);
	}
	.waiting {
		opacity: 0.7;
	}
	.heading {
		font-size: 0.7rem;
		font-weight: 600;
		color: var(--accent);
	}
	.title {
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
		font-size: 0.95rem;
		color: var(--text);
	}
	.secondary,
	.meta {
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
		font-size: 0.7rem;
		color: var(--text-muted);
	}
</style>
