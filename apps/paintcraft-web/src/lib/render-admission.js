// One admission owner for all persistent render protocols in this JS realm.
// Bytes are logical declarations, not allocator, GPU or RSS interception.
const MAIN_BYTES_MAX = 64 * 1024 * 1024;
function fail(message) {
	return Object.assign(new Error(message), { code: 'ResourceLimit' });
}
function extent(bytes) {
	if (!Number.isSafeInteger(bytes) || bytes <= 0)
		throw fail('Render admission byte extent');
}
export function createRenderAdmission() {
	let current = null;
	return Object.freeze({
		usage() {
			return {
				reserved: current?.bytes ?? 0,
				quarantined: current?.quarantined ?? false,
			};
		},
		acquire(bytes) {
			extent(bytes);
			if (current)
				throw fail('Filter render service is busy or quarantined');
			const owner = {
				bytes,
				quarantined: false,
				workerClosed: false,
				mainGranted: false,
				mainBytes: 0,
			};
			current = owner;
			function retire() {
				if (
					current === owner &&
					owner.workerClosed &&
					!owner.quarantined &&
					owner.mainBytes === 0
				)
					current = null;
			}
			function active() {
				if (
					current !== owner ||
					owner.workerClosed ||
					owner.quarantined
				)
					throw fail('Render admission is retired');
			}
			return Object.freeze({
				// Grow only to the actual compiled control quote before preparation.
				resize(next) {
					active();
					extent(next);
					if (owner.mainGranted || next < owner.bytes)
						throw fail(
							'Render control quote changed after preparation',
						);
					owner.bytes = next;
				},
				// This is a slice of existing serialization credit, not extra bytes.
				// Fixed-context Source7 preparation obtains one guard per OPEN.
				reserveMain(bytes) {
					active();
					extent(bytes);
					if (
						bytes > MAIN_BYTES_MAX ||
						bytes > owner.bytes ||
						owner.mainGranted
					)
						throw fail('Render Main preparation allowance');
					owner.mainGranted = true;
					owner.mainBytes = bytes;
					let released = false;
					return Object.freeze({
						release() {
							if (released) return;
							released = true;
							owner.mainBytes = 0;
							retire();
						},
					});
				},
				// Called only after actual close ACK, or before any Worker exists.
				release() {
					owner.workerClosed = true;
					retire();
				},
				// terminate() without a join cannot return this lease's credit.
				quarantine() {
					if (current === owner) owner.quarantined = true;
				},
			});
		},
	});
}
export const renderAdmission = createRenderAdmission();
