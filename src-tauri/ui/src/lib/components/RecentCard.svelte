<script lang="ts">
	import type { RecentFileEntry } from '$lib/ipc';
	import { miningMode } from '$lib/stores';
	import {
		fileIcon,
		filename,
		formatFileSize,
		formatOpened,
		formatTermCount,
		modeLabel,
		openHint
	} from '$lib/recents';

	interface Props {
		entry: RecentFileEntry;
		/** Off where every entry shown is from one mode. */
		showBadge: boolean;
		onopen: () => void;
	}

	let { entry, showBadge, onopen }: Props = $props();

	const title = $derived(entry.title.trim() || filename(entry.file_path));
	const badge = $derived(showBadge && entry.mode ? modeLabel(entry.mode) : null);
	const tooltip = $derived(
		[
			openHint(entry, $miningMode),
			entry.creator,
			entry.file_path,
			entry.file_size === null ? null : formatFileSize(entry.file_size)
		]
			.filter(Boolean)
			.join('\n')
	);
</script>

<button class="card" class:badged={badge !== null} title={tooltip} onclick={onopen}>
	{#if badge}<span class="badge">{badge}</span>{/if}
	<span class="title">{fileIcon(entry.file_path)} {title}</span>
	{#if entry.subtitle}<span class="secondary">{entry.subtitle}</span>{/if}
	<span class="meta"
		><span class="terms">{formatTermCount(entry.term_count)}</span> · {formatOpened(
			entry.last_opened
		)}</span
	>
</button>

<style>
	.card {
		position: relative;
		display: flex;
		flex-direction: column;
		gap: 0.1rem;
		width: 100%;
		padding: 0.4rem 0.6rem;
		text-align: left;
		background: color-mix(in srgb, var(--bg-raised) 55%, transparent);
		border: 1px solid color-mix(in srgb, var(--border) 35%, transparent);
		border-radius: var(--radius);
	}
	.card:hover,
	.card:focus-visible {
		background: var(--bg-hover);
		border-color: var(--accent);
	}
	/* Room for the badge, so a long title wraps before reaching it. */
	.badged {
		padding-right: 6rem;
	}
	.badge {
		position: absolute;
		top: 0.4rem;
		right: 0.6rem;
		padding: 0 0.4rem;
		font-size: 0.65rem;
		color: var(--text-muted);
		border: 1px solid var(--border);
		border-radius: 999px;
	}
	.title {
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
		font-size: 0.9rem;
		color: var(--text);
	}
	.secondary {
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
		font-size: 0.7rem;
		color: var(--text-muted);
	}
	.meta {
		font-size: 0.7rem;
		color: var(--text-muted);
	}
	.terms {
		color: var(--info);
	}
</style>
