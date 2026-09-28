// Dictionary HTML from Yomitan, made safe to insert with {@html}.

/** Anki-media refs can never resolve here (and every DOM insert re-requests
 * them, spamming 404s), so anything that isn't a data: URI becomes `none`. */
function scrubCssUrls(css: string): string {
	return css
		.replace(/@import[^;]*(;|$)/gi, '')
		.replace(/url\(\s*(?!['"]?data:)[^)]*\)/gi, 'none');
}

/** Defang third-party dictionary HTML. Embedded style tags are kept — Yomitan
 * scopes them under `.yomitan-glossary` — but purged of external loads. */
export function sanitize(html: string): string {
	const doc = new DOMParser().parseFromString(html, 'text/html');
	doc.querySelectorAll('script, iframe, object, embed, link, meta').forEach((el) =>
		el.remove()
	);
	doc.querySelectorAll('style').forEach((el) => {
		el.textContent = scrubCssUrls(el.textContent ?? '');
	});
	for (const el of doc.body.querySelectorAll('*')) {
		for (const attr of [...el.attributes]) {
			const name = attr.name.toLowerCase();
			if (name.startsWith('on')) el.removeAttribute(attr.name);
			else if ((name === 'src' || name === 'href') && /^\s*javascript:/i.test(attr.value))
				el.removeAttribute(attr.name);
			else if (name === 'style' && /url\(/i.test(attr.value))
				el.setAttribute(attr.name, scrubCssUrls(attr.value));
		}
		if (el.tagName === 'A') el.removeAttribute('href');
		// An unresolvable image must take its Yomitan container along —
		// the styled wrapper alone renders as an empty white box.
		if (el.tagName === 'IMG' && !/^(https?:|data:)/i.test(el.getAttribute('src') ?? ''))
			(el.closest('a.gloss-image-link, span.gloss-image-container') ?? el).remove();
	}
	return doc.body.innerHTML;
}
