import { renderAdmission } from './render-admission.js';
import {
	RESERVATION_BYTES,
	SOURCE_BYTES_MAX,
} from './filter-preview-protocol.js';
import { appearanceChannel7 } from './filter-preview-appearance-channel.js';
import { appearanceSession7 } from './filter-preview-appearance-session.js';
import {
	APPEARANCE7,
	equal7,
	fail7,
	freeze7,
	numericalDigests7,
	numericalEntry7,
	shape7,
	snapshot7,
	validateCapabilities7,
	validateHello7,
	validatePlan7,
	validatePreplan7,
	validateReady7,
	verifyPlan7,
} from './filter-preview-appearance-protocol.js';
const createWorker = () =>
	new Worker(
		new URL('./layer-appearance-preview-runtime.js', import.meta.url),
		{ type: 'module' },
	);
export function beginLayerAppearancePreview(preplan) {
	return begin7(preplan, renderAdmission, createWorker);
}
// Explicitly supplied broker for embedding/mock realms; application uses singleton.
export function createLayerAppearanceController7(
	broker,
	factory,
	timeoutMs = 120000,
) {
	if (
		!broker ||
		typeof broker.acquire !== 'function' ||
		typeof factory !== 'function'
	)
		throw fail7('InputSchema', 'Source7 owner/factory');
	return Object.freeze({
		beginLayerAppearancePreview: (preplan) =>
			begin7(preplan, broker, factory, timeoutMs),
	});
}
function begin7(input, broker, factory, timeoutMs) {
	const preplan = freeze7(snapshot7(input, 4096));
	validatePreplan7(preplan);
	const lease = broker.acquire(RESERVATION_BYTES);
	let channel,
		opening = null,
		session = null,
		closing = false,
		consumed = false,
		readySeen = false,
		mainGranted = false,
		closePromise;
	try {
		channel = appearanceChannel7(factory(), lease, timeoutMs);
	} catch (error) {
		lease.release();
		throw error;
	}
	const ready = (async () => {
		const response = await channel.send('runtime-ready', {
			noise: preplan.mode === 'noise',
		});
		shape7(response, ['kind', 'runtimeFacts', 'controlQuote']);
		if (response.kind !== 'runtime-ready')
			throw fail7('CodecFailed', 'Source7 runtime handshake');
		const value = {
			runtimeFacts: response.runtimeFacts,
			controlQuote: response.controlQuote,
		};
		validateReady7(value, preplan.mode);
		if (BigInt(value.controlQuote) > 67108864n)
			throw fail7('ResourceLimit', 'Source7 broker envelope');
		lease.resize(RESERVATION_BYTES + Number(value.controlQuote));
		readySeen = true;
		return freeze7(value);
	})().catch((error) => {
		if (!closing) channel.abort(error);
		throw error;
	});
	// The host may abandon a synchronous begin before observing ready.
	ready.catch(() => {});
	async function close() {
		if (closePromise) return closePromise;
		if (session) return session.close();
		closing = true;
		// Worker joins pending bootstrap tasks; ACK cancels their suppressed replies.
		closePromise = (async () => {
			const ack = await channel.close();
			// ACK rejects suppressed bootstrap calls. Join remaining local hashes
			// and drop the exclusively owned intake buffer before returning credit.
			try {
				await ready;
			} catch {
				/* Genuine joined cancellation. */
			}
			if (opening) {
				try {
					await opening;
				} catch {
					/* Refusal/cancellation. */
				}
			}
			channel.releaseJoined();
			return ack;
		})().catch((error) => {
			channel.abort(error);
			throw error;
		});
		closePromise.catch(() => {});
		return closePromise;
	}
	return Object.freeze({
		ready,
		reserveMain(bytes) {
			if (closing || consumed || !readySeen)
				throw fail7(
					'OperationCancelled',
					'Source7 preparation not ready',
				);
			const guard = lease.reserveMain(bytes);
			mainGranted = true;
			return guard;
		},
		cancel() {
			close().catch(() => {});
		},
		close,
		open(source, sourceSha256, finalControl) {
			if (consumed || closing || !mainGranted)
				return Promise.reject(
					fail7('OperationCancelled', 'Source7 admission consumed'),
				);
			consumed = true;
			let owned, control;
			try {
				shape7(finalControl, ['plan', 'hello']);
				const properties =
					Object.getOwnPropertyDescriptors(finalControl);
				if (
					!Object.hasOwn(properties.plan, 'value') ||
					!Object.hasOwn(properties.hello, 'value')
				)
					throw fail7(
						'InputSchema',
						'Source7 preparation wrapper accessor',
					);
				// The transport wrapper is not one wire control. Preserve each
				// existing bound without expanding the shared encoder's maximum.
				control = {
					plan: snapshot7(properties.plan.value, 65536),
					hello: snapshot7(properties.hello.value, 131072),
				};
				validatePlan7(control.plan);
				validateHello7(control.hello);
				const { plan, hello } = control;
				if (
					sourceSha256 !== plan.sourceSha256 ||
					!equal7(plan.owner, preplan.owner) ||
					plan.draft !== preplan.draft ||
					plan.admission.mode !== preplan.mode ||
					!equal7(plan.admission, hello.admission) ||
					!equal7(plan.slice, hello.slice) ||
					!equal7(hello.appearance, APPEARANCE7)
				)
					throw fail7(
						'ConflictStaleBase',
						'Source7 sealed preparation identity',
					);
				if (
					!(source instanceof Uint8Array) ||
					Object.getPrototypeOf(source) !== Uint8Array.prototype ||
					!(source.buffer instanceof ArrayBuffer) ||
					Object.getPrototypeOf(source.buffer) !==
						ArrayBuffer.prototype ||
					source.byteOffset !== 0 ||
					source.byteLength !== source.buffer.byteLength ||
					!source.byteLength ||
					source.byteLength > SOURCE_BYTES_MAX ||
					source.buffer.resizable === true
				)
					throw fail7(
						'InputSchema',
						'Source7 exclusive full-span source',
					);
				// Ownership changes BEFORE any await, including ready/HELLO/digests.
				owned = structuredClone(source, { transfer: [source.buffer] });
			} catch (error) {
				close().catch(() => {});
				return Promise.reject(error);
			}
			opening = (async () => {
				try {
					const { runtimeFacts } = await ready;
					if (closing)
						throw fail7(
							'OperationCancelled',
							'Source7 open cancelled',
						);
					const { plan, hello } = control;
					const expected = await numericalDigests7(
						hello.pairs,
						hello.pairs.map(numericalEntry7),
					);
					const capabilities = await channel.send('hello', hello);
					const entries = await validateCapabilities7(
						capabilities,
						hello.pairs,
						expected,
						plan.admission,
						runtimeFacts,
					);
					const sourceEntries = plan.sourceRequirements.pairs.map(
						(pair) => {
							const at = hello.pairs.findIndex((p) =>
								equal7(p, pair),
							);
							if (at < 0)
								throw fail7(
									'CodecFailed',
									'Source7 HELLO omits source pair',
								);
							return entries[at];
						},
					);
					const frozenPlan = await verifyPlan7(plan, sourceEntries);
					if (closing)
						throw fail7(
							'OperationCancelled',
							'Source7 source transfer cancelled',
						);
					const reply = await channel.send(
						'open',
						{ source: owned.buffer, plan: frozenPlan },
						[owned.buffer],
					);
					owned = null;
					shape7(reply, ['kind', 'binding']);
					if (
						reply.kind !== 'ready' ||
						!equal7(reply.binding, frozenPlan)
					)
						throw fail7(
							'CodecFailed',
							'Source7 original OPEN acknowledgment',
						);
					session = appearanceSession7(channel, frozenPlan, expected);
					if (closing)
						throw fail7(
							'OperationCancelled',
							'Source7 opened during close',
						);
					return session;
				} catch (error) {
					owned = null;
					// Completed refusals still require a genuine join; no auto-reopen.
					if (!closing) close().catch(() => {});
					throw error;
				} finally {
					control = null;
				}
			})();
			opening.catch(() => {});
			return opening;
		},
	});
}
