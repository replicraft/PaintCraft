// Application entry uses the existing singleton. Routing remains an explicit host gate.
import { renderAdmission } from './render-admission.js';
import { blendChannel8 } from './layer-blend-preview-channel.js';
import { blendSession8 } from './layer-blend-preview-session.js';
import {
	validateOpen8,
	validateSourceBuffer8,
} from './layer-blend-preview-protocol.js';
import {
	CONTROL_JS8,
	hBytes8,
	profile8,
	verifyOpenCompiled8,
} from './layer-blend-preview-wire.js';
import { fail8, shape8 } from './layer-blend-preview-common.js';
const RESERVATION8 = 448 * 1024 * 1024;
const createWorker = () =>
	new Worker(new URL('./layer-blend-preview-runtime.js', import.meta.url), {
		type: 'module',
	});
export function beginLayerBlendPreview8() {
	return createLayerBlendController8(renderAdmission, createWorker).begin();
}
// Only trusted Main calls this with its own defining compiled storage quote.
// Rust checks the returned native guard's exact bytes before creating its F.
export function beginLayerBlendPreview8WithHostControl(controlBytes) {
	return createLayerBlendController8(
		renderAdmission,
		createWorker,
	).beginWithHostControl(controlBytes);
}
// Embedders supply their EXISTING realm broker; this constructs no extra pool.
export function createLayerBlendController8(
	broker,
	factory,
	timeoutMs = 120000,
) {
	if (
		!broker ||
		typeof broker.acquire !== 'function' ||
		typeof factory !== 'function'
	)
		fail8('InputSchema', 'Source8 existing broker/Worker factory');
	return Object.freeze({
		begin: () => begin(broker, factory, timeoutMs),
		beginWithHostControl(controlBytes) {
			if (
				!Number.isSafeInteger(controlBytes) ||
				controlBytes <= 0 ||
				!Number.isSafeInteger(RESERVATION8 + CONTROL_JS8 + controlBytes)
			)
				fail8(
					'InputSchema',
					'Source8 compiled Main host control quote',
				);
			return begin(broker, factory, timeoutMs, controlBytes);
		},
	});
}
function begin(broker, factory, timeoutMs, controlBytes = 0) {
	let hostControl = null;
	const lease = broker.acquire(RESERVATION8);
	// Known bounded JS metadata credit precedes even Worker/READY/profile setup.
	try {
		lease.resize(RESERVATION8 + CONTROL_JS8 + controlBytes);
		if (controlBytes) hostControl = lease.reserveHostControl(controlBytes);
	} catch (error) {
		hostControl?.release();
		lease.release();
		throw error;
	}
	let channel, worker;
	try {
		worker = factory();
		channel = blendChannel8(worker, lease, timeoutMs);
	} catch (error) {
		if (worker) {
			lease.quarantine();
			worker.terminate();
		} else lease.release(); // actual factory returned no Worker
		hostControl?.release(); // no application F exists before begin returns
		throw error;
	}
	let facts = null,
		profile = null,
		mainGranted = false,
		consumed = false;
	let owner = null,
		session = null,
		opening = null,
		closeTask = null;
	let retirement = null;
	let mainResolve;
	let mainRetired = false;
	let mainGone = Promise.resolve();
	function retireOwner() {
		if (!owner) return Promise.resolve();
		if (!retirement)
			retirement = Promise.resolve().then(() => owner.retire());
		return retirement;
	}
	const ready = (async () => {
		const reply = await channel.ready;
		facts = reply.runtimeFacts;
		profile = await profile8(facts);
		// No growth after the one original Main grant, per singleton broker contract.
		lease.resize(
			RESERVATION8 + hBytes8(facts) + CONTROL_JS8 + controlBytes,
		);
		return Object.freeze({ ready: reply, profileSha256: profile });
	})();
	ready.catch((error) => channel.abort(error));
	async function close() {
		if (closeTask) return closeTask;
		closeTask = (async () => {
			try {
				// Join READY, profile verification and compiled-H credit negotiation
				// before an empty close can retire the broker lease.
				await ready;
				// Join OPEN including a typed failed-OPEN consuming close. Unknown
				// failures quarantine; they are never reclassified by a live getter.
				if (opening) {
					try {
						await opening;
					} catch {
						/* channel retains exact join state */
					}
				}
				if (session) await session.close();
				else {
					await channel.close('0');
					await retireOwner();
				}
				await mainGone;
				channel.releaseJoined();
			} catch (error) {
				lease.quarantine();
				channel.abort(error);
				// Retire local actual owners even when Worker credit is quarantined.
				try {
					await retireOwner();
				} catch {
					/* cannot claim Main retired */
				}
				throw error;
			}
		})();
		return closeTask;
	}
	const controller = Object.freeze({
		ready,
		close,
		reserveMain(bytes) {
			if (!facts || !profile || closeTask || mainGranted)
				fail8('ConflictStaleBase', 'Source8 Main preparation phase');
			const credit = lease.reserveMain(bytes);
			mainGranted = true;
			mainGone = new Promise((resolve) => {
				mainResolve = resolve;
			});
			let retired = false;
			return Object.freeze({
				release() {
					// Host must put this LAST behind actual source/output data, scopes and
					// accounts. A capacity number or ArrayBuffer detach is not retirement.
					if (retired) return;
					retired = true;
					mainRetired = true;
					credit.release();
					mainResolve();
				},
			});
		},
		open(control, source, mainOwner) {
			if (consumed || closeTask || !mainGranted)
				fail8(
					'ConflictStaleBase',
					'Source8 original Main grant required',
				);
			const captured = validateOpen8(control);
			validateSourceBuffer8(source, captured);
			verifyOpenCompiled8(captured, facts, profile);
			shape8(mainOwner, ['admitFrame', 'current', 'retire']);
			for (const f of Object.values(mainOwner))
				if (typeof f !== 'function')
					fail8('InputSchema', 'Source8 real Main owner');
			consumed = true;
			owner = mainOwner;
			opening = (async () => {
				try {
					await channel.open(captured, source);
				} finally {
					source = null;
				}
				session = blendSession8(channel, captured, {
					admitFrame(bytes, request) {
						if (mainRetired || closeTask)
							fail8(
								'OperationCancelled',
								'Source8 original Main owner retired',
							);
						return owner.admitFrame(bytes, request);
					},
					current: (request) =>
						!mainRetired && !closeTask && owner.current(request),
					retire: retireOwner,
				});
				return session;
			})();
			return opening;
		},
	});
	// close joins Worker/S without awaiting the host guard that owns its F.
	return hostControl
		? Object.freeze({ controller, hostControl })
		: controller;
}
