<script lang="ts">
	// Standalone Themes window (open_themes_window): no backdrop, draggable via
	// its native title bar, so theme changes are visible on the main window.
	// Picks are instant (like the top-bar toggle) — no staged Save.
	import { onMount } from 'svelte';
	import * as ipc from '$lib/ipc';
	import { settings, setPreferredTheme } from '$lib/stores';
	import { allThemes, resolveTheme, type Theme, type TokenName } from '$lib/themes';
	import ThemeEditorModal from '$lib/components/ThemeEditorModal.svelte';

	onMount(async () => {
		settings.set(await ipc.getSettings());
	});

	const themes = $derived(allThemes($settings));
	const activeThemeId = $derived(resolveTheme($settings).id);
	const preferredDark = $derived($settings?.theme_dark ?? 'dracula');
	const preferredLight = $derived($settings?.theme_light ?? 'paper');
	const SURFACES: TokenName[] = ['bg-deep', 'bg-panel', 'bg', 'bg-raised', 'bg-hover'];
	const SWATCHES: TokenName[] = ['accent', 'danger', 'success', 'info'];

	const pick = (t: Theme) => void setPreferredTheme(t.dark ? 'dark' : 'light', t.id);

	let editorOpen = $state(false);
	let editorInitial = $state<string | null>(null);
	function openEditor(name: string | null) {
		editorInitial = name;
		editorOpen = true;
	}
</script>

<div class="page">
	<div class="theme-grid">
		{#each themes as theme (theme.id)}
			<!-- svelte-ignore a11y_no_static_element_interactions -- role/tabindex/keydown
			     are present; a real <button> can't nest the edit button. -->
			<div
				class="theme-card"
				class:current={theme.id === activeThemeId}
				role="button"
				tabindex="0"
				style="background: {theme.colors.bg}; border-color: {theme.id === activeThemeId
					? theme.colors.accent
					: theme.colors.border}"
				onclick={() => pick(theme)}
				onkeydown={(e) => e.key === 'Enter' && pick(theme)}
			>
				<span class="strip">
					{#each SURFACES as s (s)}<span style="background: {theme.colors[s]}"></span>{/each}
				</span>
				{#if theme.id === preferredDark || theme.id === preferredLight}
					<span
						class="slot-badge"
						title={theme.dark ? 'Your dark theme' : 'Your light theme'}
						style="background: {theme.colors.accent}; color: {theme.colors.bg}"
						>{theme.dark ? 'DARK' : 'LIGHT'}</span
					>
				{/if}
				<span class="card-main">
					<span class="card-name" style="color: {theme.colors.accent}">{theme.label}</span>
					<span class="chips">
						{#each SWATCHES as c (c)}
							<span class="chip" style="background: {theme.colors[c]}; color: {theme.colors.bg}"
								>あ</span
							>
						{/each}
					</span>
				</span>
				{#if theme.id.startsWith('user:')}
					<button
						class="edit"
						title="Edit theme"
						onclick={(e) => {
							e.stopPropagation();
							openEditor(theme.label);
						}}>✎</button
					>
				{/if}
			</div>
		{/each}
		<button class="theme-card new" onclick={() => openEditor(null)}>＋ New theme</button>
	</div>
</div>

<ThemeEditorModal bind:open={editorOpen} initial={editorInitial} />

<style>
	.page {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		height: 100%;
		padding: 1rem;
		overflow-y: auto;
	}
	.theme-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
		gap: 0.5rem;
	}
	/* Cards render in their OWN theme's colors (inline styles), not the active one. */
	.theme-card {
		position: relative;
		display: flex;
		align-items: stretch;
		gap: 0.5rem;
		padding: 0;
		overflow: hidden;
		border: 2px solid;
		border-radius: var(--radius);
		cursor: pointer;
		text-align: left;
	}
	.strip {
		display: flex;
		flex-direction: column;
		width: 8px;
		flex: none;
	}
	.strip span {
		flex: 1;
	}
	.card-main {
		display: flex;
		flex-direction: column;
		gap: 0.3rem;
		padding: 0.45rem 0.5rem 0.5rem 0;
		min-width: 0;
	}
	.card-name {
		font-size: 0.85rem;
		font-weight: 700;
		overflow-wrap: anywhere;
	}
	.chips {
		display: flex;
		gap: 0.3rem;
	}
	.chip {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 1.3rem;
		height: 1.3rem;
		border-radius: 4px;
		font-size: 0.75rem;
		line-height: 1;
	}
	.slot-badge {
		position: absolute;
		bottom: 0.3rem;
		right: 0.3rem;
		padding: 0.1rem 0.4rem;
		border-radius: 999px;
		font-size: 0.6rem;
		font-weight: 700;
		letter-spacing: 0.05em;
	}
	.theme-card .edit {
		position: absolute;
		bottom: 0.25rem;
		right: 0.25rem;
		padding: 0 0.3rem;
		font-size: 0.75rem;
		background: none;
		border: none;
		opacity: 0.6;
	}
	.theme-card .edit:hover {
		opacity: 1;
	}
	.theme-card.new {
		align-items: center;
		justify-content: center;
		padding: 0.45rem;
		background: var(--bg-raised);
		border: 2px dashed var(--border);
		color: var(--text-muted);
		font-size: 0.85rem;
	}
	.hint {
		margin: 0;
		font-size: 0.85rem;
		color: var(--text-muted);
	}
</style>
