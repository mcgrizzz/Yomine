<script lang="ts">
	import Modal from './Modal.svelte';
	import RecentCard from './RecentCard.svelte';
	import ShowCard from './ShowCard.svelte';
	import ShowTile from './ShowTile.svelte';
	import {
		findEpisodes,
		getMinedEpisodes,
		type FoundEpisode,
		type MinedEpisode,
		type MiningMode,
		type RecentFileEntry
	} from '$lib/ipc';
	import { recentFiles, browseModalOpen, openEpisode, openRecentFile } from '$lib/stores';
	import { filename, recentKey } from '$lib/recents';
	import { groupShows, minedIn, openedEpisodes, type Openable, type Show } from '$lib/shows';

	type Filter = MiningMode | 'all';
	type View = 'shows' | 'recent';

	let view = $state<View>('shows');
	let filter = $state<Filter>('all');
	let search = $state('');

	// Books count as Local: they're files on disk, though opening one keeps the mode.
	const inFilter = (entry: RecentFileEntry, f: Filter) =>
		f === 'all' || (entry.mode ?? 'local') === f;
	const filters: [Filter, string][] = [
		['all', 'All'],
		['local', 'Local'],
		['asbplayer', 'asbplayer']
	];
	const counts = $derived(
		Object.fromEntries(
			filters.map(([f]) => [f, $recentFiles.filter((entry) => inFilter(entry, f)).length])
		) as Record<Filter, number>
	);
	const shown = $derived.by(() => {
		const f = filter;
		const query = search.trim().toLowerCase();
		return $recentFiles.filter(
			(entry) =>
				inFilter(entry, f) &&
				(!query ||
					[
						entry.title,
						entry.subtitle ?? '',
						filename(entry.file_path),
						entry.media?.work.title ?? '',
						entry.media?.work.original_title ?? ''
					].some((text) => text.toLowerCase().includes(query)))
		);
	});

	const grouped = $derived(groupShows(shown));

	// Kept between openings, and shown while the folders are checked again.
	let found = $state<Record<string, FoundEpisode>>({});
	/** The shows whose folders are being looked in. */
	let checking = $state<Set<number>>(new Set());
	let scans = 0;
	async function scan(rescan = false) {
		const scanId = ++scans;
		const opened = openedEpisodes($recentFiles);
		const seen = new Set<string>();
		checking = new Set(opened.map(([, show]) => show));
		try {
			await findEpisodes(opened, rescan, (episode) => {
				if (scanId !== scans) return;
				seen.add(episode.path);
				found[episode.path] = episode;
			});
			if (scanId === scans) {
				for (const path of Object.keys(found)) if (!seen.has(path)) delete found[path];
			}
		} catch {
			// Keeps what was found before.
		} finally {
			if (scanId === scans) checking = new Set();
		}
	}

	let mined = $state<MinedEpisode[]>([]);
	$effect(() => {
		if (!$browseModalOpen || view !== 'shows') return;
		void scan();
		void getMinedEpisodes()
			.then((episodes) => (mined = episodes))
			.catch(() => {});
	});
	const foundFor = (tmdbId: number) =>
		filter === 'asbplayer' ? [] : Object.values(found).filter((f) => f.work.tmdb_id === tmdbId);

	const showKey = (show: Show) => `${show.work.kind}:${show.work.tmdb_id}`;
	const series = $derived(grouped.shows.filter((s) => s.work.kind === 'tv'));
	const films = $derived(grouped.shows.filter((s) => s.work.kind !== 'tv'));
	// Starts on the show opened last.
	let selectedKey = $state<string | null>(null);
	const selected = $derived(
		grouped.shows.find((s) => showKey(s) === selectedKey) ?? grouped.shows[0] ?? null
	);

	function open(entry: RecentFileEntry) {
		browseModalOpen.set(false);
		void openRecentFile(entry);
	}

	function openShowEpisode(episode: Openable) {
		browseModalOpen.set(false);
		void openEpisode(episode);
	}
</script>

<Modal
	open={$browseModalOpen}
	title="Browse Files"
	width="min(660px, 92%)"
	onclose={() => browseModalOpen.set(false)}
>
	{#snippet actions()}
		{#if view === 'shows'}
			<button
				class="rescan"
				disabled={checking.size > 0}
				title="Match and check every file in the folders of opened episodes again"
				onclick={() => void scan(true)}
				>{checking.size > 0 ? 'Checking folders…' : '⟳ Rescan folders'}</button
			>
		{/if}
	{/snippet}
	<div class="controls">
		<div class="filters" role="radiogroup" aria-label="View">
			{#each [['shows', 'Shows'], ['recent', 'Recent']] as const as [value, label] (value)}
				<button
					role="radio"
					aria-checked={view === value}
					class:on={view === value}
					onclick={() => (view = value)}>{label}</button
				>
			{/each}
		</div>
		<div class="filters" role="radiogroup" aria-label="Source">
			{#each filters as [value, label] (value)}
				<button
					role="radio"
					aria-checked={filter === value}
					class:on={filter === value}
					onclick={() => (filter = value)}
					>{label} <span class="count">{counts[value]}</span></button
				>
			{/each}
		</div>
		<input
			class="search"
			type="search"
			placeholder="Search titles and files"
			aria-label="Search files"
			bind:value={search}
		/>
	</div>
	{#if $recentFiles.length === 0}
		<p class="empty">No recent files.</p>
	{:else if shown.length === 0}
		<p class="empty">No matching files.</p>
	{:else if view === 'shows'}
		<div class="library">
			{#if selected}
				{#key showKey(selected)}
					<ShowCard
						show={selected}
						found={foundFor(selected.work.tmdb_id)}
						checking={selected.work.kind === 'tv' && checking.has(selected.work.tmdb_id)}
						mined={minedIn(mined, selected.work.tmdb_id)}
						onopen={open}
						onopenepisode={openShowEpisode}
					/>
				{/key}
			{/if}
			{#each [['Shows', series], ['Films', films]] as const as [heading, shows] (heading)}
				{#if shows.length > 0}
					<h3 class="section">{heading}</h3>
					<div class="tiles">
						{#each shows as show (showKey(show))}
							<ShowTile
								{show}
								selected={show === selected}
								onselect={() => (selectedKey = showKey(show))}
							/>
						{/each}
					</div>
				{/if}
			{/each}
			{#if grouped.unmatched.length > 0}
				<h3 class="section">Other files</h3>
				<ul class="files">
					{#each grouped.unmatched as entry (recentKey(entry))}
						<li><RecentCard {entry} showBadge={filter === 'all'} onopen={() => open(entry)} /></li>
					{/each}
				</ul>
			{/if}
		</div>
	{:else}
		<ul class="list">
			{#each shown as entry (recentKey(entry))}
				<li><RecentCard {entry} showBadge={filter === 'all'} onopen={() => open(entry)} /></li>
			{/each}
		</ul>
	{/if}
</Modal>

<style>
	.controls {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.6rem;
		padding: 0 1rem 0.6rem;
	}
	.filters {
		display: inline-flex;
		padding: 2px;
		border: 1px solid var(--border);
		border-radius: 999px;
	}
	.filters button {
		padding: 0.15rem 0.7rem;
		font-size: 0.75rem;
		color: var(--text-muted);
		background: transparent;
		border: none;
		border-radius: 999px;
	}
	.filters button.on {
		color: var(--bg);
		background: var(--accent);
	}
	.filters button:not(.on):hover {
		color: var(--text);
	}
	.count {
		opacity: 0.75;
	}
	.search {
		flex: 1 1 10rem;
		min-width: 0;
		font-size: 0.8rem;
	}
	.empty {
		margin: 0;
		padding: 0 1rem;
		color: var(--text-muted);
		font-size: 0.85rem;
	}
	.rescan {
		padding: 0.15rem 0.5rem;
		font-size: 0.7rem;
	}
	.section {
		margin: 0.5rem 0 0.1rem;
		font-size: 0.75rem;
		font-weight: 600;
		color: var(--text-muted);
	}
	.library {
		display: flex;
		flex-direction: column;
		gap: 0.4rem;
		padding: 0 1rem 1rem;
		overflow-y: auto;
		/* Fades over the bottom padding, so a card is only faded while more follow. */
		mask-image: linear-gradient(to bottom, black calc(100% - 1rem), transparent);
	}
	.tiles {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(9.5rem, 1fr));
		gap: 0.3rem;
	}
	.files {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 0.3rem;
	}
	.list {
		list-style: none;
		margin: 0;
		padding: 0 1rem 1rem;
		display: flex;
		flex-direction: column;
		gap: 0.3rem;
		overflow-y: auto;
		/* Fades over the bottom padding, so a card is only faded while more follow. */
		mask-image: linear-gradient(to bottom, black calc(100% - 1rem), transparent);
	}
</style>
