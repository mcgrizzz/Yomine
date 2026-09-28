<script lang="ts">
	import { closeQueue, currentQueue, goToQueued, playerBusy } from '$lib/stores';
	import { filename } from '$lib/recents';

	let listOpen = $state(false);

	// Rows show only the episode when every video names the same show.
	const show = $derived.by(() => {
		const first = $currentQueue?.videos[0].show;
		return first && $currentQueue?.videos.every((v) => v.show === first && v.episode)
			? first
			: null;
	});

	function go(index: number) {
		listOpen = false;
		void goToQueued(index);
	}
</script>

<svelte:window
	onclick={() => (listOpen = false)}
	onkeydown={(e) => e.key === 'Escape' && (listOpen = false)}
/>

{#if $currentQueue}
	{@const { videos, index } = $currentQueue}
	<span class="stepper">
		<button
			class="step"
			aria-label="Previous video"
			title="Previous video"
			disabled={$playerBusy || index === 0}
			onclick={() => go(index - 1)}>‹</button
		>
		<span class="wrap">
			<button
				class="count"
				aria-haspopup="menu"
				aria-expanded={listOpen}
				title="Videos in the queue"
				onclick={(e) => {
					e.stopPropagation();
					listOpen = !listOpen;
				}}>{index + 1}/{videos.length}</button
			>
			{#if listOpen}
				<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions --
				     the handler only fences the window's close-on-outside-click listener. -->
				<div class="list" role="menu" tabindex="-1" onclick={(e) => e.stopPropagation()}>
					{#if show}<p class="show" title={show}>{show}</p>{/if}
					<div class="items">
						{#each videos as video, i (video.path)}
							<button
								class="item"
								class:current={i === index}
								role="menuitem"
								title={video.path}
								disabled={$playerBusy}
								onclick={() => go(i)}
							>
								<span class="n">{i === index ? '▸' : ''} {i + 1}</span>
								<span class="name">{show ? video.episode : filename(video.path)}</span>
							</button>
						{/each}
					</div>
					<button class="item close" role="menuitem" onclick={closeQueue}>Close queue</button>
				</div>
			{/if}
		</span>
		<button
			class="step"
			aria-label="Next video"
			title="Next video"
			disabled={$playerBusy || index === videos.length - 1}
			onclick={() => go(index + 1)}>›</button
		>
	</span>
{/if}

<style>
	.stepper {
		display: inline-flex;
		align-items: center;
		flex-shrink: 0;
		font-size: 0.8rem;
	}
	.stepper button {
		font-family: inherit;
		color: var(--text-muted);
		background: transparent;
		border: 1px solid transparent;
		border-radius: var(--radius);
	}
	.stepper button:hover:not(:disabled) {
		color: var(--text);
		background: var(--bg-hover);
	}
	.step {
		padding: 0 0.35rem;
		font-size: 1rem;
		line-height: 1.2;
	}
	.count {
		padding: 0.05rem 0.3rem;
		font-variant-numeric: tabular-nums;
	}
	.wrap {
		position: relative;
	}
	.list {
		position: absolute;
		top: calc(100% + 0.4rem);
		left: 0;
		z-index: var(--z-toast);
		display: flex;
		flex-direction: column;
		width: max-content;
		max-width: 28rem;
		padding: 0.3rem;
		background: var(--bg-raised);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		box-shadow: var(--shadow-overlay);
	}
	.show {
		margin: 0;
		padding: 0.3rem 0.5rem 0.4rem;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-weight: 600;
		color: var(--text);
		border-bottom: 1px solid var(--border);
	}
	.items {
		max-height: 60vh;
		overflow-y: auto;
	}
	.stepper .item {
		display: flex;
		gap: 0.6rem;
		width: 100%;
		padding: 0.3rem 0.5rem;
		text-align: left;
		color: var(--text);
	}
	.stepper .item.current {
		color: var(--accent);
	}
	.n {
		min-width: 2.2rem;
		text-align: right;
		color: var(--text-muted);
		font-variant-numeric: tabular-nums;
	}
	.current .n {
		color: var(--accent);
	}
	.name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.stepper .close {
		margin-top: 0.2rem;
		padding-top: 0.4rem;
		color: var(--text-muted);
		border-top: 1px solid var(--border);
		border-radius: 0;
	}
</style>
