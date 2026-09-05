import { writable } from 'svelte/store';

/** View-only preference; its independent module avoids the settings/controls cycle. */
export const showPossibleKnownMatches = writable(true);
