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
