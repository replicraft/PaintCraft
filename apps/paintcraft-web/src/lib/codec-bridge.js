import { DDS_PROTOCOL_VERSION as DDS_PREPARED_PROTOCOL_VERSION } from './dds-prepared-worker-protocol.js';
import { MSP_PROTOCOL_VERSION as MSP_PREPARED_PROTOCOL_VERSION } from './msp-prepared-worker-protocol.js';
import { DDS_PROTOCOL_VERSION as DDS_SPATIAL_PROTOCOL_VERSION } from './dds-spatial-worker-protocol.js';
import { MSP_PROTOCOL_VERSION as MSP_SPATIAL_PROTOCOL_VERSION } from './msp-spatial-worker-protocol.js';
import { DDS_PROTOCOL_VERSION as DDS_CATALOG_PROTOCOL_VERSION } from './dds-catalog-worker-protocol.js';
import { MSP_PROTOCOL_VERSION as MSP_CATALOG_PROTOCOL_VERSION } from './msp-catalog-worker-protocol.js';
// Vite owns the actual modules/Worker/Wasm. wasm-bindgen receives this object,
// avoiding copied snippets with missing relative module dependencies.
import {
	admitDdsInWorker,
	admitMspInWorker,
	codecWorkerBudget,
	decodeFileInWorker,
	tiffFileInWorker,
} from './codec-worker.js';
import {
	matchesOwner,
	ownerBytes,
	PIXELS_MAX,
	refusal,
} from './codec-worker-protocol.js';
import { DDS_PROTOCOL_VERSION } from './dds-worker-protocol.js';
import { MSP_PROTOCOL_VERSION } from './msp-worker-protocol.js';
import {
	freezeTiffPlan,
	TIFF_PROTOCOL_VERSION,
} from './tiff-page-worker-protocol.js';
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
		return job(
			abort,
			(source, finalizedPlan) => ticket.run(source, finalizedPlan),
			() => ticket.release(),
		);
	}
	function job(abort, run, releaseUnused) {
		active = abort;
		let used = false;
		return Object.freeze({
			// This handle belongs only to this admission. A GUI may still hold
			// it after completion while another operation owns the service.
			cancel() {
				if (active === abort) abort.abort();
			},
			async run(source, finalizedPlan) {
				if (used)
					throw refusal(
						'InputSchema',
						'Codec admission already consumed',
					);
				used = true;
				try {
					return await run(source, finalizedPlan);
				} finally {
					if (active === abort) active = null;
				}
			},
			release() {
				if (!used) {
					used = true;
					releaseUnused();
					if (active === abort) active = null;
				}
			},
		});
	}
	function beginDecodeFile(file, capturedOwner, pixels, nonce, codec = 2) {
		if (codecWorkerBudget().quarantined)
			throw refusal('ResourceLimit', 'Codec service needs a reload');
		if (active)
			throw refusal('ResourceLimit', 'A codec import is already active');
		const captured = Object.freeze({ ...capturedOwner });
		if (!owner || !matchesOwner(owner, ownerBytes(captured)))
			throw refusal(
				'ConflictStaleBase',
				'Codec source belongs to a stale owner',
			);
		const abort = new AbortController();
		return job(
			abort,
			() =>
				decodeFileInWorker({
					file,
					owner: captured,
					codec,
					nonce,
					pixels,
					signal: abort.signal,
					currentOwner: () => owner,
					applicationVersion,
				}),
			() => {},
		);
	}
	function beginTiffFile(file, capturedOwner, plan, nonce) {
		if (codecWorkerBudget().quarantined)
			throw refusal('ResourceLimit', 'Codec service needs a reload');
		if (active)
			throw refusal('ResourceLimit', 'A codec job is already active');
		const captured = Object.freeze({ ...capturedOwner }),
			selection = freezeTiffPlan(plan);
		if (!owner || !matchesOwner(owner, ownerBytes(captured)))
			throw refusal(
				'ConflictStaleBase',
				'TIFF source belongs to a stale owner',
			);
		const abort = new AbortController();
		return job(
			abort,
			() =>
				tiffFileInWorker({
					file,
					owner: captured,
					plan: selection,
					nonce,
					signal: abort.signal,
					currentOwner: () => owner,
					applicationVersion,
				}),
			() => {},
		);
	}

	return Object.freeze({
		applicationVersion: () => applicationVersion,
		protocolVersion: () => 2,
		budget: () => codecWorkerBudget(),
		tiffPageProtocolVersion: () => TIFF_PROTOCOL_VERSION,
		beginTiffFile,
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
		ddsExportProtocolVersion: () => DDS_PROTOCOL_VERSION,
		ddsSpatialExportProtocolVersion: () => DDS_SPATIAL_PROTOCOL_VERSION,
		ddsCatalogExportProtocolVersion: () => DDS_CATALOG_PROTOCOL_VERSION,
		ddsPreparedExportProtocolVersion: () => DDS_PREPARED_PROTOCOL_VERSION,
		beginDdsExport(capturedOwner, plan, nonce) {
			return beginExport(capturedOwner, plan, nonce, admitDdsInWorker);
		},
		mspExportProtocolVersion: () => MSP_PROTOCOL_VERSION,
		mspSpatialExportProtocolVersion: () => MSP_SPATIAL_PROTOCOL_VERSION,
		mspCatalogExportProtocolVersion: () => MSP_CATALOG_PROTOCOL_VERSION,
		mspPreparedExportProtocolVersion: () => MSP_PREPARED_PROTOCOL_VERSION,
		beginMspExport(capturedOwner, plan, nonce) {
			return beginExport(capturedOwner, plan, nonce, admitMspInWorker);
		},
		beginDecodeFile(file, capturedOwner, pixels = PIXELS_MAX, nonce) {
			return beginDecodeFile(file, capturedOwner, pixels, nonce);
		},
		beginDecodeIntegerFile(
			file,
			capturedOwner,
			pixels = PIXELS_MAX,
			nonce,
		) {
			return beginDecodeFile(file, capturedOwner, pixels, nonce, 9);
		},
		decodeFile(file, capturedOwner, pixels = PIXELS_MAX, nonce) {
			// Preserve the old always-Promise API, including synchronous guard
			// failures, while using the same job lifetime as explicit tickets.
			try {
				return beginDecodeFile(
					file,
					capturedOwner,
					pixels,
					nonce,
				).run();
			} catch (error) {
				return Promise.reject(error);
			}
		},
	});
}
