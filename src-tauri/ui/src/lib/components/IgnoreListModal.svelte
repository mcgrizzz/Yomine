<script lang="ts">
	// Ignore-list modal (T038): lists the ignored lemmas (get_ignore_list) and lets
	// you remove them (remove_from_ignore_list); each removal re-filters the loaded
	// table in place. Terms are added from a row's right-click menu, not here.
	import { ignoreList, ignoreModalOpen, removeFromIgnore } from '$lib/stores';
</script>

{#if $ignoreModalOpen}
	<div
		class="backdrop"
		role="button"
		tabindex="-1"
		onclick={() => ignoreModalOpen.set(false)}
		onkeydown={(e) => e.key === 'Escape' && ignoreModalOpen.set(false)}
	>
		<!-- Stop backdrop clicks inside the dialog from closing it. -->
		<div
			class="dialog"
			role="dialog"
			aria-modal="true"
			aria-label="Ignore list"
			tabindex="-1"
			onclick={(e) => e.stopPropagation()}
		>
			<header>
				<h2>Ignore List</h2>
				<button class="close" aria-label="Close" onclick={() => ignoreModalOpen.set(false)}
					>✕</button
				>
			</header>

			{#if $ignoreList.length === 0}
				<p class="empty">No ignored terms.</p>
			{:else}
				<ul>
					{#each $ignoreList as term (term)}
						<li>
							<span class="term" lang="ja">{term}</span>
							<button
								class="remove"
								aria-label="Remove {term}"
								onclick={() => removeFromIgnore(term)}>✕</button
							>
						</li>
					{/each}
				</ul>
			{/if}
		</div>
	</div>
{/if}

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		background: color-mix(in srgb, var(--bg-darker) 70%, transparent);
		z-index: 50;
	}
	.dialog {
		display: flex;
		flex-direction: column;
		width: min(420px, 90vw);
		max-height: 70vh;
		background: var(--bg-dark);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
	}
	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.75rem 1rem;
		border-bottom: 1px solid var(--border);
	}
	header h2 {
		margin: 0;
		font-size: 1.05rem;
		color: var(--cyan);
	}
	.close {
		padding: 0.1rem 0.4rem;
	}
	.empty {
		margin: 0;
		padding: 1.5rem 1rem;
		color: var(--comment);
		text-align: center;
	}
	ul {
		list-style: none;
		margin: 0;
		padding: 0.5rem;
		overflow-y: auto;
		display: flex;
		flex-direction: column;
		gap: 0.3rem;
	}
	li {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.4rem 0.6rem;
		background: var(--bg-light);
		border: 1px solid var(--border);
		border-radius: 3px;
	}
	.term {
		font-size: 1.1rem;
		color: var(--fg);
	}
	.remove {
		padding: 0.1rem 0.4rem;
		color: var(--red);
	}
</style>
