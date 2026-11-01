import { renderAdmission } from './render-admission.js';
import { joinableCopy, makeEncodedCopy } from './filter-preview-copy.js';
import {
	capabilityFacts6,
	facts6,
	freezeSource6,
	NOISE_CONTROL_BYTES,
	validateCapabilities6,
	validateRuntimeFacts6,
} from './filter-preview-noise-protocol.js';
import {
	BEGIN_BYTES_MAX,
	encodeProgram,
	freezeSource5,
	OPEN_BYTES_MAX,
	PROGRAM_CONTROL_BYTES,
	requirementDigests,
	unionPairs,
	validateCapabilities,
	validateReady5,
} from './filter-preview-program-protocol.js';
import {
	canonical,
	fail,
	RESERVATION_BYTES,
	SOURCE_BYTES_MAX,
	sha,
	validateBinding,
	validatePlan,
	validateRequest,
	validateResult,
} from './filter-preview-protocol.js';

export {
	LINEAR_MEMORY_BYTES,
	OUTPUT_PIXELS_MAX,
	SOURCE_BYTES_MAX,
} from './filter-preview-protocol.js';

const typedRustErrors = new Set([
	'input.schema',
	'policy.unknown',
	'reference.unknown_alias',
	'reference.unknown_id',
	'target.invalid',
	'capability.unsupported',
	'conflict.stale_base',
	'resource.limit',
	'operation.cancelled',
	'codec.failed',
	'storage.failed',
	'invariant.violation',
]);
export function filterPreviewUsage() {
	return renderAdmission.usage();
}
export function beginFilterPreview(input) {
	return beginWithAdmission(input, renderAdmission);
}
// Embedding hosts and isolated mock realms must supply their logical owner.
// Normal application exports always share the single production broker.
export function createFilterPreviewController(broker) {
	if (
		!broker ||
		typeof broker.acquire !== 'function' ||
		typeof broker.usage !== 'function'
	)
		throw fail('InputSchema', 'Render admission owner required');
	return Object.freeze({
		beginFilterPreview: (input) => beginWithAdmission(input, broker),
		filterPreviewUsage: () => broker.usage(),
	});
}
function beginWithAdmission(input, broker) {
	validatePlan(input);
	const frozen = JSON.parse(
		[5, 6].includes(input?.protocol)
			? encodeProgram(input, OPEN_BYTES_MAX)
			: canonical(input),
	);
	const lease = broker.acquire(
		RESERVATION_BYTES +
			(input?.protocol === 6
				? NOISE_CONTROL_BYTES
				: input?.protocol === 5
					? PROGRAM_CONTROL_BYTES
					: 0),
	);
	let used = false;
	return {
		cancel() {
			if (!used) {
				used = true;
				lease.release();
			}
		},
		async open(source, sourceSha256) {
			if (used)
				throw fail('InputSchema', 'Filter admission already consumed');
			used = true;
			if (
				!(source instanceof Uint8Array) ||
				source.length < 1 ||
				source.length > SOURCE_BYTES_MAX
			) {
				lease.release();
				throw fail('ResourceLimit', 'Filter source allowance');
			}
			let worker;
			const completedOpenRefusals = new WeakSet();
			try {
				if ((await sha(source)) !== sourceSha256)
					throw fail(
						'ConflictStaleBase',
						'Filter source digest changed',
					);
				const plan = { ...frozen, sourceSha256 };
				worker = new Worker(
					new URL('./filter-preview-runtime.js', import.meta.url),
					{ type: 'module' },
				);
				let sequence = 0,
					closed = false,
					closedAck = null,
					closePromise = null,
					frameHeld = false,
					latest = -1n,
					retiredThrough = -1n,
					desired = null,
					active = null,
					hashing = null,
					cancelInFlight = null,
					pumpScheduled = false,
					closing = false;
				const pending = new Map();
				let copying = false;
				function hardFail(error) {
					if (closed) return;
					closed = true;
					lease.quarantine();
					// terminate is not an acknowledged join; keep this one lease.
					worker.terminate();
					if (desired) {
						desired.reject(error);
						desired = null;
					}
					if (hashing) {
						hashing.reject(error);
						hashing = null;
					}
					if (active) active.reject(error);
					for (const p of pending.values()) {
						clearTimeout(p.timer);
						p.reject(error);
					}
					pending.clear();
				}
				function send(kind, body, transfer = []) {
					if (closed)
						return Promise.reject(
							fail('CodecFailed', 'Filter Worker closed'),
						);
					if (pending.size >= 3)
						return Promise.reject(
							fail(
								'ResourceLimit',
								'Filter Worker control queue full',
							),
						);
					if (++sequence > 0xffffffff)
						return Promise.reject(
							fail(
								'ResourceLimit',
								'Filter Worker request sequence exhausted',
							),
						);
					const id = sequence;
					return new Promise((resolve, reject) => {
						const timer = setTimeout(
							() =>
								hardFail(
									fail(
										'CodecFailed',
										'Filter Worker did not acknowledge work',
									),
								),
							120000,
						);
						pending.set(id, { resolve, reject, timer, kind });
						try {
							worker.postMessage({ kind, id, ...body }, transfer);
						} catch (error) {
							pending.delete(id);
							clearTimeout(timer);
							reject(error);
						}
					});
				}
				worker.onerror = () =>
					hardFail(fail('CodecFailed', 'Filter render Worker trap'));
				worker.onmessage = ({ data }) => {
					const p = pending.get(data?.id);
					if (!p) return;
					pending.delete(data.id);
					clearTimeout(p.timer);
					if (data.kind === 'error') {
						const error = fail(
							typeof data.code === 'string' &&
								data.code.length <= 64
								? data.code
								: 'CodecFailed',
							typeof data.message === 'string'
								? data.message.slice(0, 4096)
								: 'Render Worker failed',
						);
						const completed =
							data.completed === true &&
							typedRustErrors.has(data.code) &&
							typeof data.message === 'string' &&
							data.message.length <= 4096;
						if (
							(p.kind === 'open' ||
								([5, 6].includes(plan.protocol) &&
									p.kind === 'hello')) &&
							completed
						)
							completedOpenRefusals.add(error);
						if (
							!completed ||
							p.kind === 'cancel' ||
							p.kind === 'close'
						)
							hardFail(error);
						p.reject(error);
					} else if (data.kind === 'retired' && p.update)
						p.reject(
							fail(
								'OperationCancelled',
								'Filter preview superseded',
							),
						);
					else p.resolve(data);
				};
				let capabilities = null,
					runtimeFacts = null;
				async function resolveSubset(pairs, expected) {
					let entries = pairs.map((p) =>
						(capabilities ?? []).find(
							(e) =>
								e.profile_id === p.profile_id &&
								e.codec_id === p.codec_id,
						),
					);
					if (capabilities === null || entries.some((e) => !e)) {
						const reply = await send('hello', {
							registryContract: plan.protocol === 6 ? 2 : 1,
							...(plan.protocol === 6
								? { protocol: 6, workload: plan.workload }
								: {}),
							pairs,
						});
						try {
							if (plan.protocol === 6) {
								entries = await validateCapabilities6(
									reply,
									pairs,
									expected,
									plan.workload,
								);
								validateRuntimeFacts6(reply.runtimeFacts);
								if (
									runtimeFacts &&
									encodeProgram(
										runtimeFacts,
										OPEN_BYTES_MAX,
									) !==
										encodeProgram(
											reply.runtimeFacts,
											OPEN_BYTES_MAX,
										)
								)
									throw fail(
										'CodecFailed',
										'Source6 runtime facts changed',
									);
								runtimeFacts = Object.freeze({
									controlQuote: Object.freeze({
										...reply.runtimeFacts.controlQuote,
									}),
									slicePolicy: Object.freeze({
										...reply.runtimeFacts.slicePolicy,
									}),
								});
							} else
								entries = await validateCapabilities(
									reply,
									pairs,
									expected,
								);
						} catch (error) {
							hardFail(error);
							throw error;
						}
						// Retain only one bounded subset, never an accumulating
						// history of all algorithms requested during this owner.
						capabilities = entries;
					} else {
						const actual =
							plan.protocol === 6
								? await capabilityFacts6(
										pairs,
										entries,
										plan.workload,
									)
								: await requirementDigests(pairs, entries);
						if (
							JSON.stringify(actual) !==
							JSON.stringify(
								plan.protocol === 6
									? facts6(expected)
									: {
											count: expected.count,
											requiredSha256:
												expected.requiredSha256,
											resolutionSha256:
												expected.resolutionSha256,
										},
							)
						)
							throw fail(
								'CodecFailed',
								'Prepared cached capability subset mismatch',
							);
					}
				}
				if ([5, 6].includes(plan.protocol))
					await resolveSubset(
						plan.sourceRequirements.pairs,
						plan.sourceRequirements,
					);
				const bytes = source.slice();
				const ready = await send(
					'open',
					{ source: bytes.buffer, plan },
					[bytes.buffer],
				);
				if (ready.kind !== 'ready')
					throw fail(
						'CodecFailed',
						'Missing render source acknowledgment',
					);
				validateBinding(ready.binding, plan);
				if (plan.protocol === 5) validateReady5(ready, plan);
				if (plan.protocol === 6 && !runtimeFacts)
					throw fail('CodecFailed', 'Source6 ABI handshake missing');
				function cancelled() {
					return fail(
						'OperationCancelled',
						'Filter preview superseded',
					);
				}
				function schedulePump() {
					if (!pumpScheduled) {
						pumpScheduled = true;
						setTimeout(() => {
							pumpScheduled = false;
							pump();
						}, 0);
					}
				}
				async function retireThrough(generation) {
					const value = BigInt(generation);
					if (value > retiredThrough) retiredThrough = value;
					if (desired && desired.generation <= value) {
						desired.reject(cancelled());
						desired = null;
					}
					if (!cancelInFlight) {
						cancelInFlight = send('cancel', { generation });
						cancelInFlight
							.finally(() => {
								cancelInFlight = null;
								schedulePump();
							})
							.catch(() => {});
					}
					const ack = await cancelInFlight;
					if (ack.kind !== 'retired') {
						const error = fail(
							'CodecFailed',
							'Missing render retirement acknowledgment',
						);
						hardFail(error);
						throw error;
					}
					return { ...ack, generation };
				}
				async function pump() {
					if (
						closed ||
						closing ||
						frameHeld ||
						copying ||
						active ||
						hashing ||
						cancelInFlight ||
						!desired
					)
						return;
					const job = desired;
					desired = null;
					hashing = job;
					try {
						const operationSha256 = await sha(
							new TextEncoder().encode(
								canonical(job.request.operation),
							),
						);
						if ([5, 6].includes(plan.protocol))
							await resolveSubset(
								unionPairs(
									plan.sourceRequirements.pairs,
									job.request.operation,
								),
								job.request.unionRequirements,
							);
						if (
							closing ||
							closed ||
							job.generation !== latest ||
							job.generation <= retiredThrough
						)
							throw cancelled();
						hashing = null;
						active = job;
						const promise = send('update', {
							request: job.request,
						});
						const slot = pending.get(sequence);
						if (slot) slot.update = true;
						const result = await promise;
						if (
							job.generation !== latest ||
							job.generation <= retiredThrough ||
							closing
						)
							throw cancelled();
						try {
							validateResult(
								result,
								{ ...job.request, operationSha256 },
								plan,
							);
						} catch (error) {
							hardFail(error);
							throw error;
						}
						result.release = () => {};
						if (result.kind === 'frame') {
							frameHeld = true;
							let released = false;
							result.release = () => {
								if (!released) {
									released = true;
									frameHeld = false;
									schedulePump();
								}
							};
						}
						job.resolve(result);
					} catch (error) {
						job.reject(error);
					} finally {
						if (hashing === job) hashing = null;
						if (active === job) active = null;
						schedulePump();
					}
				}
				if (plan.protocol === 5) freezeSource5(ready.binding);
				if (plan.protocol === 6) freezeSource6(ready.binding);
				Object.freeze(ready.binding.owner);
				Object.freeze(ready.binding.size);
				const rawEncodedCopy =
					plan.protocol === 6
						? makeEncodedCopy({
								plan,
								send,
								start(options) {
									if (
										closing ||
										closed ||
										frameHeld ||
										active ||
										hashing ||
										desired ||
										copying ||
										cancelInFlight
									)
										throw fail(
											'ResourceLimit',
											'Flat output slot busy',
										);
									const generation = BigInt(
										options.generation,
									);
									if (generation <= latest)
										throw fail(
											'ConflictStaleBase',
											'Flat output generation',
										);
									const request = JSON.parse(
										encodeProgram(
											{
												...options,
												owner: plan.owner,
												draft: plan.draft,
												sourceSha256,
												protocol: 6,
												registryContract: 2,
												workloadSha256:
													plan.workload.sha256,
											},
											BEGIN_BYTES_MAX,
										),
									);
									latest = generation;
									copying = true;
									return request;
								},
								finish() {
									copying = false;
									frameHeld = false;
									schedulePump();
								},
								hold() {
									frameHeld = true;
								},
								markUpdate() {
									const slot = pending.get(sequence);
									if (slot) slot.update = true;
								},
								checkCurrent(request) {
									if (
										closing ||
										closed ||
										BigInt(request.generation) <=
											retiredThrough
									)
										throw fail(
											'OperationCancelled',
											'Flat output cancelled',
										);
								},
							})
						: undefined;
				const copyFinalization =
					plan.protocol === 6 ? joinableCopy(rawEncodedCopy) : null;
				const encodedCopy = copyFinalization
					? (...args) => copyFinalization.run(...args)
					: undefined;
				const client = {
					...(plan.protocol === 6 ? { encodedCopy } : {}),
					binding: Object.freeze(ready.binding),
					...(plan.protocol === 6 ? { runtimeFacts } : {}),
					update(options) {
						try {
							if (closing || closed || copying)
								throw fail(
									'CodecFailed',
									'Filter Worker closing',
								);
							validateRequest(options, plan);
							if (frameHeld)
								throw fail(
									'ResourceLimit',
									'Release the previous render frame before updating',
								);
							const generation = BigInt(options.generation);
							if (generation <= latest)
								throw fail(
									'ConflictStaleBase',
									'Filter generation did not advance',
								);
							const request = JSON.parse(
								([5, 6].includes(plan.protocol)
									? (value) =>
											encodeProgram(
												value,
												BEGIN_BYTES_MAX,
											)
									: canonical)({
									...options,
									...(plan.protocol === 6
										? {
												protocol: 6,
												registryContract: 2,
												workloadSha256:
													plan.workload.sha256,
											}
										: {}),
									owner: plan.owner,
									draft: plan.draft,
									sourceSha256,
								}),
							);
							latest = generation;
							if (desired) desired.reject(cancelled());
							const result = new Promise((resolve, reject) => {
								desired = {
									request,
									generation,
									resolve,
									reject,
								};
							});
							if (active && !cancelInFlight)
								retireThrough(active.request.generation).catch(
									hardFail,
								);
							schedulePump();
							return result;
						} catch (error) {
							return Promise.reject(error);
						}
					},
					base(options) {
						return this.update({
							...options,
							operation: null,
							target: null,
							render: true,
						});
					},
					async cancel(generation) {
						if (
							typeof generation !== 'string' ||
							!/^(0|[1-9]\d{0,19})$/.test(generation) ||
							BigInt(generation) > 0xffffffffffffffffn
						)
							throw fail('InputSchema', 'Cancel generation');
						return retireThrough(generation);
					},
					async close() {
						if (closed) {
							if (closedAck) return closedAck;
							throw fail(
								'CodecFailed',
								'Filter Worker termination was not acknowledged',
							);
						}
						if (closePromise) return closePromise;
						if (frameHeld)
							throw fail(
								'ResourceLimit',
								'Release the render frame before closing',
							);
						closing = true;
						closePromise = (async () => {
							await retireThrough(
								latest < 0n ? '0' : latest.toString(),
							);
							if (copyFinalization) await copyFinalization.join();
							const ack = await send('close', {});
							if (ack.kind !== 'closed')
								throw fail(
									'CodecFailed',
									'Missing source-close acknowledgment',
								);
							closedAck = Object.freeze({
								kind: 'closed',
								id: ack.id,
							});
							closed = true;
							worker.terminate();
							lease.release();
							return closedAck;
						})().catch((error) => {
							hardFail(error);
							throw error;
						});
						return closePromise;
					},
				};
				return client;
			} catch (error) {
				if (worker) {
					if (completedOpenRefusals.has(error)) lease.release();
					else lease.quarantine();
					worker.terminate();
				} else lease.release();
				throw error;
			}
		},
	};
}
export async function openFilterPreview(source, plan) {
	const admission = beginFilterPreview(plan);
	try {
		return await admission.open(source, plan.sourceSha256);
	} catch (error) {
		admission.cancel();
		throw error;
	}
}
