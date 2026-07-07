<script lang="ts">
	// State machine: selecting → analyzing → results → exporting → complete|error.
	// Starts fresh each open; the full result for export lives backend-side.
	// The flow is driven off the command promises — the analysis/export events
	// are intentionally NOT subscribed, handling both would double-fire.
	import { open as openDialog } from '@tauri-apps/plugin-dialog';
	import * as ipc from '$lib/ipc';
	import { analyzerModalOpen } from '$lib/stores';
	import { buildFileTree, collectChecked, type TreeNode } from '$lib/fileTree';

	type Phase = 'selecting' | 'analyzing' | 'results' | 'exporting' | 'complete' | 'error';

	let phase = $state<Phase>('selecting');

	// --- Selection ------------------------------------------------------------
	// The accumulated set of file paths (dedup across multiple Add actions), and
	// the per-path checked flag (default everything selected).
	let selectedPaths = $state<string[]>([]);
	let checked = $state<Record<string, boolean>>({});
	let collapsed = $state<Record<string, boolean>>({});

	const tree = $derived(buildFileTree(selectedPaths));
	const checkedPaths = $derived(selectedPaths.filter((p) => checked[p] !== false));
	const checkedCount = $derived(checkedPaths.length);

	function addPaths(paths: string[]) {
		const existing = new Set(selectedPaths);
		const fresh = paths.filter((p) => !existing.has(p));
		if (fresh.length === 0) return;
		selectedPaths = [...selectedPaths, ...fresh];
		checked = { ...checked, ...Object.fromEntries(fresh.map((p) => [p, true])) };
	}

	async function addFiles() {
		const picked = await openDialog({
			multiple: true,
			filters: [{ name: 'Subtitles/Text', extensions: ['srt', 'ass', 'ssa', 'txt'] }]
		});
		if (!picked) return;
		addPaths(Array.isArray(picked) ? picked : [picked]);
	}

	async function addFolder() {
		const dir = await openDialog({ directory: true });
		if (!dir || Array.isArray(dir)) return;
		const files = await ipc.findAnalysisFiles(dir);
		addPaths(files);
	}

	function clearAll() {
		selectedPaths = [];
		checked = {};
		collapsed = {};
	}

	function toggleNode(node: TreeNode, value: boolean) {
		// Cascade the checkbox to every file under a node (and itself if a file).
		const next = { ...checked };
		const apply = (n: TreeNode) => {
			if (n.path) next[n.path] = value;
			n.children.forEach(apply);
		};
		apply(node);
		checked = next;
	}

	/** A node is checked when all its files are checked; indeterminate handled in template. */
	function nodeChecked(node: TreeNode): boolean {
		const files = collectChecked(node, () => true);
		return files.length > 0 && files.every((p) => checked[p] !== false);
	}
	function nodePartial(node: TreeNode): boolean {
		const files = collectChecked(node, () => true);
		const some = files.some((p) => checked[p] !== false);
		return some && !files.every((p) => checked[p] !== false);
	}

	// --- Analysis -------------------------------------------------------------
	// Balance corpus by source: trimmed-mean (10%) down-sampling so no single
	// source dominates.
	let balanceCorpus = $state(false);
	let progress = $state<ipc.AnalysisProgressDto | null>(null);
	let preview = $state<ipc.AnalysisPreview | null>(null);
	// Top 250 / Bottom 250 toggle for the results table.
	let showTop = $state(true);
	let errorMessage = $state<string | null>(null);
	let exportMessage = $state<string | null>(null);

	// Which slice the results table renders: Top 250 (`entries`) or Bottom 250
	// (`bottom`, lowest-frequency terms). Mirrors egui's `show_top` radio.
	const displayedEntries = $derived(showTop ? (preview?.entries ?? []) : (preview?.bottom ?? []));

	const progressFraction = $derived(
		progress && progress.total_files > 0 ? progress.current_file / progress.total_files : 0
	);

	function fmtSecs(s: number): string {
		return `${Math.round(s)}s`;
	}

	async function analyze() {
		if (checkedCount === 0) {
			phase = 'error';
			errorMessage = 'No files selected';
			return;
		}
		const paths = checkedPaths;
		progress = null;
		preview = null;
		errorMessage = null;
		phase = 'analyzing';
		try {
			const result = await ipc.startAnalysis(paths, balanceCorpus, (p) => {
				progress = p;
			});
			preview = result;
			phase = 'results';
		} catch (err) {
			const msg = String(err);
			if (msg.includes('cancelled')) {
				// Cancel is not an error — return to selection (egui parity).
				phase = 'selecting';
			} else {
				errorMessage = `Analysis failed: ${msg}`;
				phase = 'error';
			}
		}
	}

	async function cancel() {
		await ipc.cancelAnalysis();
		// The rejected `start_analysis` promise drives the transition back to
		// `selecting` (see the catch above) — no state change here.
	}

	// --- Export ---------------------------------------------------------------
	let opts = $state<ipc.ExportOptions>(ipc.defaultExportOptions());

	const canExport = $derived(opts.export_yomitan || opts.export_csv);

	async function doExport() {
		const dir = await openDialog({ directory: true });
		if (!dir || Array.isArray(dir)) return;
		phase = 'exporting';
		try {
			exportMessage = await ipc.exportAnalysis(dir, opts);
			phase = 'complete';
		} catch (err) {
			errorMessage = String(err);
			phase = 'error';
		}
	}

	// --- Navigation -----------------------------------------------------------
	function close() {
		analyzerModalOpen.set(false);
	}

	function newAnalysis() {
		clearAll();
		preview = null;
		progress = null;
		errorMessage = null;
		exportMessage = null;
		opts = ipc.defaultExportOptions();
		phase = 'selecting';
	}

	function onKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') close();
	}
</script>

<!-- Esc closes from anywhere: the backdrop's own keydown only fires once focus
     is inside the modal, which it isn't right after opening from a menu. -->
<svelte:window onkeydown={(e) => $analyzerModalOpen && e.key === 'Escape' && close()} />

{#if $analyzerModalOpen}
	<div
		class="backdrop"
		role="button"
		tabindex="-1"
		onclick={close}
		onkeydown={onKeydown}
	>
		<div
			class="dialog"
			role="dialog"
			aria-modal="true"
			aria-label="Frequency analyzer"
			tabindex="-1"
			onclick={(e) => e.stopPropagation()}
		>
			<header>
				<h2>Frequency Analyzer</h2>
				<button class="close" aria-label="Close" onclick={close}>✕</button>
			</header>

			<div class="body">
				{#if phase === 'selecting'}
					<h3>Step 1 · Select files</h3>
					<div class="row">
						<button onclick={addFiles}>Add Files…</button>
						<button onclick={addFolder}>Add Folder…</button>
						<button class="ghost" onclick={clearAll} disabled={selectedPaths.length === 0}
							>Clear all</button
						>
						<span class="count">{checkedCount} of {selectedPaths.length} files selected</span>
					</div>

					{#if selectedPaths.length > 0}
						<div class="tree">
							{#each tree as node (node.id)}
								{@render treeNode(node, 0)}
							{/each}
						</div>
					{:else}
						<p class="hint">Add subtitle/text files or a folder to begin.</p>
					{/if}

					<footer>
						<label
							class="cb"
							title="Uses trimmed mean (10% trimming) to calculate balanced sample sizes."
						>
							<input type="checkbox" bind:checked={balanceCorpus} /> Balance corpus by source
						</label>
						<button class="primary" onclick={analyze} disabled={checkedCount === 0}
							>Analyze files</button
						>
					</footer>
				{:else if phase === 'analyzing'}
					<h3>Step 2 · Analyzing</h3>
					<div class="progress-bar">
						<div class="progress-fill" style:width={`${progressFraction * 100}%`}></div>
						<span class="progress-text"
							>{progress?.current_file ?? 0}/{progress?.total_files ?? 0}</span
						>
					</div>
					<p class="message">{progress?.message ?? 'Starting…'}</p>
					{#if progress?.eta_secs != null}
						<p class="hint">Estimated {fmtSecs(progress.eta_secs)} remaining</p>
					{/if}
					<footer>
						<button onclick={cancel}>Cancel Analysis</button>
					</footer>
				{:else if phase === 'results'}
					<div class="results-head">
						<h3>Step 3 · Results &amp; export</h3>
						<button class="ghost" onclick={() => (phase = 'selecting')}
							>← Back to file selection</button
						>
					</div>
					<p class="hint">
						{preview?.total ?? 0} unique terms{#if preview && preview.total > preview.entries.length}
							(showing top {preview.entries.length}){/if}
					</p>

					<div class="results-grid">
						<div class="results-table-col">
							<div class="show-toggle">
								<span>Show:</span>
								<label class="cb"
									><input type="radio" name="show-slice" checked={showTop} onchange={() => (showTop = true)} />
									Top 250</label
								>
								<label class="cb"
									><input
										type="radio"
										name="show-slice"
										checked={!showTop}
										onchange={() => (showTop = false)}
									/> Bottom 250</label
								>
							</div>
							<div class="results-table-wrap">
							<table class="results-table">
								<thead>
									<tr>
										<th class="rank">#</th>
										<th>Term</th>
										<th>Reading</th>
										<th class="num">Freq</th>
									</tr>
								</thead>
								<tbody>
									{#each displayedEntries as e, i (e.term + (e.reading ?? '') + i)}
										<tr>
											<td class="rank">{i + 1}</td>
											<td class="jp">{e.term}</td>
											<td class="jp reading">{e.reading ?? ''}</td>
											<td class="num">{e.frequency}</td>
										</tr>
									{/each}
								</tbody>
							</table>
							</div>
						</div>

						<div class="export-form">
							<h4>Export Options</h4>
							<label>Title<input type="text" bind:value={opts.dict_name} /></label>
							<label>Author<input type="text" bind:value={opts.dict_author} /></label>
							<label>URL<input type="text" bind:value={opts.dict_url} /></label>
							<label
								>Revision prefix<input type="text" bind:value={opts.revision_prefix} /></label
							>
							<label
								>Description<textarea rows="2" bind:value={opts.dict_description}></textarea></label
							>

							<div class="checks">
								<label class="cb"
									><input type="checkbox" bind:checked={opts.export_yomitan} /> Export as Yomitan
									ZIP</label
								>
								<label class="cb"
									><input type="checkbox" bind:checked={opts.export_csv} /> Export as CSV</label
								>
								<label class="cb"
									><input type="checkbox" bind:checked={opts.pretty_json} /> Pretty JSON output</label
								>
								<label class="cb"
									><input type="checkbox" bind:checked={opts.exclude_hapax} /> Exclude hapax
									(occurrences=1)</label
								>
							</div>

							<button class="primary" onclick={doExport} disabled={!canExport}>Export…</button>
						</div>
					</div>
				{:else if phase === 'exporting'}
					<h3>Exporting…</h3>
					<p class="message">Writing dictionary files…</p>
				{:else if phase === 'complete'}
					<p class="success">{exportMessage}</p>
					<footer>
						<button onclick={() => (phase = 'results')}>← Back to Results</button>
						<button class="ghost" onclick={newAnalysis}>New Analysis</button>
					</footer>
				{:else if phase === 'error'}
					<p class="error">{errorMessage}</p>
					<footer>
						<button onclick={newAnalysis}>Start New Analysis</button>
					</footer>
				{/if}
			</div>

			<div class="modal-footer">
				<button class="ghost" onclick={close}>Close</button>
			</div>
		</div>
	</div>
{/if}

{#snippet treeNode(node: TreeNode, depth: number)}
	<div class="tree-row" style:padding-left={`${depth * 1.1}rem`}>
		{#if node.path}
			<label class="cb">
				<input
					type="checkbox"
					checked={checked[node.path] !== false}
					onchange={(e) => toggleNode(node, e.currentTarget.checked)}
				/>
				<span class="leaf">{node.name}</span>
			</label>
		{:else}
			<button
				class="twisty"
				aria-label={collapsed[node.id] ? 'Expand' : 'Collapse'}
				onclick={() => (collapsed = { ...collapsed, [node.id]: !collapsed[node.id] })}
				>{collapsed[node.id] ? '▸' : '▾'}</button
			>
			<label class="cb">
				<input
					type="checkbox"
					checked={nodeChecked(node)}
					indeterminate={nodePartial(node)}
					onchange={(e) => toggleNode(node, e.currentTarget.checked)}
				/>
				<span class="dir">{node.name} ({collectChecked(node, () => true).length})</span>
			</label>
		{/if}
	</div>
	{#if !node.path && !collapsed[node.id]}
		{#each node.children as child (child.id)}
			{@render treeNode(child, depth + 1)}
		{/each}
	{/if}
{/snippet}

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
		width: min(760px, 94vw);
		max-height: 88vh;
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
	.body {
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
		padding: 0.9rem 1rem;
		overflow-y: auto;
	}
	h3 {
		margin: 0;
		font-size: 1rem;
		color: var(--fg);
	}
	h4 {
		margin: 0 0 0.3rem;
		font-size: 0.9rem;
		color: var(--fg);
	}
	.row {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		flex-wrap: wrap;
	}
	.count {
		margin-left: auto;
		font-size: 0.85rem;
		color: var(--comment);
	}
	.hint {
		margin: 0;
		font-size: 0.85rem;
		color: var(--comment);
	}
	.message {
		margin: 0;
		font-size: 0.9rem;
		color: var(--fg);
	}
	.tree {
		max-height: 220px;
		overflow-y: auto;
		border: 1px solid var(--border);
		border-radius: 3px;
		padding: 0.35rem;
		background: var(--bg-darker);
	}
	.tree-row {
		display: flex;
		align-items: center;
		gap: 0.25rem;
		padding: 0.05rem 0;
	}
	.twisty {
		padding: 0 0.25rem;
		font-size: 0.7rem;
		background: none;
		border: none;
		color: var(--comment);
		cursor: pointer;
	}
	.cb {
		display: flex;
		align-items: center;
		gap: 0.35rem;
		font-size: 0.85rem;
		cursor: pointer;
	}
	.dir {
		color: var(--yellow);
	}
	.leaf {
		color: var(--fg);
	}
	.progress-bar {
		position: relative;
		height: 1.4rem;
		border: 1px solid var(--border);
		border-radius: 3px;
		background: var(--bg-darker);
		overflow: hidden;
	}
	.progress-fill {
		height: 100%;
		background: var(--cyan);
		transition: width 0.2s ease;
	}
	.progress-text {
		position: absolute;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 0.8rem;
		color: var(--fg);
	}
	.results-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.5rem;
	}
	.results-grid {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 1rem;
		align-items: start;
	}
	.results-table-col {
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
	}
	.show-toggle {
		display: flex;
		align-items: center;
		gap: 0.6rem;
		font-size: 0.85rem;
		color: var(--comment);
	}
	.results-table-wrap {
		max-height: 340px;
		overflow-y: auto;
		border: 1px solid var(--border);
		border-radius: 3px;
	}
	.results-table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.85rem;
	}
	.results-table th,
	.results-table td {
		padding: 0.2rem 0.45rem;
		text-align: left;
		border-bottom: 1px solid var(--border);
	}
	.results-table thead th {
		position: sticky;
		top: 0;
		background: var(--bg-light);
		color: var(--comment);
		z-index: 1;
	}
	.results-table .rank {
		width: 2.5rem;
		color: var(--comment);
		text-align: right;
	}
	.results-table .num {
		text-align: right;
		width: 4rem;
	}
	.results-table .jp {
		font-size: 0.95rem;
	}
	.results-table .reading {
		color: var(--comment);
	}
	.export-form {
		display: flex;
		flex-direction: column;
		gap: 0.4rem;
	}
	.export-form label {
		display: flex;
		flex-direction: column;
		gap: 0.15rem;
		font-size: 0.8rem;
		color: var(--comment);
	}
	.export-form input,
	.export-form textarea {
		padding: 0.3rem 0.4rem;
		background: var(--bg-light);
		color: var(--fg);
		border: 1px solid var(--border);
		border-radius: 3px;
		font: inherit;
	}
	.export-form .checks {
		display: flex;
		flex-direction: column;
		gap: 0.2rem;
		margin: 0.3rem 0;
	}
	.export-form .checks .cb {
		color: var(--fg);
	}
	.success {
		margin: 0;
		color: var(--green);
		font-size: 0.95rem;
	}
	.error {
		margin: 0;
		color: var(--red);
		font-size: 0.95rem;
	}
	footer {
		display: flex;
		gap: 0.5rem;
		margin-top: 0.3rem;
	}
	.modal-footer {
		display: flex;
		justify-content: flex-end;
		padding: 0.6rem 1rem;
		border-top: 1px solid var(--border);
	}
	.primary {
		background: var(--cyan);
		color: var(--bg-darker);
		border-color: var(--cyan);
	}
	.ghost {
		background: none;
	}
	button:disabled {
		opacity: 0.5;
		cursor: default;
	}
</style>
