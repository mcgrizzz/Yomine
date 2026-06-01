# Feature Specification: Yomine on a Web-Based UI Shell (Tauri Port)

**Feature Branch**: `feat/tauri-port`

**Created**: 2026-06-01

**Status**: Draft

**Input**: User description: "Investigate a tauri port of Yomine. We previously looked into
it but didn't find a super easy/painfree way to do it. Maybe spec driven."

## Why this feature exists

Yomine helps Japanese learners mine vocabulary from native content. Today its desktop UI is
drawn with an immediate-mode toolkit that makes rich Japanese text presentation (furigana,
ruby text, fine typography), styling, and layout harder than they need to be, and that limits
contributor velocity for UI work. This feature re-platforms the UI onto a web technology
shell while keeping the proven Rust analysis engine, so the app gains HTML/CSS text rendering
and a more approachable UI codebase **without changing what the tool does or how accurate it
is**.

The bar for this work is **feature parity**: a learner who uses the new app should be able to
do everything they can do today, with the same analysis results, plus benefit from better
text rendering. Re-platforming is the only goal — no new analysis features are in scope.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Mine vocabulary from a subtitle file (Priority: P1)

A learner opens a Japanese subtitle file (SRT, ASS/SSA) or a plain-text file. The app segments
the text, extracts unique words and multi-word expressions, ranks them by frequency, and
shows a table of terms with their dictionary (lemma) form, reading, part of speech, frequency,
how many example sentences contain them, and a comprehension estimate. The learner can expand
a term to read its example sentence(s) with readings shown over the Japanese.

**Why this priority**: This is the core value of Yomine and the minimum viable product. Every
other story builds on having a ranked term table from a file.

**Independent Test**: Open a known SRT file with no Anki connection; verify the term count,
ordering, readings, parts of speech, and frequency values match the egui app on the same file.

**Acceptance Scenarios**:

1. **Given** the app is open and language tools have loaded, **When** the learner opens a
   supported subtitle file, **Then** a ranked table of unknown terms appears with reading,
   part of speech, frequency, sentence count, and comprehension columns.
2. **Given** a term in the table, **When** the learner expands it, **Then** its example
   sentence(s) are shown with furigana-style readings over the Japanese text.
3. **Given** an unsupported or unreadable file, **When** the learner tries to open it, **Then**
   the app shows a clear error and leaves any previously loaded results intact.
4. **Given** the app is launched, **When** the learner drags a supported file onto the window,
   **Then** the same load-and-segment flow runs as opening via the file dialog.

### User Story 2 - Hide words I already know via Anki (Priority: P1)

A learner who studies with Anki connects the app to their collection so that words they already
know are filtered out of the mining table, and remaining terms and sentences carry a
comprehension estimate derived from their card intervals. The filtering uses a fast cached
snapshot on load and refreshes against live Anki in the background when reachable.

**Why this priority**: Filtering known vocabulary is the second pillar of the tool's value;
without it the table is cluttered with words the learner has already learned.

**Independent Test**: With a mapped Anki note type and known cards present, load a file and
verify the same terms are hidden and the same comprehension percentages appear as in egui.

**Acceptance Scenarios**:

1. **Given** Anki is running with AnkiConnect and a note-type field mapping is configured,
   **When** the learner loads a file, **Then** terms present and known in Anki are hidden from
   the table and the remaining terms show comprehension based on card intervals.
2. **Given** a file is already loaded from the cached snapshot, **When** Anki becomes
   reachable, **Then** the table refreshes in place against live data without a manual reload.
3. **Given** Anki is not running, **When** the learner loads a file, **Then** mining still
   works offline using the last cached snapshot (or shows all terms if no snapshot exists).

### User Story 3 - Jump to the moment in the video (Priority: P2)

A learner browsing a term's example sentence clicks its timestamp to seek the connected video
player (asbplayer over a local WebSocket, or MPV over its IPC socket) to that moment, so they
can hear and see the word in context.

**Why this priority**: Context navigation is a major workflow benefit but depends on the table
(US1) and an external player; the table is useful without it.

**Independent Test**: With asbplayer connected, click a sentence timestamp and verify the
player seeks to the same time the egui app would.

**Acceptance Scenarios**:

1. **Given** asbplayer is connected via WebSocket, **When** the learner clicks a sentence
   timestamp, **Then** the player seeks to that timestamp.
2. **Given** MPV is running with its IPC socket, **When** the learner clicks a timestamp,
   **Then** MPV seeks; and the app prefers MPV over the WebSocket server when both are present.
3. **Given** no player is connected, **When** the learner clicks a timestamp, **Then** the app
   indicates that no player is available rather than failing silently.

### User Story 4 - Refine and search the term list (Priority: P2)

A learner narrows the table to what they care about: sorting by frequency, chronological
order, sentence count, or comprehension; filtering by part of speech and by a frequency-rank
range; searching for specific terms; and right-clicking a term to add it to an ignore list so
it disappears from current and future results.

**Why this priority**: Refinement greatly improves usability on large files but the table is
functional with defaults; it layers on top of US1.

**Independent Test**: Apply each sort, the POS filter, the frequency-range filter, a search
query, and an ignore-list add; verify the resulting visible set matches the egui app.

**Acceptance Scenarios**:

1. **Given** a populated table, **When** the learner chooses a sort or applies POS/frequency
   filters or types a search query, **Then** the visible terms update to match those criteria.
2. **Given** a term, **When** the learner adds it to the ignore list, **Then** it is removed
   from the table immediately and stays hidden on subsequent loads until removed from the list.
3. **Given** the ignore-list manager, **When** the learner removes an entry, **Then** affected
   terms can reappear in results.

### User Story 5 - Configure and personalize (Priority: P2)

A learner sets up and tunes the app: mapping Anki note types to term/reading fields (with a
live connection indicator and field guessing), changing the WebSocket port, loading/reloading
and weighting frequency dictionaries, choosing which parts of speech are shown by default,
toggling dark/light theme and serif/sans Japanese fonts, and following a setup checklist that
guides first-time configuration. Settings persist across restarts.

**Why this priority**: Configuration is necessary for US2/US3 to work well and for a good first
run, but sensible defaults let mining (US1) work before any setup.

**Independent Test**: Change each setting, restart the app, and verify the setting persisted
and took effect, matching egui behavior.

**Acceptance Scenarios**:

1. **Given** Anki is reachable, **When** the learner opens Anki settings, **Then** note types
   and fields are listed, field guesses are offered, and a saved mapping persists.
2. **Given** changed settings (port, weights, POS defaults, theme, font), **When** the learner
   restarts the app, **Then** the settings are retained and applied.
3. **Given** a first-time user, **When** they open the setup checklist, **Then** it reflects the
   true state of Anki, dictionaries, and player connectivity and links to the relevant actions.

### User Story 6 - Build a frequency dictionary from my own content (Priority: P3)

A learner selects a set of files and generates a custom frequency dictionary, watching progress
with a time estimate, reviewing the resulting term ranking, and exporting it as a
Yomitan-compatible dictionary and/or CSV with configurable metadata. Long runs can be cancelled.

**Why this priority**: A powerful but advanced, self-contained tool used by a minority of
sessions; not required for everyday mining.

**Independent Test**: Run the analyzer over a small fixed file set and verify the produced
ranking and exported artifacts match the egui app's output for the same inputs and options.

**Acceptance Scenarios**:

1. **Given** selected input files, **When** the learner starts analysis, **Then** progress and
   a time estimate are shown and the run can be cancelled.
2. **Given** a completed analysis, **When** the learner exports, **Then** the chosen Yomitan zip
   and/or CSV are written with the provided name/author/URL/description and options honored.

### User Story 7 - See my overall knowledge at a glance (Priority: P3)

A learner views a summary of how much of each JLPT level and each frequency band they already
know, based on their Anki data, toggling between raw Anki coverage and a graded comprehension
estimate.

**Why this priority**: A motivating, informational overview; valuable but not part of the core
mining loop and dependent on Anki data (US2).

**Independent Test**: With a cached Anki snapshot present, verify the JLPT and frequency-band
coverage/estimate values match the egui knowledge summary for the same data.

**Acceptance Scenarios**:

1. **Given** an Anki vocabulary snapshot exists, **When** the learner views the knowledge
   summary, **Then** per-JLPT-level and per-frequency-band statistics are shown.
2. **Given** the summary is visible, **When** the learner toggles the mode, **Then** it switches
   between Anki coverage and estimated knowledge.

### Edge Cases

- Opening a file before language tools finish loading: the app must queue or clearly indicate
  it is not ready, never crash or silently drop the request.
- A file that parses but yields zero terms (e.g., non-Japanese content): show an empty table
  with an explanatory state, not an error.
- Anki reachable but the configured note type/field mapping is missing or wrong: mining must
  still proceed; known-word filtering degrades gracefully.
- Player connectivity changing mid-session (MPV opens/closes): the app must switch player modes
  the way egui does without requiring a restart.
- Reloading frequency dictionaries while a file is loaded: rankings and bands must recompute and
  the table refresh, without losing the loaded file.
- Very large files / very large term counts: the table must stay responsive (scrolling,
  sorting, filtering) without freezing the window.
- App data directory or cache files missing or unreadable: fall back to defaults rather than
  failing to start.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The app MUST open SRT, ASS/SSA, and TXT files via a file dialog and via
  drag-and-drop, and MUST list and reopen recent files with their metadata.
- **FR-002**: The app MUST segment Japanese text and extract deduplicated terms and multi-word
  expressions with lemma form, reading, part of speech, per-dictionary frequencies, the
  sentences that reference them, and a comprehension value — producing the same results as the
  current engine.
- **FR-003**: The app MUST present terms in a table with reading, part of speech, frequency,
  sentence count, and comprehension, and MUST render example sentences with readings displayed
  over the Japanese text.
- **FR-004**: The app MUST support sorting (frequency, chronological, sentence count,
  comprehension), filtering by part of speech and by frequency-rank range, and text search over
  terms, with results equivalent to the current app.
- **FR-005**: The app MUST integrate with Anki via AnkiConnect to hide known terms and compute
  comprehension from card intervals, using a cached snapshot on load and a live background
  refresh when Anki is reachable.
- **FR-006**: The app MUST let the user map Anki note types to term and reading fields, offer
  field guesses, and show live Anki connection status.
- **FR-007**: The app MUST maintain an ignore list (add via term context action, manage via a
  dedicated view) that hides terms from current and future results and persists across sessions.
- **FR-008**: The app MUST connect to a video player — asbplayer via a local WebSocket server
  and MPV via its IPC socket — seek to sentence timestamps, prefer MPV when both are available,
  and report when no player is connected.
- **FR-009**: The app MUST download, load, reload, enable/disable, and weight Yomitan-compatible
  frequency dictionaries, and recompute rankings and bands when they change.
- **FR-010**: The app MUST provide a frequency-analyzer tool that builds a frequency dictionary
  from selected files with progress, a time estimate, cancellation, results review, and export
  to Yomitan zip and/or CSV with configurable metadata and options.
- **FR-011**: The app MUST display a knowledge summary across JLPT levels and frequency bands
  from Anki data, with a toggle between coverage and estimated knowledge.
- **FR-012**: The app MUST provide a setup checklist/banner reflecting true Anki, dictionary,
  and player state and linking to the corresponding configuration actions.
- **FR-013**: The app MUST persist all user settings (Anki mappings, known-interval, WebSocket
  port, dictionary weights/enabled state, POS defaults, theme, font choice) to the OS app-data
  directory and apply them on startup.
- **FR-014**: The app MUST present long-running operations (tool loading, file processing, Anki
  refresh, analysis) with progress/status feedback and MUST keep the UI responsive during them.
- **FR-015**: The app MUST start and operate fully offline, downloading dictionaries on demand
  the way the current app does, and MUST never lose a loaded file due to a background failure.
- **FR-016**: The existing egui application MUST continue to build and run unchanged until the
  new app reaches parity (no regression to current users during the migration).

### Key Entities *(include if feature involves data)*

- **Term**: A mined word or expression — lemma form and reading, surface form and reading,
  part of speech, per-dictionary frequency ranks, the sentences it appears in, kana flag, and a
  comprehension value (0–1).
- **Sentence**: A line of source text — its text, segments (reading + POS + span), optional
  timestamp, and a comprehension value.
- **Source File**: The opened content — path, title, optional source/creator, and file type.
- **Settings**: User configuration — Anki note-type→field mappings, known-interval, WebSocket
  port, frequency-dictionary weights/enabled flags, default POS filter, theme, and font choice.
- **Frequency Dictionary**: A named, weightable, enable-able ranking source used for ranking
  terms and defining frequency bands.
- **Knowledge Summary**: Per-JLPT-level and per-frequency-band coverage and comprehension stats
  derived from Anki data.
- **Analysis Result**: A generated frequency ranking with the per-term counts and metadata
  needed to export a dictionary.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of the user-facing capabilities in the current app (the parity checklist)
  are available in the new app.
- **SC-002**: For an identical input (same file, Anki state, and dictionaries), the term list,
  ordering, readings, parts of speech, frequencies, and comprehension values match the current
  app exactly.
- **SC-003**: The frequency analyzer produces the same ranking and equivalent exported
  artifacts as the current app for the same inputs and options.
- **SC-004**: The knowledge summary's JLPT and frequency-band values match the current app for
  the same Anki snapshot.
- **SC-005**: Opening a typical subtitle file (a single episode's worth of lines) shows results
  in a time comparable to the current app, with no perceptible UI freeze.
- **SC-006**: Sorting, filtering, searching, and scrolling a large term list remain responsive
  (no noticeable lag) on a typical file.
- **SC-007**: All settings survive an app restart and take effect on the next launch.
- **SC-008**: The app launches and mines a file with no network connection (using existing
  cached/downloaded data).
- **SC-009**: Throughout the migration, the current app continues to build and run with no
  behavioral regression.

## Assumptions

- The Rust analysis engine (segmentation, ranking, Anki, player, analyzer) is reused as-is; this
  feature changes the UI layer and the boundary to it, not the algorithms.
- Existing external integrations are unchanged: AnkiConnect over HTTP, a local WebSocket server
  for asbplayer, and MPV over its IPC socket.
- Dictionary and model data continue to download at runtime into the OS app-data directory;
  only fonts, the JLPT vocabulary data, and the app icon are app-bundled resources.
- Parity is defined against the current egui application as the reference implementation.
- The new app targets the same desktop platforms as today (Windows, macOS, Linux).
- A single window with the same overall information architecture is acceptable; an exact pixel
  match of the egui layout is not required, only equivalent capability and better text rendering.
