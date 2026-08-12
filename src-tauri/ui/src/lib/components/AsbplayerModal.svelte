<script lang="ts">
	// asbplayer media picker (issue #105). Media without loaded subtitles are
	// shown but not loadable, so the user learns why nothing happens.
	import { untrack } from 'svelte';
	import Modal from './Modal.svelte';
	import {
		asbplayerModalOpen,
		loadFromAsbplayer,
		playerStatus,
		setAsbplayerFollowNewMedia,
		setAsbplayerFollowActiveTab,
		settings
	} from '$lib/stores';
	import * as ipc from '$lib/ipc';

	let media = $state<ipc.BoundMedia[]>([]);
	let loading = $state(false);
	let error = $state<string | null>(null);
	/** Media id currently loading (all Load buttons disable; only its row shows it). */
	let busyId = $state<string | null>(null);
	/** Selected track per media id: a track number, or -1 = all tracks. */
	let selectedTrack = $state<Record<string, number>>({});

	$effect(() => {
		if ($asbplayerModalOpen) untrack(() => void refresh());
	});

	async function refresh() {
		loading = true;
		error = null;
		try {
			media = await ipc.getAsbplayerMedia();
			// Default each media's selection to its first track.
			const defaults: Record<string, number> = {};
			for (const m of media) {
				defaults[m.id] = selectedTrack[m.id] ?? m.loaded_subtitles[0]?.track_number ?? -1;
			}
			selectedTrack = defaults;
		} catch (err) {
			media = [];
			error = String(err);
		} finally {
			loading = false;
		}
	}

	async function load(m: ipc.BoundMedia) {
		busyId = m.id;
		const choice = selectedTrack[m.id] ?? -1;
		const tracks = choice === -1 ? null : [choice];
		const ok = await loadFromAsbplayer(m, tracks);
		busyId = null;
		if (ok) asbplayerModalOpen.set(false);
	}

	function close() {
		asbplayerModalOpen.set(false);
	}

	const connected = $derived($playerStatus.ws_clients > 0);
</script>

<Modal
	open={$asbplayerModalOpen}
	title="Load from asbplayer"
	width="min(720px, 92%)"
	onclose={close}
>
	{#snippet actions()}
		<button class="refresh" disabled={loading || busyId !== null} onclick={refresh}>
			{loading ? 'Refreshing…' : '⟳ Refresh'}
		</button>
	{/snippet}

	{#if !connected}
		<p class="hint">
			asbplayer is not connected. Open asbplayer and connect it to
			<code>ws://127.0.0.1</code> (Settings → WebSocket Server), then refresh.
		</p>
	{:else if loading}
		<p class="hint">Asking asbplayer for its media…</p>
	{:else if error}
		<p class="error">{error}</p>
	{:else if media.length === 0}
		<p class="hint">
			asbplayer isn't tracking any media. Open a video in a tab asbplayer is bound
			to, then refresh.
		</p>
	{:else}
		<ul class="media-list">
			{#each media as m (m.id)}
				{@const hasSubs = m.loaded_subtitles.length > 0}
				<li class="media" class:disabled={!hasSubs}>
					<div class="media-head">
						{#if m.favicon_url}
							<img class="favicon" src={m.favicon_url} alt="" />
						{:else}
							<span class="favicon placeholder">🎬</span>
						{/if}
						<span class="title" title={m.title ?? undefined}
							>{m.title?.trim() || 'Untitled video'}</span
						>
						<span class="badge">{m.media_type}</span>
						<span class="badge" class:active={m.active}
							>{m.active ? 'active tab' : 'background tab'}</span
						>
						<button
							class="load"
							disabled={!hasSubs || busyId !== null}
							onclick={() => load(m)}>{busyId === m.id ? 'Loading…' : 'Load'}</button
						>
					</div>
					{#if !hasSubs}
						<p class="no-subs">No subtitles loaded — load a subtitle file in asbplayer first.</p>
					{:else if m.loaded_subtitles.length > 1}
						<div class="tracks">
							{#each m.loaded_subtitles as t (t.track_number)}
								<label class="track">
									<input
										type="radio"
										name={`track-${m.id}`}
										value={t.track_number}
										checked={selectedTrack[m.id] === t.track_number}
										onchange={() => (selectedTrack[m.id] = t.track_number)}
									/>
									{t.file_name || `Track ${t.track_number}`}
								</label>
							{/each}
							<label class="track">
								<input
									type="radio"
									name={`track-${m.id}`}
									value={-1}
									checked={selectedTrack[m.id] === -1}
									onchange={() => (selectedTrack[m.id] = -1)}
								/>
								All tracks
							</label>
						</div>
					{:else}
						<p class="single-track">{m.loaded_subtitles[0].file_name}</p>
					{/if}
				</li>
			{/each}
		</ul>
	{/if}

	<!-- Follow modes (persisted, opt-in): armed after loading a video here;
	     opening a regular file disarms until the next asbplayer load. Also
	     exposed in the top bar's asbplayer status menu. -->
	<div class="follow-box">
		<label class="follow" title="Automatically load new videos asbplayer picks up (e.g. the next episode). Switching between already-open tabs does nothing.">
			<input
				type="checkbox"
				checked={$settings?.asbplayer_follow_new_media ?? false}
				onchange={(e) => setAsbplayerFollowNewMedia(e.currentTarget.checked)}
			/>
			Follow new videos — load the next episode automatically
		</label>
		<label
			class="follow"
			title="When you switch to a tab whose video has subtitles, load that video. Recommended: keeps mined screenshots correct — asbplayer captures the visible tab."
		>
			<input
				type="checkbox"
				checked={$settings?.asbplayer_follow_active_tab ?? true}
				onchange={(e) => setAsbplayerFollowActiveTab(e.currentTarget.checked)}
			/>
			Follow active tab — switch when I change tabs <em>(recommended)</em>
		</label>
	</div>
</Modal>

<style>
	.refresh {
		font-size: 0.8rem;
		padding: 0.2rem 0.5rem;
	}
	.hint {
		margin: 0;
		padding: 0 1rem;
		color: var(--text-muted);
	}
	.error {
		margin: 0;
		padding: 0 1rem;
		color: var(--danger);
	}
	.media-list {
		list-style: none;
		margin: 0;
		padding: 0 1rem;
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
		overflow-y: auto;
	}
	.media {
		padding: 0.5rem 0.6rem;
		background: var(--bg-raised);
		border: 1px solid var(--border);
		border-radius: var(--radius);
	}
	.media.disabled .title {
		color: var(--text-muted);
	}
	.media-head {
		display: flex;
		align-items: flex-start;
		gap: 0.5rem;
	}
	.media-head .badge,
	.media-head .load {
		flex-shrink: 0;
	}
	.favicon {
		width: 16px;
		height: 16px;
		flex-shrink: 0;
	}
	.favicon.placeholder {
		font-size: 0.85rem;
		line-height: 1;
	}
	/* Full title, wrapping up to three lines (Japanese video titles run long). */
	.title {
		flex: 1;
		min-width: 0;
		font-weight: 600;
		overflow-wrap: anywhere;
		display: -webkit-box;
		-webkit-line-clamp: 3;
		line-clamp: 3;
		-webkit-box-orient: vertical;
		overflow: hidden;
	}
	.badge {
		font-size: 0.7rem;
		padding: 0.1rem 0.4rem;
		border: 1px solid var(--border);
		border-radius: var(--radius-pill);
		color: var(--text-muted);
		white-space: nowrap;
	}
	.badge.active {
		color: var(--success);
		border-color: var(--success);
	}
	.load {
		padding: 0.25rem 0.7rem;
	}
	.no-subs {
		margin: 0.3rem 0 0;
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	.single-track {
		margin: 0.3rem 0 0;
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	.tracks {
		display: flex;
		flex-direction: column;
		gap: 0.2rem;
		margin-top: 0.4rem;
	}
	.track {
		display: flex;
		align-items: center;
		gap: 0.35rem;
		font-size: 0.85rem;
		cursor: pointer;
	}
	.follow-box {
		display: flex;
		flex-direction: column;
		gap: 0.3rem;
		padding: 0.5rem 1rem 0;
		border-top: 1px solid var(--border);
	}
	.follow {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		font-size: 0.85rem;
		color: var(--text-muted);
		cursor: pointer;
	}
</style>
