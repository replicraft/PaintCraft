import {
	SOURCE_BYTES_MAX,
	OUTPUT_PIXELS_MAX,
	RESERVATION_BYTES,
	fail,
	canonical,
	sha,
	validatePlan,
	validateBinding,
	validateResult,
	validateRequest,
} from './filter-preview-protocol.js';
export {
	SOURCE_BYTES_MAX,
	OUTPUT_PIXELS_MAX,
	LINEAR_MEMORY_BYTES,
} from './filter-preview-protocol.js';
let reserved = false,
	quarantined = false;
export function filterPreviewUsage() {
	return { reserved: reserved ? RESERVATION_BYTES : 0, quarantined };
}
export function beginFilterPreview(input) {
	validatePlan(input);
	const frozen = JSON.parse(canonical(input));
	if (reserved || quarantined)
		throw fail(
			'ResourceLimit',
			'Filter render service is busy or quarantined',
		);
	reserved = true;
	let used = false;
	return {
		cancel() {
			if (!used) {
				used = true;
				reserved = false;
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
				reserved = false;
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
					frameHeld = false,
					latest = -1n,
					retiredThrough = -1n;
				const pending = new Map();
				function hardFail(error) {
					if (closed) return;
					closed = true;
					quarantined = true;
					// terminate is not an acknowledged join; keep this one lease.
					worker.terminate();
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
						if (p.kind === 'open' && data.completed === true)
							completedOpenRefusals.add(error);
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
				let desired = null,
					active = null,
					hashing = null,
					cancelInFlight = null,
					pumpScheduled = false,
					closing = false;
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
				Object.freeze(ready.binding.owner);
				Object.freeze(ready.binding.size);
				const client = {
					binding: Object.freeze(ready.binding),
					update(options) {
						try {
							if (closing || closed)
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
								canonical({
									...options,
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
						if (closed) return { kind: 'closed' };
						if (frameHeld)
							throw fail(
								'ResourceLimit',
								'Release the render frame before closing',
							);
						closing = true;
						await retireThrough(
							latest < 0n ? '0' : latest.toString(),
						);
						const ack = await send('close', {});
						if (ack.kind !== 'closed') {
							const error = fail(
								'CodecFailed',
								'Missing source-close acknowledgment',
							);
							hardFail(error);
							throw error;
						}
						closed = true;
						worker.terminate();
						reserved = false;
						return ack;
					},
				};
				return client;
			} catch (error) {
				if (worker) {
					if (completedOpenRefusals.has(error)) reserved = false;
					else quarantined = true;
					worker.terminate();
				} else reserved = false;
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
