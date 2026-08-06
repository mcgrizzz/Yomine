/** Two-step dismissal for a modal with staged edits: the first request arms, the second closes. */
export function dirtyGuard(isDirty: () => boolean, close: () => void) {
	let armed = $state(false);
	return {
		get armed() {
			return armed;
		},
		disarm: () => (armed = false),
		request: () => {
			if (isDirty() && !armed) {
				armed = true;
				return;
			}
			armed = false;
			close();
		}
	};
}
