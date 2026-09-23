import type { BatchItem, BatchOutcome, BatchRecord, FileLoadResult, SentenceDto } from './ipc';
import { termKey } from './table';

export function sameSource(batch: BatchRecord, file: FileLoadResult | null): boolean {
	return (
		file !== null &&
		file.batch_source.locator === batch.source.locator &&
		file.batch_source.fingerprint === batch.source.fingerprint
	);
}

export function lacksMedia(outcome: BatchOutcome): boolean {
	return outcome.status === 'created' && ['pending', 'failed', 'skipped'].includes(outcome.media);
}

export function retryIndices(batch: BatchRecord, media = false): number[] {
	return batch.items.flatMap((item, index) => {
		const o = item.outcome;
		const retry = media ? lacksMedia(o) : o.status === 'failed' || o.status === 'unattempted';
		return retry ? [index] : [];
	});
}

export function needsReview(batch: BatchRecord): boolean {
	return batch.finished_at === null || batch.items.some((i) => i.outcome.status === 'attempting');
}

export function occurrenceIndex(item: BatchItem, file: FileLoadResult): number | null {
	const refs = item.adhoc
		? file.sentences.map((_, i) => [i])
		: file.terms.find((t) => termKey(t) === item.key)?.sentence_references;
	const index = refs?.findIndex(([sid]) => sameOccurrence(item, file.sentences[sid])) ?? -1;
	return index < 0 ? null : index;
}

function sameOccurrence(item: BatchItem, sentence: SentenceDto | undefined): boolean {
	const compact = (text: string) => text.replace(/\s+/g, '');
	if (!sentence || compact(sentence.text) !== compact(item.sentence)) return false;
	const a = sentence.timestamp;
	const b = item.timestamp;
	if (!a || !b) return a === b;
	const ms = (secs: number) => Math.round(secs * 1000);
	return ms(a.start_secs) === ms(b.start_secs) && ms(a.end_secs) === ms(b.end_secs);
}

export type BatchPhase = 'create' | 'record';

export interface PhasePlan {
	indices: number[];
	done: number;
	samples: number[];
}

export type BatchPlan = Record<BatchPhase, PhasePlan>;

const CREATE_GUESS_SECS = 2;
const RECORD_OVERHEAD_GUESS_SECS = 3;

export function cueSecs(item: BatchItem): number {
	const t = item.timestamp;
	return t ? Math.max(0, t.end_secs - t.start_secs) : 0;
}

/** Record-phase samples are the overhead beyond each cue's own length. */
export function estimateProgress(
	batch: BatchRecord,
	plan: BatchPlan
): { doneSecs: number; currentSecs: number; totalSecs: number } {
	const sum = (xs: number[]) => xs.reduce((a, b) => a + b, 0);
	const average = (xs: number[], guess: number) => (xs.length ? sum(xs) / xs.length : guess);
	const create = average(plan.create.samples, CREATE_GUESS_SECS);
	const overhead = average(plan.record.samples, RECORD_OVERHEAD_GUESS_SECS);
	const createCosts = plan.create.indices.map(() => create);
	const recordCosts = plan.record.indices.map((i) => cueSecs(batch.items[i]) + overhead);
	const costs = [
		...createCosts.slice(plan.create.done),
		...recordCosts.slice(plan.record.done)
	];
	const totalSecs = sum(createCosts) + sum(recordCosts);
	return { doneSecs: totalSecs - sum(costs), currentSecs: costs[0] ?? 0, totalSecs };
}
