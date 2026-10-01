import type {
	BatchItem,
	BatchOutcome,
	BatchPart,
	BatchRecord,
	FileLoadResult,
	SentenceDto
} from './ipc';
import { termKey } from './table';

export function sameSource(batch: BatchRecord, file: FileLoadResult | null): boolean {
	return (
		file !== null &&
		file.batch_source.locator === batch.source.locator &&
		file.batch_source.fingerprint === batch.source.fingerprint
	);
}

export function mediaParts(outcome: BatchOutcome): BatchPart[] {
	return outcome.status === 'created' ? [outcome.audio, outcome.picture] : [];
}

export function lacksMedia(outcome: BatchOutcome): boolean {
	return mediaParts(outcome).some((p) => p === 'pending' || p === 'failed' || p === 'skipped');
}

/** A skipped part waits until the user asks for it. */
export function recordsNow(outcome: BatchOutcome): boolean {
	return mediaParts(outcome).some((p) => p === 'pending' || p === 'failed');
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
const LOCAL_RECORD_GUESS_SECS = 1;

export function cueSecs(item: BatchItem): number {
	const t = item.timestamp;
	return t ? Math.max(0, t.end_secs - t.start_secs) : 0;
}

/** asbplayer plays each line to record it, so its record samples are the overhead beyond
 * the cue's own length. Cutting from a local file doesn't depend on the cue's length. */
export function estimateProgress(
	batch: BatchRecord,
	plan: BatchPlan,
	local = false
): { doneSecs: number; currentSecs: number; totalSecs: number } {
	const sum = (xs: number[]) => xs.reduce((a, b) => a + b, 0);
	const average = (xs: number[], guess: number) => (xs.length ? sum(xs) / xs.length : guess);
	const create = average(plan.create.samples, CREATE_GUESS_SECS);
	const overhead = average(
		plan.record.samples,
		local ? LOCAL_RECORD_GUESS_SECS : RECORD_OVERHEAD_GUESS_SECS
	);
	const createCosts = plan.create.indices.map(() => create);
	const recordCosts = plan.record.indices.map(
		(i) => (local ? 0 : cueSecs(batch.items[i])) + overhead
	);
	const costs = [...createCosts.slice(plan.create.done), ...recordCosts.slice(plan.record.done)];
	const totalSecs = sum(createCosts) + sum(recordCosts);
	return { doneSecs: totalSecs - sum(costs), currentSecs: costs[0] ?? 0, totalSecs };
}
