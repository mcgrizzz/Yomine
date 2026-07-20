<script lang="ts">
	// Staged edits; Save persists AND re-processes the loaded file so the new
	// filters take effect immediately (issue #92).
	import { untrack } from 'svelte';
	import {
		getTextFilterPresets,
		testTextFilters,
		type FilterPreset,
		type TextFilterSetting
	} from '$lib/ipc';
	import {
		fileResult,
		lastError,
		reloadCurrentFile,
		saveTextFilters,
		settings,
		textFiltersModalOpen
	} from '$lib/stores';

	let presets = $state<FilterPreset[]>([]);
	let stagedPresets = $state<Record<string, boolean>>({});
	let stagedFilters = $state<TextFilterSetting[]>([]);
	let original = $state('');
	let sample = $state('（田中）おはよう♪');
	let testResult = $state<string | null>(null);
	let testError = $state<string | null>(null);
	let saving = $state(false);

	const snapshot = () => JSON.stringify([stagedPresets, stagedFilters]);
	const dirty = $derived(snapshot() !== original);

	$effect(() => {
		if ($textFiltersModalOpen) untrack(hydrate);
	});

	function hydrate() {
		if (presets.length === 0) {
			getTextFilterPresets().then(
				(p) => (presets = p),
				() => {}
			);
		}
		stagedPresets = { ...($settings?.text_filter_presets ?? {}) };
		stagedFilters = ($settings?.text_filters ?? []).map((f) => ({ ...f }));
		original = snapshot();
	}

	// Live preview + validation, debounced. Runs even with an empty sample —
	// the command validates every enabled pattern before filtering.
	$effect(() => {
		if (!$textFiltersModalOpen) return;
		const staged = snapshot();
		const text = sample;
		const timer = setTimeout(() => {
			const [p, f] = JSON.parse(staged) as [Record<string, boolean>, TextFilterSetting[]];
			testTextFilters(p, f, text).then(
				(result) => {
					testResult = result;
					testError = null;
				},
				(err) => {
					testResult = null;
					testError = String(err);
				}
			);
		}, 250);
		return () => clearTimeout(timer);
	});

	function addFilter() {
		stagedFilters.push({ pattern: '', replacement: '', enabled: true });
	}

	async function save() {
		saving = true;
		try {
			const filters = stagedFilters.filter((f) => f.pattern.trim() !== '');
			if (!(await saveTextFilters(stagedPresets, filters))) return;
			stagedFilters = filters.map((f) => ({ ...f }));
			original = snapshot();
			if ($fileResult) await reloadCurrentFile();
			textFiltersModalOpen.set(false);
		} catch (err) {
			lastError.set({
				title: 'Text Filters',
				message: 'Saved, but reprocessing the loaded file failed',
				detail: String(err)
			});
		} finally {
			saving = false;
		}
	}

	function cancel() {
		const [p, f] = JSON.parse(original) as [Record<string, boolean>, TextFilterSetting[]];
		stagedPresets = p;
		stagedFilters = f;
	}
</script>

<!-- Esc closes from anywhere: the backdrop's own keydown only fires once focus
     is inside the modal, which it isn't right after opening from a menu. -->
<svelte:window
	onkeydown={(e) => $textFiltersModalOpen && e.key === 'Escape' && textFiltersModalOpen.set(false)}
/>

{#if $textFiltersModalOpen}
	<div
		class="backdrop"
		role="button"
		tabindex="-1"
		onclick={() => textFiltersModalOpen.set(false)}
		onkeydown={(e) => e.key === 'Escape' && textFiltersModalOpen.set(false)}
	>
		<!-- Stop backdrop clicks inside the dialog from closing it. -->
		<div
			class="dialog"
			role="dialog"
			aria-modal="true"
			aria-label="Text filters"
			tabindex="-1"
			onclick={(e) => e.stopPropagation()}
		>
			<header>
				<h2>Text Filters</h2>
				<button class="close" aria-label="Close" onclick={() => textFiltersModalOpen.set(false)}
					>✕</button
				>
			</header>

			<p class="blurb">
				Filters run on each line before terms and comprehension are computed. A line left empty is
				dropped entirely.
			</p>

			<section>
				<h3>Presets</h3>
				{#each presets as preset (preset.id)}
					<label class="preset">
						<input type="checkbox" bind:checked={stagedPresets[preset.id]} />
						<span>
							<span class="preset-label" lang="ja">{preset.label}</span>
							<span class="preset-desc" lang="ja">{preset.description}</span>
						</span>
					</label>
				{/each}
			</section>

			<section>
				<h3>Custom filters <span class="dim">(regex, applied in order)</span></h3>
				{#each stagedFilters as filter, i (i)}
					<div class="rule">
						<input
							type="checkbox"
							bind:checked={filter.enabled}
							aria-label="Enable this filter"
						/>
						<input
							class="mono"
							type="text"
							placeholder="pattern (regex)"
							bind:value={filter.pattern}
						/>
						<input
							class="mono"
							type="text"
							placeholder="replacement (empty = remove)"
							bind:value={filter.replacement}
						/>
						<button
							class="close"
							aria-label="Remove this filter"
							onclick={() => stagedFilters.splice(i, 1)}>✕</button
						>
					</div>
				{/each}
				<button class="add" onclick={addFilter}>+ Add filter</button>
			</section>

			<section>
				<h3>Test</h3>
				<input class="mono" type="text" lang="ja" placeholder="Sample line" bind:value={sample} />
				{#if testError}
					<p class="test-out error">{testError}</p>
				{:else if testResult !== null}
					<p class="test-out" lang="ja">
						→ {testResult === '' ? '(line dropped)' : testResult}
					</p>
				{/if}
			</section>

			<div class="status">
				{#if testError}⚠ Fix the invalid pattern to save{:else if dirty}⚠ Settings have been
					modified{/if}
			</div>

			<footer>
				<button disabled={!dirty || saving || testError !== null} onclick={save}>
					{saving ? 'Applying…' : $fileResult ? 'Save & Apply' : 'Save Settings'}
				</button>
				<button disabled={!dirty || saving} onclick={cancel}>Cancel</button>
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
		background: color-mix(in srgb, var(--bg-darker) 70%, transparent);
		z-index: 50;
	}
	.dialog {
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
		width: min(600px, 92%);
		max-height: 88vh;
		overflow-y: auto;
		padding-bottom: 0.75rem;
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
	.blurb {
		margin: 0;
		padding: 0 1rem;
		font-size: 0.85rem;
		color: var(--comment);
	}
	section {
		display: flex;
		flex-direction: column;
		gap: 0.4rem;
		padding: 0 1rem;
	}
	section h3 {
		margin: 0;
		font-size: 0.85rem;
		text-transform: uppercase;
		letter-spacing: 0.03em;
		color: var(--comment);
	}
	.dim {
		text-transform: none;
		letter-spacing: normal;
		font-weight: 400;
	}
	.preset {
		display: flex;
		align-items: baseline;
		gap: 0.5rem;
		cursor: pointer;
	}
	.preset-label {
		font-size: 0.9rem;
	}
	.preset-desc {
		display: block;
		font-size: 0.78rem;
		color: var(--comment);
	}
	.rule {
		display: grid;
		grid-template-columns: auto 1fr 1fr auto;
		align-items: center;
		gap: 0.4rem;
	}
	.mono {
		font-family: monospace;
		font-size: 0.85rem;
	}
	.add {
		align-self: flex-start;
		padding: 0.2rem 0.6rem;
		font-size: 0.85rem;
	}
	.test-out {
		margin: 0;
		font-size: 0.85rem;
		color: var(--green);
		overflow-wrap: anywhere;
	}
	.test-out.error {
		color: var(--red);
	}
	.status {
		min-height: 1.2rem;
		padding: 0 1rem;
		font-size: 0.85rem;
		color: var(--yellow);
	}
	footer {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0 1rem;
	}
	footer button:disabled {
		opacity: 0.5;
		cursor: default;
	}
</style>
