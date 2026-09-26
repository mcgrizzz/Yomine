<script lang="ts">
	// Staged edits, discarded on close/Cancel. The row right-click "Add to
	// ignore list" stays immediate (the stores' toggleIgnore), unlike this modal.
	import { settingsDraft } from '$lib/settingsDraft.svelte';
	import Modal from './Modal.svelte';
	import SettingsFooter from './SettingsFooter.svelte';
	import { ignoreModalOpen, saveIgnore } from '$lib/stores';
	import * as ipc from '$lib/ipc';
	import { textMatches } from '$lib/table';

	let newTerm = $state('');
	let searchFilter = $state('');
	let exportMessage = $state<{ ok: boolean; text: string } | null>(null);

	const form = settingsDraft({
		open: ignoreModalOpen,
		initial: { terms: [] as string[], files: [] as ipc.IgnoreFileView[] },
		load: async () => {
			newTerm = '';
			searchFilter = '';
			exportMessage = null;
			return await ipc.getIgnoreListFull();
		},
		// Only a file's path and enabled flag are saved, like egui.
		compared: ({ terms, files }) => [terms, files.map((f) => [f.path, f.enabled])]
	});
	const draft = $derived(form.value);

	const search = $derived(searchFilter.trim());
	const filteredTerms = $derived(
		search === '' ? draft.terms : draft.terms.filter((t) => textMatches(t, search))
	);
	// "From Files" count = terms across enabled files (egui's file_term_counts sum).
	const fileTermCount = $derived(
		draft.files.filter((f) => f.enabled).reduce((n, f) => n + f.term_count, 0)
	);

	function addTerm() {
		const term = newTerm.trim();
		if (term === '' || draft.terms.includes(term)) return;
		draft.terms = [...draft.terms, term];
		newTerm = '';
	}

	function removeTerm(term: string) {
		const i = draft.terms.indexOf(term);
		if (i !== -1) draft.terms = draft.terms.toSpliced(i, 1);
	}

	function toggleFile(i: number) {
		draft.files[i].enabled = !draft.files[i].enabled;
	}

	function removeFile(i: number) {
		draft.files = draft.files.toSpliced(i, 1);
	}

	async function refreshFile(i: number) {
		const v = await ipc.refreshIgnoreFile(draft.files[i].path);
		// Preserve the staged enabled; only the display metadata is refreshed.
		draft.files[i] = { ...draft.files[i], exists: v.exists, term_count: v.term_count };
	}

	async function importFile() {
		const v = await ipc.importIgnoreFile();
		if (v && !draft.files.some((f) => f.path === v.path)) draft.files = [...draft.files, v];
	}

	async function restoreDefault() {
		draft.terms = await ipc.getDefaultIgnoredTerms();
		draft.files = [];
	}

	async function exportTerms() {
		try {
			const path = await ipc.exportIgnoreList(draft.terms);
			exportMessage = path ? { ok: true, text: 'Terms exported successfully' } : null;
		} catch (err) {
			exportMessage = { ok: false, text: `Export failed: ${String(err)}` };
		}
	}

	async function save() {
		const saved = await saveIgnore(
			draft.terms,
			draft.files.map((f) => ({ path: f.path, enabled: f.enabled }))
		);
		if (!saved) return;
		form.commit();
		ignoreModalOpen.set(false);
	}

	function fileName(path: string): string {
		return path.split(/[\\/]/).pop() || path;
	}
</script>

<Modal
	open={$ignoreModalOpen}
	title="Ignore List"
	width="min(620px, 92%)"
	onclose={form.request}
	oninteract={form.disarm}
>
	<!-- Controls: add new term + search. -->
	<div class="controls">
		<div class="field">
			<label for="ignore-new-term">Add New Term</label>
			<div class="row">
				<input
					id="ignore-new-term"
					lang="ja"
					bind:value={newTerm}
					onkeydown={(e) => e.key === 'Enter' && addTerm()}
					placeholder="term…"
				/>
				<button onclick={addTerm}>Add</button>
			</div>
		</div>
		<div class="field">
			<label for="ignore-search">Search Terms</label>
			<input id="ignore-search" lang="ja" bind:value={searchFilter} placeholder="filter…" />
		</div>
	</div>

	<!-- Current terms: file pills + term pills. -->
	<div class="list">
		<div class="list-head">
			<span>Current Terms</span>
			<span class="counts">Manual: {draft.terms.length} | From Files: {fileTermCount}</span>
		</div>

		<div class="scroll">
			<div class="pills">
				{#each draft.files as file, i (file.path)}
					<span
						class="file-pill"
						class:enabled={file.enabled}
						class:missing={!file.exists}
						title={file.path}
					>
						<input
							type="checkbox"
							checked={file.enabled}
							aria-label="Enable {fileName(file.path)}"
							onchange={() => toggleFile(i)}
						/>
						<span class="file-name">📄 {fileName(file.path)}</span>
						{#if !file.exists}<span class="missing-tag">(missing)</span>{/if}
						<span class="file-count">{file.term_count}</span>
						<button class="icon" aria-label="Refresh {fileName(file.path)}" onclick={() => refreshFile(i)}>↻</button>
						<button class="icon remove" aria-label="Remove {fileName(file.path)}" onclick={() => removeFile(i)}>✕</button>
					</span>
				{/each}
				{#if search === ''}
					<button class="import-pill" onclick={importFile}>+ Import File</button>
				{/if}
			</div>

			{#if draft.files.length > 0 && filteredTerms.length > 0}
				<hr />
			{/if}

			{#if filteredTerms.length === 0 && draft.files.length === 0}
				<p class="empty">No terms found</p>
			{:else if filteredTerms.length > 0}
				<div class="pills">
					{#each filteredTerms as term (term)}
						<span class="term-pill">
							<span class="term" lang="ja">{term}</span>
							<button class="icon remove" aria-label="Remove {term}" onclick={() => removeTerm(term)}>✕</button>
						</span>
					{/each}
				</div>
			{/if}
		</div>
	</div>

	{#snippet footer()}
		{#if exportMessage}
			<p class="export-msg" class:ok={exportMessage.ok}>
				{exportMessage.ok ? '✓' : '⚠'} {exportMessage.text}
			</p>
		{/if}
		<SettingsFooter {form} onsave={save} oncancel={form.revert} onrestore={restoreDefault}>
			<button onclick={exportTerms}>Export…</button>
		</SettingsFooter>
	{/snippet}
</Modal>

<style>
	.controls {
		display: flex;
		gap: 1rem;
		padding: 0 1rem;
	}
	.field {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
	}
	.field:first-child {
		flex: 1;
	}
	.field label {
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	.field .row {
		display: flex;
		gap: 0.4rem;
	}
	.field input {
		padding: 0.3rem 0.5rem;
		background: var(--bg-raised);
		color: var(--text);
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
	}
	.field .row input {
		flex: 1;
	}
	.list {
		display: flex;
		flex-direction: column;
		margin: 0 1rem;
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
	}
	.list-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.4rem 0.6rem;
		border-bottom: 1px solid var(--border);
		font-size: 0.85rem;
	}
	.list-head .counts {
		color: var(--text-muted);
	}
	.scroll {
		overflow-y: auto;
		max-height: 260px;
		padding: 0.6rem;
	}
	.pills {
		display: flex;
		flex-wrap: wrap;
		gap: 0.4rem;
	}
	.scroll hr {
		border: none;
		border-top: 1px solid var(--border);
		margin: 0.6rem 0;
	}
	.file-pill,
	.term-pill {
		display: inline-flex;
		align-items: center;
		gap: 0.3rem;
		padding: 0.3rem 0.5rem;
		background: var(--bg-raised);
		border: 1px solid var(--border);
		border-radius: 4px;
	}
	.file-pill.enabled {
		border-color: var(--accent);
	}
	.file-pill.missing {
		opacity: 0.6;
	}
	.file-name {
		font-size: 0.85rem;
	}
	.missing-tag {
		color: var(--danger);
		font-size: 0.8rem;
	}
	.file-count {
		color: var(--text-muted);
		font-size: 0.8rem;
	}
	.term {
		font-size: 1.05rem;
		color: var(--text);
	}
	.icon {
		padding: 0 0.25rem;
		min-width: 24px;
		min-height: 24px;
		background: none;
		border: none;
		color: var(--text);
		cursor: pointer;
	}
	.icon.remove {
		color: var(--danger);
	}
	.import-pill {
		padding: 0.3rem 0.5rem;
		background: var(--bg-raised);
		border: 1px dashed var(--border);
		border-radius: 4px;
		color: var(--text);
		cursor: pointer;
	}
	.empty {
		margin: 0;
		padding: 0.5rem;
		color: var(--text-muted);
		text-align: center;
	}
	.export-msg {
		margin: 0;
		padding: 0 1rem;
		font-size: 0.85rem;
		color: var(--danger);
	}
	.export-msg.ok {
		color: var(--success);
	}
</style>
