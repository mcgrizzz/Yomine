import { renderDefinition, type DefinitionEntry } from '$lib/ipc';

// Session-wide so reopening a term never refetches.
const cache = new Map<string, DefinitionEntry[]>();

export function cachedEntries(text: string): DefinitionEntry[] | undefined {
	return cache.get(text);
}

export async function fetchEntries(text: string): Promise<DefinitionEntry[]> {
	const result = await renderDefinition(text);
	cache.set(text, result);
	return result;
}

/** The queued entry, resolved by Yomitan's pre-filter index rather than position. */
export function cachedEntry(scanText?: string, entryIndex?: number): DefinitionEntry | undefined {
	if (scanText === undefined || entryIndex === undefined) return undefined;
	return cache.get(scanText)?.find((e) => e.index === entryIndex);
}

/** Plain-text entry label, matching DefinitionPopover's non-furigana header branch. */
export function entryLabel(entry: DefinitionEntry, fallback = ''): string {
	const expression = entry.expression || fallback;
	return entry.reading && entry.reading !== entry.expression
		? `${expression}【${entry.reading}】`
		: expression;
}
