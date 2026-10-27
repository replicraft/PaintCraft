// Vite owns the actual modules/Worker/Wasm. wasm-bindgen receives this object,
// avoiding copied snippets with missing relative module dependencies.
import {
	decodeFileInWorker,
	codecWorkerBudget,
	admitDdsInWorker,
} from './codec-worker.js';
import {
	PIXELS_MAX,
	ownerBytes,
	matchesOwner,
	refusal,
} from './codec-worker-protocol.js';
export function createCodecService(applicationVersion) {
	let owner;
	let active;
	return Object.freeze({
		applicationVersion: () => applicationVersion,
		protocolVersion: () => 2,
		budget: () => codecWorkerBudget(),
		setOwner(value) {
			ownerBytes(value);
			const changed = owner && !matchesOwner(value, ownerBytes(owner));
			owner = Object.freeze({ ...value });
			if (changed)
				active?.abort(
					refusal('ConflictStaleBase', 'Codec owner changed'),
				);
		},
		cancel() {
			active?.abort();
		},
		ddsExportProtocolVersion: () => 1,
		beginDdsExport(capturedOwner, plan, nonce) {
			if (codecWorkerBudget().quarantined)
				throw refusal('ResourceLimit', 'Codec service needs a reload');
			if (active)
				throw refusal('ResourceLimit', 'A codec job is already active');
			if (!owner || !matchesOwner(owner, ownerBytes(capturedOwner)))
				throw refusal(
					'ConflictStaleBase',
					'Codec source belongs to a stale owner',
				);
			const abort = new AbortController();
			const ticket = admitDdsInWorker({
				owner: capturedOwner,
				plan,
				nonce,
				signal: abort.signal,
				currentOwner: () => owner,
				applicationVersion,
			});
			active = abort;
			let used = false;
			return Object.freeze({
				async run(source) {
					if (used)
						throw refusal(
							'InputSchema',
							'DDS admission already consumed',
						);
					used = true;
					try {
						return await ticket.run(source);
					} finally {
						if (active === abort) active = null;
					}
				},
				release() {
					if (!used) {
						used = true;
						ticket.release();
						if (active === abort) active = null;
					}
				},
			});
		},
		async decodeFile(file, capturedOwner, pixels = PIXELS_MAX, nonce) {
			if (codecWorkerBudget().quarantined)
				throw refusal('ResourceLimit', 'Codec service needs a reload');
			if (active)
				throw refusal(
					'ResourceLimit',
					'A codec import is already active',
				);
			const captured = ownerBytes(capturedOwner);
			if (!owner || !matchesOwner(owner, captured))
				throw refusal(
					'ConflictStaleBase',
					'Codec source belongs to a stale owner',
				);
			const abort = new AbortController();
			active = abort;
			try {
				return await decodeFileInWorker({
					file,
					owner: capturedOwner,
					codec: 2,
					nonce,
					pixels,
					signal: abort.signal,
					currentOwner: () => owner,
					applicationVersion,
				});
			} finally {
				active = null;
			}
		},
	});
}
