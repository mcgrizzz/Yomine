// Open-flags only — every modal stages its own edits and hydrates them on open.

import { writable } from 'svelte/store';

export const ignoreModalOpen = writable(false);
export const websocketModalOpen = writable(false);
export const ankiModalOpen = writable(false);
export const frequencyModalOpen = writable(false);
export const posModalOpen = writable(false);
export const analyzerModalOpen = writable(false);
export const setupModalOpen = writable(false);
export const asbplayerModalOpen = writable(false);
export const appearanceModalOpen = writable(false);

export const openIgnoreModal = (): void => ignoreModalOpen.set(true);
export const openWebsocketModal = (): void => websocketModalOpen.set(true);
export const openAnkiModal = (): void => ankiModalOpen.set(true);
export const openFrequencyModal = (): void => frequencyModalOpen.set(true);
export const openPosModal = (): void => posModalOpen.set(true);
export const openAnalyzerModal = (): void => analyzerModalOpen.set(true);
export const openSetupModal = (): void => setupModalOpen.set(true);
export const openAsbplayerModal = (): void => asbplayerModalOpen.set(true);
export const openAppearanceModal = (): void => appearanceModalOpen.set(true);
