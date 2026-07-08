<script lang="ts">
	// Staged edits, discarded on close/Cancel. The row right-click "Add to
	// ignore list" stays immediate (the stores' toggleIgnore), unlike this modal.
	import { ignoreModalOpen, saveIgnore } from '$lib/stores';
	import * as ipc from '$lib/ipc';
	import { textMatches } from '$lib/table';

	// Staged state + the snapshot it's diffed against for the dirty indicator.
	let tempTerms = $state<string[]>([]);
	let tempFiles = $state<ipc.IgnoreFileView[]>([]);
	let originalTerms = $state<string[]>([]);
	let originalFiles = $state<ipc.IgnoreFileView[]>([]);

	let newTerm = $state('');
	let searchFilter = $state('');
	let exportMessage = $state<{ ok: boolean; text: string } | null>(null);

	// Hydrate (egui's open_modal) each time the modal opens; reset on close.
	$effect(() => {
		if ($ignoreModalOpen) hydrate();
	});

	async function hydrate() {
		newTerm = '';
		searchFilter = '';
		exportMessage = null;
		const view = await ipc.getIgnoreListFull();
		tempTerms = view.terms;
		tempFiles = view.files;
		originalTerms = [...view.terms];
		originalFiles = view.files.map((f) => ({ ...f }));
	}

	// Compare only the persisted fields (path + enabled) for files, like egui.
	const fileKey = (f: ipc.IgnoreFile[]) => JSON.stringify(f.map((x) => [x.path, x.enabled]));
	const dirty = $derived(
		JSON.stringify(tempTerms) !== JSON.stringify(originalTerms) ||
			fileKey(tempFiles) !== fileKey(originalFiles)
	);

	const search = $derived(searchFilter.trim());
	const filteredTerms = $derived(
		search === '' ? tempTerms : tempTerms.filter((t) => textMatches(t, search))
	);
	// "From Files" count = terms across enabled files (egui's file_term_counts sum).
	const fileTermCount = $derived(
		tempFiles.filter((f) => f.enabled).reduce((n, f) => n + f.term_count, 0)
	);

	function addTerm() {
		const term = newTerm.trim();
		if (term === '' || tempTerms.includes(term)) return;
		tempTerms = [...tempTerms, term];
		newTerm = '';
	}

	function removeTerm(term: string) {
		const i = tempTerms.indexOf(term);
		if (i !== -1) tempTerms = tempTerms.toSpliced(i, 1);
	}

	function toggleFile(i: number) {
		tempFiles[i].enabled = !tempFiles[i].enabled;
	}

	function removeFile(i: number) {
		tempFiles = tempFiles.toSpliced(i, 1);
	}

	async function refreshFile(i: number) {
		const v = await ipc.refreshIgnoreFile(tempFiles[i].path);
		// Preserve the staged enabled; only the display metadata is refreshed.
		tempFiles[i] = { ...tempFiles[i], exists: v.exists, term_count: v.term_count };
	}

	async function importFile() {
		const v = await ipc.importIgnoreFile();
		if (v && !tempFiles.some((f) => f.path === v.path)) tempFiles = [...tempFiles, v];
	}

	async function restoreDefault() {
		tempTerms = await ipc.getDefaultIgnoredTerms();
		tempFiles = [];
	}

	async function exportTerms() {
		try {
			const path = await ipc.exportIgnoreList(tempTerms);
			exportMessage = path ? { ok: true, text: 'Terms exported successfully' } : null;
		} catch (err) {
			exportMessage = { ok: false, text: `Export failed: ${String(err)}` };
		}
	}

	async function save() {
		await saveIgnore(
			tempTerms,
			tempFiles.map((f) => ({ path: f.path, enabled: f.enabled }))
		);
		originalTerms = [...tempTerms];
		originalFiles = tempFiles.map((f) => ({ ...f }));
		ignoreModalOpen.set(false);
	}

	// egui's Cancel reverts staged edits but keeps the modal open.
	function cancel() {
		tempTerms = [...originalTerms];
		tempFiles = originalFiles.map((f) => ({ ...f }));
	}

	function fileName(path: string): string {
		return path.split(/[\\/]/).pop() || path;
	}
</script>

<!-- Esc closes from anywhere: the backdrop's own keydown only fires once focus
     is inside the modal, which it isn't right after opening from a menu. -->
<svelte:window
	onkeydown={(e) => $ignoreModalOpen && e.key === 'Escape' && ignoreModalOpen.set(false)}
/>

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
					<span class="counts">Manual: {tempTerms.length} | From Files: {fileTermCount}</span>
				</div>

				<div class="scroll">
					<div class="pills">
						{#each tempFiles as file, i (file.path)}
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

					{#if tempFiles.length > 0 && filteredTerms.length > 0}
						<hr />
					{/if}

					{#if filteredTerms.length === 0 && tempFiles.length === 0}
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

			<div class="status">
				{#if dirty}⚠ Settings have been modified{/if}
			</div>

			{#if exportMessage}
				<p class="export-msg" class:ok={exportMessage.ok}>
					{exportMessage.ok ? '✓' : '⚠'} {exportMessage.text}
				</p>
			{/if}

			<footer>
				<button disabled={!dirty} onclick={save}>Save Settings</button>
				<button disabled={!dirty} onclick={cancel}>Cancel</button>
				<button onclick={exportTerms}>Export…</button>
				<button class="right" onclick={restoreDefault}>Restore Default</button>
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
		width: min(620px, 92%);
		max-height: 82%;
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
		color: var(--comment);
	}
	.field .row {
		display: flex;
		gap: 0.4rem;
	}
	.field input {
		padding: 0.3rem 0.5rem;
		background: var(--bg-light);
		color: var(--fg);
		border: 1px solid var(--border);
		border-radius: 3px;
	}
	.field .row input {
		flex: 1;
	}
	.list {
		display: flex;
		flex-direction: column;
		margin: 0 1rem;
		border: 1px solid var(--border);
		border-radius: 3px;
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
		color: var(--comment);
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
		background: var(--bg-light);
		border: 1px solid var(--border);
		border-radius: 4px;
	}
	.file-pill.enabled {
		border-color: var(--cyan);
	}
	.file-pill.missing {
		opacity: 0.6;
	}
	.file-name {
		font-size: 0.85rem;
	}
	.missing-tag {
		color: var(--red);
		font-size: 0.8rem;
	}
	.file-count {
		color: var(--comment);
		font-size: 0.8rem;
	}
	.term {
		font-size: 1.05rem;
		color: var(--fg);
	}
	.icon {
		padding: 0 0.25rem;
		background: none;
		border: none;
		color: var(--fg);
		cursor: pointer;
	}
	.icon.remove {
		color: var(--red);
	}
	.import-pill {
		padding: 0.3rem 0.5rem;
		background: var(--bg-light);
		border: 1px dashed var(--border);
		border-radius: 4px;
		color: var(--fg);
		cursor: pointer;
	}
	.empty {
		margin: 0;
		padding: 0.5rem;
		color: var(--comment);
		text-align: center;
	}
	.status {
		min-height: 1.2rem;
		padding: 0 1rem;
		font-size: 0.85rem;
		color: var(--yellow);
	}
	.export-msg {
		margin: 0;
		padding: 0 1rem;
		font-size: 0.85rem;
		color: var(--red);
	}
	.export-msg.ok {
		color: var(--green);
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
	button:disabled {
		opacity: 0.5;
		cursor: default;
	}
</style>
