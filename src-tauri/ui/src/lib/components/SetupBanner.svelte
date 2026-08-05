<script lang="ts">
	import { showSetupBanner, openSetupModal } from '$lib/stores';
</script>

{#if $showSetupBanner}
	<button class="banner" onclick={openSetupModal}>
		⚠ Setup Incomplete - Click to view setup checklist
	</button>
{/if}

<style>
	.banner {
		display: block;
		width: 100%;
		padding: 0.35rem 1rem;
		border: none;
		border-radius: 0;
		background: color-mix(in srgb, var(--warning) 55%, var(--bg));
		color: var(--text);
		font-size: 0.875rem;
		text-align: center;
		cursor: pointer;
	}
	.banner:hover {
		background: color-mix(in srgb, var(--warning) 70%, var(--bg));
	}
</style>
