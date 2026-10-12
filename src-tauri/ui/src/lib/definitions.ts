import { renderDefinition, type DefinitionEntry } from '$lib/ipc';

// Session-wide so reopening a term never refetches.
const cache = new Map<string, DefinitionEntry[]>();

export function cachedEntries(text: string): DefinitionEntry[] | undefined {
	return cache.get(text);
}

const waiting: (() => void)[] = [];
let busy = false;

/** Runs Yomitan lookups one at a time, `first` ahead of those waiting. Yomitan answers them in
 * turn anyway, and the backend fails one it waits on too long (10 s for definitions, 2 s for
 * default entries), so a burst of lookups would fail its last ones. */
export function inTurn<T>(run: () => Promise<T>, first = false): Promise<T> {
	return new Promise<T>((resolve, reject) => {
		const task = () => void run().then(resolve, reject).finally(nextLookup);
		if (first) waiting.unshift(task);
		else waiting.push(task);
		if (!busy) nextLookup();
	});
}

function nextLookup(): void {
	const task = waiting.shift();
	busy = task !== undefined;
	task?.();
}

/** `first` for a lookup someone is waiting on (a hover), ahead of a list's. */
export async function fetchEntries(text: string, first = false): Promise<DefinitionEntry[]> {
	const result = await inTurn(() => renderDefinition(text), first);
	cache.set(text, result);
	return result;
}

/** Plain-text entry label, matching DefinitionPopover's non-furigana header branch. */
export function entryLabel(entry: DefinitionEntry, fallback = ''): string {
	const expression = entry.expression || fallback;
	return entry.reading && entry.reading !== entry.expression
		? `${expression}【${entry.reading}】`
		: expression;
}

/** The first glosses as plain text. Yomitan marks them `data-sc-content="glossary"`; other
 * dictionaries fall back to leaf list items. */
export function glossarySummary(html: string, max = 6): string {
	const doc = new DOMParser().parseFromString(html, 'text/html');
	doc.querySelectorAll('style, script').forEach((el) => el.remove());
	const text = (el: Element) => (el.textContent ?? '').replace(/\s+/g, ' ').trim();
	let items = [...doc.querySelectorAll('[data-sc-content="glossary"] > li')];
	if (items.length === 0)
		items = [...doc.querySelectorAll('li')].filter((li) => !li.querySelector('li'));
	const glosses = items.map(text).filter(Boolean);
	return glosses.length > 0 ? glosses.slice(0, max).join('; ') : text(doc.body);
}
