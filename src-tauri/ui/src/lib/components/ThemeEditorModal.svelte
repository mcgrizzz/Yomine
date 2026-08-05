<script lang="ts">
	// Create/edit user themes with live preview: edits apply to :root while the
	// modal is open and revert to the saved theme on close.
	import { untrack } from 'svelte';
	import { emit } from '@tauri-apps/api/event';
	import { exportThemeFile, importThemeFile, type UserTheme } from '$lib/ipc';
	import { saveUserThemes, settings } from '$lib/stores';
	import {
		allThemes,
		applyTheme,
		BUILTIN_THEMES,
		mergeColors,
		resolveTheme,
		themeFromUser,
		TOKEN_GROUPS,
		TOKENS,
		userThemeId,
		type ThemeColors
	} from '$lib/themes';

	let {
		open = $bindable(),
		initial
	}: {
		open: boolean;
		/** User theme name to edit; null = create a new theme. */
		initial: string | null;
	} = $props();

	let name = $state('');
	let dark = $state(true);
	let colors = $state<ThemeColors>({ ...BUILTIN_THEMES[0].colors });
	let originalName = $state<string | null>(null);
	let seedId = $state('dracula');
	let deleteArmed = $state(false);
	let fileError = $state<string | null>(null);

	$effect(() => {
		if (open) untrack(hydrate);
	});

	function hydrate() {
		deleteArmed = false;
		fileError = null;
		const existing = initial
			? ($settings?.user_themes ?? []).find((t) => t.name === initial)
			: undefined;
		if (existing) {
			originalName = existing.name;
			name = existing.name;
			dark = existing.dark;
			colors = { ...themeFromUser(existing).colors };
		} else {
			originalName = null;
			name = '';
			seedId = resolveTheme($settings).id;
			seed(seedId);
		}
	}

	function seed(id: string) {
		const from = allThemes($settings).find((t) => t.id === id) ?? BUILTIN_THEMES[0];
		dark = from.dark;
		colors = { ...from.colors };
	}

	// Preview locally AND broadcast so the main window shows the edit too.
	$effect(() => {
		if (!open) return;
		const preview = { id: 'preview', label: name, dark, colors: { ...colors } };
		applyTheme(preview);
		void emit('theme-preview', preview);
	});

	const themeJson = () => JSON.stringify({ name: name.trim(), dark, colors }, null, '\t') + '\n';

	async function saveToFile() {
		fileError = null;
		try {
			await exportThemeFile(trimmed || 'theme', themeJson());
		} catch (e) {
			fileError = String(e);
		}
	}

	async function loadFromFile() {
		fileError = null;
		try {
			const text = await importThemeFile();
			if (text !== null) applyThemeJson(text);
		} catch (e) {
			fileError = String(e);
		}
	}

	function applyThemeJson(text: string) {
		try {
			const parsed = JSON.parse(text);
			const parsedName = parsed.name ?? parsed.label;
			if (parsedName !== undefined && typeof parsedName !== 'string')
				throw new Error('"name" must be a string');
			if (parsed.dark !== undefined && typeof parsed.dark !== 'boolean')
				throw new Error('"dark" must be true or false');
			if (typeof parsed.colors !== 'object' || parsed.colors === null)
				throw new Error('missing "colors" object');
			const clean: Record<string, string> = {};
			for (const [token, value] of Object.entries(parsed.colors)) {
				if (!(TOKENS as readonly string[]).includes(token)) continue;
				if (typeof value !== 'string' || !/^#[0-9a-fA-F]{6}$/.test(value))
					throw new Error(`"${token}" must be a #rrggbb hex color`);
				clean[token] = value.toLowerCase();
			}
			if (parsedName) name = parsedName;
			if (parsed.dark !== undefined) dark = parsed.dark;
			colors = mergeColors(dark, clean);
		} catch (e) {
			fileError =
				e instanceof SyntaxError ? 'Not a valid JSON file' : String((e as Error).message ?? e);
		}
	}

	const trimmed = $derived(name.trim());
	const taken = $derived(
		BUILTIN_THEMES.some((t) => t.label.toLowerCase() === trimmed.toLowerCase()) ||
			($settings?.user_themes ?? []).some(
				(t) => t.name === trimmed && t.name !== originalName
			)
	);
	const invalid = $derived(trimmed === '' || taken);

	function close() {
		open = false;
		applyTheme(resolveTheme($settings));
		void emit('theme-preview', null);
	}

	async function save() {
		if (invalid) return;
		const s = $settings;
		if (!s) return;
		const theme: UserTheme = { name: trimmed, dark, colors: { ...colors } };
		const list = originalName
			? s.user_themes.map((t) => (t.name === originalName ? theme : t))
			: [...s.user_themes, theme];
		// A rename must follow the theme into any preferred slot referencing it.
		const slots: { theme_dark?: string; theme_light?: string } = {};
		if (originalName && originalName !== trimmed) {
			const oldId = userThemeId(originalName);
			if (s.theme_dark === oldId) slots.theme_dark = userThemeId(trimmed);
			if (s.theme_light === oldId) slots.theme_light = userThemeId(trimmed);
		}
		await saveUserThemes(list, slots);
		close();
	}

	async function remove() {
		if (!deleteArmed) {
			deleteArmed = true;
			return;
		}
		const s = $settings;
		if (!s || !originalName) return;
		const id = userThemeId(originalName);
		const slots: { theme_dark?: string; theme_light?: string } = {};
		if (s.theme_dark === id) slots.theme_dark = 'dracula';
		if (s.theme_light === id) slots.theme_light = 'paper';
		await saveUserThemes(
			s.user_themes.filter((t) => t.name !== originalName),
			slots
		);
		close();
	}
</script>

<svelte:window
	onkeydown={(e) => open && e.key === 'Escape' && close()}
	onbeforeunload={() => open && void emit('theme-preview', null)}
/>

{#if open}
	<div
		class="backdrop"
		role="button"
		tabindex="-1"
		onclick={close}
		onkeydown={(e) => e.key === 'Escape' && close()}
	>
		<div
			class="dialog"
			role="dialog"
			aria-modal="true"
			aria-label="Theme editor"
			tabindex="-1"
			onclick={(e) => e.stopPropagation()}
		>
			<header>
				<h2>{originalName ? 'Edit Theme' : 'New Theme'}</h2>
				<button class="close" aria-label="Close" onclick={close}>✕</button>
			</header>

			<div class="row">
				<label for="theme-name">Name:</label>
				<input id="theme-name" type="text" bind:value={name} placeholder="My theme" />
			</div>
			{#if taken}
				<p class="error">A theme with this name already exists.</p>
			{/if}

			{#if !originalName}
				<div class="row">
					<label for="theme-seed">Start from:</label>
					<select
						id="theme-seed"
						bind:value={seedId}
						onchange={() => seed(seedId)}
					>
						{#each allThemes($settings) as t (t.id)}
							<option value={t.id}>{t.label}</option>
						{/each}
					</select>
				</div>
			{/if}

			<label class="row dark-toggle">
				<input type="checkbox" bind:checked={dark} />
				Dark theme (sets native control styling and which toggle slot it can fill)
			</label>

			<div class="row file-row">
				<button onclick={saveToFile}>Save to File…</button>
				<button onclick={loadFromFile}>Load from File…</button>
			</div>
			{#if fileError}
				<p class="error">{fileError}</p>
			{/if}

			<div class="groups">
				{#each TOKEN_GROUPS as group (group.label)}
					<div class="group">
						<h3>{group.label}</h3>
						<div class="tokens">
							{#each group.tokens as token (token)}
								<label class="token">
									<input type="color" bind:value={colors[token]} />
									<span>{token}</span>
								</label>
							{/each}
						</div>
					</div>
				{/each}
			</div>

			<footer>
				<button disabled={invalid} onclick={save}>Save Theme</button>
				<button onclick={close}>Cancel</button>
				{#if originalName}
					<button class="right danger" onclick={remove}>
						{deleteArmed ? 'Really delete?' : 'Delete'}
					</button>
				{/if}
			</footer>
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
		background: color-mix(in srgb, var(--bg-deep) 70%, transparent);
		z-index: 60;
	}
	.dialog {
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
		width: min(460px, 92%);
		max-height: 90%;
		overflow-y: auto;
		padding-bottom: 0.75rem;
		background: var(--bg-panel);
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
		color: var(--accent);
	}
	.close {
		padding: 0.1rem 0.4rem;
	}
	.row {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0 1rem;
	}
	.row input[type='text'],
	.row select {
		flex: 1;
	}
	.dark-toggle {
		cursor: pointer;
		font-size: 0.85rem;
		color: var(--text-muted);
	}
	.file-row button {
		padding: 0.2rem 0.6rem;
		font-size: 0.8rem;
	}
	.error {
		margin: 0;
		padding: 0 1rem;
		font-size: 0.85rem;
		color: var(--danger);
	}
	.groups {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		padding: 0 1rem;
	}
	.group h3 {
		margin: 0 0 0.3rem;
		font-size: 0.85rem;
		color: var(--text-muted);
	}
	.tokens {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: 0.25rem 1rem;
	}
	.token {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
		cursor: pointer;
		font-size: 0.85rem;
		font-family: monospace;
	}
	.token input[type='color'] {
		width: 2rem;
		height: 1.4rem;
		padding: 0;
		border: 1px solid var(--border);
		border-radius: 3px;
		background: none;
		cursor: pointer;
	}
	footer {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0 1rem;
	}
	footer .right {
		margin-left: auto;
	}
	footer .danger {
		color: var(--danger);
	}
	button:disabled {
		opacity: 0.5;
		cursor: default;
	}
</style>
