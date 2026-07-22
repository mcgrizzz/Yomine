import { get } from 'svelte/store';
import * as ipc from '$lib/ipc';
import { termKey } from '$lib/table';
import { dragHovering, initProgress, lastError, showNotice } from './ui';
import { ankiStatus, knowledge, languageToolsStatus } from './status';
import { checkForUpdate } from './update';
import { asbContext, playerStatus } from './player';
import { fileResult, isSupportedPath, loadAndStore, recentFiles } from './file';
import { jlptEnabled, posCatalog, posEnabled } from './controls';
import { settings } from './settings';
import { refreshIgnoredLemmas } from './ignore';
import { refreshRecommendedDicts } from './dictionaries';
import { refreshMinedState, yomitanReachable } from './mining';
import { selectedTerms } from './selection';
import { refreshSetupStatus } from './setup';

let hydrated = false;

/** Idempotent; called once from the root page. */
export async function hydrate(): Promise<void> {
	if (hydrated) return;
	hydrated = true;

	const settingsPull = ipc.getSettings();
	const recentsPull = ipc.getRecentFiles().then((r) => recentFiles.set(r));

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
	// Backend probe (5s poll, change-only) — keeps the dot fresh even when no
	// file is loaded and nothing calls refreshMinedState.
	ipc.onYomitanStatus((s) => yomitanReachable.set(s.reachable));
	ipc.onTermsRefreshed((r) => fileResult.set(r));
	ipc.onError((e) => lastError.set(e));
	ipc.onAsbplayerMediaLoaded((r) => {
		fileResult.set(r);
		showNotice(`Loaded from asbplayer: ${r.source_file.title}`);
	});
	ipc.onAsbplayerContext((c) => asbContext.set(c));
	ipc.onDictionariesChanged(async () => {
		const current = await ipc.getTerms();
		if (current) fileResult.set(current);
		refreshSetupStatus();
	});

	// Rows dropped by a refresh (mined/ignored) silently leave the selection.
	// Wired here, not in selection.ts — see the note there.
	fileResult.subscribe((r) => {
		const live = new Set(r?.terms.map(termKey) ?? []);
		selectedTerms.update((s) => new Set([...s].filter((k) => live.has(k))));
	});

	// Drag-drop works while the tools are still loading (loadAndStore waits for
	// them); only a failed init disables it.
	const toolsUsable = () => typeof get(languageToolsStatus) !== 'object';
	ipc.onDragDrop({
		onEnter: (paths) => dragHovering.set(toolsUsable() && paths.some(isSupportedPath)),
		onDrop: (paths) => {
			dragHovering.set(false);
			if (!toolsUsable()) return;
			const file = paths.find(isSupportedPath);
			if (file) loadAndStore(file);
		},
		onLeave: () => dragHovering.set(false)
	});

	// player/anki must be pulled as well as subscribed: a (re)loaded webview
	// would otherwise sit on the placeholder until the next change event.
	const batch = Promise.all([
		ipc.getPosCatalog(),
		ipc.getTerms(),
		ipc.getPlayerStatus(),
		ipc.getAnkiStatus(),
		ipc.getKnowledgeSummary()
	]);

	const loadedSettings = await settingsPull;
	settings.set(loadedSettings);
	posEnabled.set({ ...loadedSettings.pos_filters });
	jlptEnabled.set({ ...loadedSettings.jlpt_filters });

	const [catalog, currentFile, player, anki, summary] = await batch;
	posCatalog.set(catalog);
	fileResult.set(currentFile);
	await recentsPull;
	if (!playerEventSeen) playerStatus.set(player);
	if (!ankiEventSeen) ankiStatus.set(anki);
	if (summary && !knowledgeEventSeen) knowledge.set(summary);

	initProgress.set('Loading language tools…');
	try {
		await ipc.loadLanguageTools((msg) => initProgress.set(msg.message));
	} catch (err) {
		languageToolsStatus.set({ error: String(err) });
	} finally {
		initProgress.set(null);
	}

	// These need the tools loaded (setup's freq-dict bit, the ignore command,
	// the recommended catalog's install states), so they run after.
	refreshSetupStatus();
	refreshIgnoredLemmas();
	refreshRecommendedDicts();
	// Restores mined state / mine-button gating on a webview reload.
	void refreshMinedState(true);

	// Best-effort update check; a failure just means no notice.
	void checkForUpdate();
}
