<script lang="ts">
	// Setup-incomplete banner (T045): parity with src/gui/setup_banner.rs. A clickable
	// amber strip shown when the freq dict is missing OR Anki model mappings are empty
	// (egui's should_show_banner). Clicking opens the setup checklist modal. The
	// show condition is the backend-derived `showSetupBanner` store.
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
		/* egui's amber rgb(180,140,0) on white text. */
		background: rgb(180, 140, 0);
		color: #fff;
		font-size: 0.875rem;
		text-align: center;
		cursor: pointer;
	}
	.banner:hover {
		background: rgb(200, 156, 0);
	}
</style>
