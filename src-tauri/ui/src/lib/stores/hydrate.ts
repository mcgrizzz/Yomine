import { get } from 'svelte/store';
import * as ipc from '$lib/ipc';
import { dragHovering, lastError, overlay, showNotice } from './ui';
import { ankiStatus, knowledge, languageToolsStatus } from './status';
import { checkForUpdate } from './update';
import { playerStatus } from './player';
import { fileResult, isSupportedPath, loadAndStore, recentFiles } from './file';
import { posCatalog, posEnabled } from './controls';
import { settings } from './settings';
import { refreshIgnoredLemmas } from './ignore';
import { refreshRecommendedDicts } from './dictionaries';
import { refreshSetupStatus } from './setup';

let hydrated = false;

/** Idempotent; called once from the root page. */
export async function hydrate(): Promise<void> {
	if (hydrated) return;
	hydrated = true;

	// Events are wired before any await so early emits aren't missed. player/
	// anki/knowledge emit only on *change*: if one fires during the pulls below,
	// that event is fresher than the pull — a stale pull must not clobber it,
	// since the backend won't re-emit until the next change.
	let playerEventSeen = false;
	let ankiEventSeen = false;
	let knowledgeEventSeen = false;
	ipc.onLanguageToolsStatus((s) => languageToolsStatus.set(s));
	ipc.onAnkiStatus((s) => {
		ankiEventSeen = true;
		ankiStatus.set(s);
	});
	ipc.onPlayerStatus((s) => {
		playerEventSeen = true;
		playerStatus.set(s);
	});
	ipc.onKnowledgeSummary((s) => {
		knowledgeEventSeen = true;
		knowledge.set(s);
	});
	ipc.onTermsRefreshed((r) => fileResult.set(r));
	ipc.onError((e) => lastError.set(e));
	ipc.onAsbplayerMediaLoaded((r) => {
		fileResult.set(r);
		showNotice(`Loaded from asbplayer: ${r.source_file.title}`);
	});
	ipc.onDictionariesChanged(async () => {
		const current = await ipc.getTerms();
		if (current) fileResult.set(current);
		refreshSetupStatus();
	});

	// Drag-drop is a no-op until the language tools can process a file.
	const toolsReady = () => get(languageToolsStatus) === 'ready';
	ipc.onDragDrop({
		onEnter: (paths) => dragHovering.set(toolsReady() && paths.some(isSupportedPath)),
		onDrop: (paths) => {
			dragHovering.set(false);
			if (!toolsReady()) return;
			const file = paths.find(isSupportedPath);
			if (file) loadAndStore(file);
		},
		onLeave: () => dragHovering.set(false)
	});

	// player/anki must be pulled as well as subscribed: a (re)loaded webview
	// would otherwise sit on the placeholder until the next change event.
	const [loadedSettings, catalog, currentFile, recents, player, anki, summary] = await Promise.all([
		ipc.getSettings(),
		ipc.getPosCatalog(),
		ipc.getTerms(),
		ipc.getRecentFiles(),
		ipc.getPlayerStatus(),
		ipc.getAnkiStatus(),
		ipc.getKnowledgeSummary()
	]);
	settings.set(loadedSettings);
	posEnabled.set({ ...loadedSettings.pos_filters });
	posCatalog.set(catalog);
	fileResult.set(currentFile);
	recentFiles.set(recents);
	if (!playerEventSeen) playerStatus.set(player);
	if (!ankiEventSeen) ankiStatus.set(anki);
	if (summary && !knowledgeEventSeen) knowledge.set(summary);

	overlay.set('Loading language tools…');
	try {
		await ipc.loadLanguageTools((msg) => overlay.set(msg.message));
	} catch (err) {
		languageToolsStatus.set({ error: String(err) });
	} finally {
		overlay.set(null);
	}

	// These need the tools loaded (setup's freq-dict bit, the ignore command,
	// the recommended catalog's install states), so they run after.
	refreshSetupStatus();
	refreshIgnoredLemmas();
	refreshRecommendedDicts();

	// Best-effort update check; a failure just means no notice.
	void checkForUpdate();
}
