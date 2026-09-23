import assert from 'node:assert/strict';
import { test } from 'node:test';
import { build } from 'esbuild';
import { get, writable } from 'svelte/store';

const f = (globalThis.batchFixture = {
	fileResult: writable(null),
	asbContext: writable({ loaded_from_asbplayer: true }),
	playerStatus: writable({ mode: 'none' }),
	mediaMissing: writable(new Set()),
	minedNoteIds: writable({}),
	minedTerms: writable(new Set()),
	sessionMinedSentences: writable(new Set()),
	normalizeSentence: (s) => s.replace(/\s+/g, ''),
	miningTerm: writable(null),
	playerBusy: writable(false),
	batchSummaryOpen: writable(false),
	lastError: writable(null),
	showNotice() {},
	refreshMinedState: async () => {},
	ipc: {}
});
const IPC = ['getLastBatch', 'createBatch', 'finishBatch', 'undoBatch', 'mineBatchItem', 'getAsbplayerMedia'];
const { outputFiles } = await build({
	stdin: {
		contents: [
			"export * from './src/lib/stores/batches.ts';",
			"export * from './src/lib/stores/selection.ts';",
			"export * from './src/lib/batch.ts';"
		].join('\n'),
		resolveDir: process.cwd()
	},
	bundle: true,
	write: false,
	format: 'esm',
	platform: 'node',
	plugins: [
		{
			name: 'batch-fixtures',
			setup(b) {
				b.onResolve({ filter: /^\$lib\/ipc$/ }, () => ({ path: 'ipc', namespace: 'fixture' }));
				b.onResolve({ filter: /^\.\/(file|player|mining|ui|modals)$/ }, (a) => ({
					path: a.path,
					namespace: 'fixture'
				}));
				b.onLoad({ filter: /.*/, namespace: 'fixture' }, (a) => ({
					contents:
						a.path === 'ipc'
							? IPC.map(
									(name) =>
										`export const ${name} = (...args) => globalThis.batchFixture.ipc.${name}(...args);`
								).join('\n')
							: Object.keys(f)
									.filter((k) => k !== 'ipc')
									.map((name) => `export const ${name} = globalThis.batchFixture.${name};`)
									.join('\n')
				}));
			}
		}
	]
});
const m = await import(`data:text/javascript;base64,${Buffer.from(outputFiles[0].text).toString('base64')}`);

const item = (lemma, status = 'unattempted') => ({
	key: `${lemma} `,
	lemma,
	surface: lemma,
	sentence: `${lemma}だ`,
	timestamp: null,
	entry_index: null,
	format_name: null,
	scan_text: null,
	adhoc: false,
	mine_media: false,
	outcome: { status }
});

function fixture() {
	const items = ['猫', '犬', '鳥'].map((s) => item(s));
	const source = { title: 'Test', locator: 'test.txt', fingerprint: 'abc' };
	const file = {
		source_file: { title: 'Test', original_file: 'test.txt', epub_chapters: null },
		batch_source: source,
		sentences: items.map((i) => ({ text: i.sentence, timestamp: null })),
		terms: items.map((i, n) => ({ lemma_form: i.lemma, lemma_reading: '', sentence_references: [[n, 0]] }))
	};
	f.fileResult.set(file);
	f.asbContext.set({ loaded_from_asbplayer: true });
	f.playerBusy.set(false);
	f.miningTerm.set(null);
	f.lastError.set(null);
	f.batchSummaryOpen.set(false);
	m.clearSelection();
	m.batchSaveError.set(null);
	const batch = { id: 'original', started_at: 1, finished_at: 2, source, items };
	m.lastBatch.set(batch);
	return { batch, file };
}

const created = (note_id, media = 'not_requested') => ({ status: 'created', note_id, media, error: null });

test('source and occurrence matching prevent restoring a different file or a missing cue', () => {
	const { batch, file } = fixture();
	assert(m.sameSource(batch, file));
	assert(!m.sameSource(batch, { ...file, batch_source: { ...file.batch_source, locator: 'other.txt' } }));
	assert(!m.sameSource(batch, { ...file, batch_source: { ...file.batch_source, fingerprint: 'def' } }));
	assert.equal(m.occurrenceIndex(batch.items[0], file), 0);
	const changed = structuredClone(file);
	changed.sentences[0].text = '別の文';
	assert.equal(m.occurrenceIndex(batch.items[0], changed), null);
	const repeated = structuredClone(file);
	repeated.terms[0].sentence_references.push([0, 3]);
	assert.equal(m.occurrenceIndex(batch.items[0], repeated), 0, 'identical occurrences are interchangeable');
});

test('retry continues the saved batch without mining or removing unrelated queued work', async () => {
	let { batch } = fixture();
	batch.items[0].outcome = created(101, 'complete');
	batch.items[1].outcome = { status: 'failed', error: { stage: 'Rendering', scope: 'item', message: 'No entry' } };
	m.selectedTerms.set(new Set(['unrelated']));
	m.queueAdhoc({ ...item('別'), key: 'unrelated-adhoc' });
	const calls = [];
	f.ipc = {
		mineBatchItem: async (id, index) => {
			calls.push([id, index]);
			batch = structuredClone(batch);
			batch.items[index].outcome = created(200 + index);
			return { batch, failure: null };
		},
		finishBatch: async () => batch
	};
	await m.retryBatch();
	assert.deepEqual(calls, [['original', 1], ['original', 2]]);
	assert.equal(get(m.lastBatch).items[0].outcome.note_id, 101);
	assert.deepEqual([...get(m.selectedTerms)], ['unrelated']);
	assert.equal(get(m.adhocQueue)[0].key, 'unrelated-adhoc');
	m.restoreBatchSelection();
	assert.equal(get(m.selectedTerms).size, 4, 'created items can also be restored');
});

test('skip pauses again on the next failure and a media retry retains the existing note', async () => {
	let { batch } = fixture();
	const pauses = [];
	const off = m.batchPause.subscribe((p) => {
		if (!p) return;
		pauses.push(p.item.lemma);
		queueMicrotask(() => m.resumeBatch('skip'));
	});
	f.ipc = {
		mineBatchItem: async (_, index) => {
			batch = structuredClone(batch);
			const failure = { stage: 'Rendering', scope: 'item', message: 'No entry', fallback: null };
			batch.items[index].outcome = { status: 'failed', error: failure };
			return { batch, failure };
		},
		finishBatch: async () => batch
	};
	await m.retryBatch();
	off();
	assert.equal(pauses.length, 3);
	batch.items[0].outcome = created(42, 'failed');
	batch.items[0].mine_media = true;
	m.lastBatch.set(batch);
	const stop = m.batchPause.subscribe((p) => p && queueMicrotask(() => m.resumeBatch('stop')));
	f.ipc.mineBatchItem = async (_, index) => {
		assert.equal(index, 0);
		const failure = { stage: 'Recording media', scope: 'unknown', message: 'No update', fallback: null };
		return { batch, failure };
	};
	await m.retryBatch(true);
	stop();
	assert.equal(get(m.lastBatch).items[0].outcome.note_id, 42);
	assert.equal(get(m.lastBatch).items[0].outcome.media, 'failed');
});

test('a batch creates every card before recording any media', async () => {
	let { batch } = fixture();
	batch.items.forEach((i, n) => {
		i.mine_media = true;
		i.timestamp = { start_secs: n, end_secs: n + 1, start_label: '', end_label: '' };
	});
	const calls = [];
	f.ipc = {
		mineBatchItem: async (_, index, target, options) => {
			batch = structuredClone(batch);
			const o = batch.items[index].outcome;
			calls.push([o.status === 'created' ? 'record' : 'create', index]);
			batch.items[index].outcome =
				o.status === 'created'
					? created(o.note_id, 'complete')
					: created(600 + index, options.record ? 'pending' : 'skipped');
			return { batch, failure: null };
		},
		finishBatch: async () => batch
	};
	const progress = [];
	const off = m.mineQueueState.subscribe((p) => p && progress.push([p.phase, p.position, p.doneSecs / p.totalSecs]));
	await m.retryBatch();
	off();
	assert.deepEqual(calls, [
		['create', 0],
		['create', 1],
		['create', 2],
		['record', 0],
		['record', 1],
		['record', 2]
	]);
	assert(get(m.lastBatch).items.every((i) => i.outcome.media === 'complete'));
	const starts = progress.filter((p, i) => i === 0 || p[1] !== progress[i - 1][1] || p[0] !== progress[i - 1][0]);
	assert.deepEqual(
		starts.map(([phase, position]) => [phase, position]),
		[['create', 1], ['create', 2], ['create', 3], ['record', 1], ['record', 2], ['record', 3]]
	);
	const fractions = starts.map(([, , fraction]) => fraction);
	assert(fractions.every((f, i) => i === 0 || f > fractions[i - 1]), 'one bar advances across both steps');
});

test('the time estimate weights recordings by cue length and learns from measured steps', () => {
	const { batch } = fixture();
	batch.items[0].timestamp = { start_secs: 0, end_secs: 10, start_label: '', end_label: '' };
	const plan = {
		create: { indices: [0, 1, 2], done: 3, samples: [1, 1, 1] },
		record: { indices: [0], done: 0, samples: [] }
	};
	assert.deepEqual(m.estimateProgress(batch, plan), { doneSecs: 3, currentSecs: 13, totalSecs: 16 });
	plan.record.samples.push(1);
	assert.deepEqual(m.estimateProgress(batch, plan), { doneSecs: 3, currentSecs: 11, totalSecs: 14 });
});

test('stopping during recording keeps the cards and leaves their media for later', async () => {
	let { batch } = fixture();
	batch.items.forEach((i) => (i.mine_media = true));
	const off = m.batchPause.subscribe((p) => p && queueMicrotask(() => m.resumeBatch('stop')));
	f.ipc = {
		mineBatchItem: async (_, index) => {
			batch = structuredClone(batch);
			const o = batch.items[index].outcome;
			if (o.status !== 'created') {
				batch.items[index].outcome = created(700 + index, 'pending');
				return { batch, failure: null };
			}
			const failure = {
				stage: 'Recording media',
				scope: 'unknown',
				message: 'Not recorded',
				fallback: null,
				kind: 'media_unverified'
			};
			batch.items[index].outcome = { ...o, media: 'failed', error: failure };
			return { batch, failure };
		},
		finishBatch: async () => batch
	};
	await m.retryBatch();
	off();
	const media = get(m.lastBatch).items.map((i) => i.outcome.media);
	assert.deepEqual(media, ['failed', 'pending', 'pending']);
	assert.deepEqual(m.retryIndices(get(m.lastBatch), true), [0, 1, 2]);
});

test('a dictionary media fallback applies only to the card it was chosen for', async () => {
	let { batch } = fixture();
	const calls = [];
	const off = m.batchPause.subscribe((p) => p && queueMicrotask(() => m.resumeBatch('without_dictionary_media')));
	f.ipc = {
		mineBatchItem: async (_, index, target, options) => {
			calls.push([index, options.require_dictionary_media]);
			batch = structuredClone(batch);
			if (index === 0 && options.require_dictionary_media) {
				const failure = { stage: 'Uploading media', scope: 'unknown', message: 'x.mp3', fallback: 'without_dictionary_media' };
				batch.items[0].outcome = { status: 'failed', error: failure };
				return { batch, failure };
			}
			batch.items[index].outcome = created(400 + index);
			return { batch, failure: null };
		},
		finishBatch: async () => batch
	};
	await m.retryBatch();
	off();
	assert.deepEqual(calls, [[0, true], [0, false], [1, true], [2, true]]);
});

test('a new batch asks before replacing one that needs review', async () => {
	const { batch } = fixture();
	batch.finished_at = null;
	let createdBatches = 0;
	f.ipc = { createBatch: async () => createdBatches++ };
	const off = m.batchReplace.subscribe((p) => p && queueMicrotask(() => m.confirmReplaceBatch(false)));
	const queued = { ...item('魚'), entryIndex: undefined, formatName: undefined, scanText: undefined };
	await m.mineQueue([queued]);
	off();
	assert.equal(createdBatches, 0);
	assert.equal(get(m.lastBatch).id, 'original');
	assert.equal(get(f.batchSummaryOpen), true);
});

test('the only usable recording tab is chosen without asking', async () => {
	let { batch } = fixture();
	batch.items[0].mine_media = true;
	f.asbContext.set({ loaded_from_asbplayer: false });
	let prompted = false;
	const off = m.batchTarget.subscribe((p) => p && (prompted = true));
	const targets = [];
	f.ipc = {
		getAsbplayerMedia: async () => [
			{ id: 'tab-1', active: true, loaded_subtitles: [{}] },
			{ id: 'tab-2', active: false, loaded_subtitles: [{}] }
		],
		mineBatchItem: async (_, index, target) => {
			targets.push(target);
			batch = structuredClone(batch);
			batch.items[index].outcome = created(500 + index);
			return { batch, failure: null };
		},
		finishBatch: async () => batch
	};
	await m.retryBatch();
	off();
	assert(!prompted);
	assert(targets.length > 0 && targets.every((t) => t === 'tab-1'));
});

test('a failed checkpoint preserves the known note ID and stops further mining and undo', async () => {
	let { batch } = fixture();
	let calls = 0;
	f.ipc = {
		mineBatchItem: async (_, index) => {
			calls++;
			batch = structuredClone(batch);
			batch.items[index].outcome = created(77, 'pending');
			return { batch, failure: { stage: 'Saving batch', scope: 'stop', message: 'Disk full', fallback: null } };
		}
	};
	await m.retryBatch();
	assert.equal(calls, 1);
	assert.equal(get(m.lastBatch).items[0].outcome.note_id, 77);
	assert.equal(get(m.batchSaveError), 'Disk full');
	assert.equal(get(f.playerBusy), false);
	await m.undoLastBatch();
	await m.retryBatch();
	assert.equal(calls, 1);
});
