<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		form: { dirty: boolean; armed: boolean };
		/** Runs on Save, unless Save submits `submits`, a form's id. */
		onsave?: () => void;
		submits?: string;
		oncancel: () => void;
		onrestore?: () => void;
		/** Disables Save even when there are changes. */
		invalid?: boolean;
		/** Shown instead of the modified notice. */
		problem?: string | null;
		/** Disables every button while a save runs. */
		busy?: boolean;
		saveLabel?: string;
		/** Extra buttons, placed before Restore Default. */
		children?: Snippet;
	}

	let {
		form,
		onsave,
		submits,
		oncancel,
		onrestore,
		invalid = false,
		problem = null,
		busy = false,
		saveLabel = 'Save Settings',
		children
	}: Props = $props();
</script>

<hr />
<div class="status" role="status">
	{#if form.armed}⚠ Unsaved changes — dismiss again to discard{:else if problem}⚠ {problem}{:else if form.dirty}⚠
		Settings have been modified{/if}
</div>
<footer>
	<button
		class="primary"
		type={submits ? 'submit' : 'button'}
		form={submits}
		disabled={!form.dirty || invalid || busy}
		onclick={submits ? undefined : onsave}>{saveLabel}</button
	>
	<button disabled={!form.dirty || busy} onclick={oncancel}>Cancel</button>
	<div class="end">
		{@render children?.()}
		{#if onrestore}
			<button disabled={busy} onclick={onrestore}>Restore Default</button>
		{/if}
	</div>
</footer>

<style>
	hr {
		border: none;
		border-top: 1px solid var(--border);
		margin: 0 1rem;
	}
	.status {
		min-height: 1.2rem;
		padding: 0 1rem;
		font-size: 0.85rem;
		color: var(--warning);
	}
	footer {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.5rem;
		padding: 0 1rem;
	}
	.end {
		display: flex;
		gap: 0.5rem;
		margin-left: auto;
	}
</style>
