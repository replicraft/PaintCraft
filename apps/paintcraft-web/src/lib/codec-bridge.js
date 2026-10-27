// Vite owns the actual modules/Worker/Wasm. wasm-bindgen receives this object,
// avoiding copied snippets with missing relative module dependencies.
import {
	admitDdsInWorker,
	admitMspInWorker,
	codecWorkerBudget,
	decodeFileInWorker,
} from './codec-worker.js';
import {
	matchesOwner,
	ownerBytes,
	PIXELS_MAX,
	refusal,
} from './codec-worker-protocol.js';
export function createCodecService(applicationVersion) {
	let owner;
	let active;
	function beginExport(capturedOwner, plan, nonce, admit) {
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
		const ticket = admit({
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
			// This handle belongs only to this admission. A GUI may still hold
			// it after completion while another operation owns the service.
			cancel() {
				if (active === abort) abort.abort();
			},
			async run(source) {
				if (used)
					throw refusal(
						'InputSchema',
						'Export admission already consumed',
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
	}
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
			return beginExport(capturedOwner, plan, nonce, admitDdsInWorker);
		},
		mspExportProtocolVersion: () => 1,
		beginMspExport(capturedOwner, plan, nonce) {
			return beginExport(capturedOwner, plan, nonce, admitMspInWorker);
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
