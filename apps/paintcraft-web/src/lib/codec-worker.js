// Isolated browser service; UI reader admission is qualified separately.
import {
	capture,
	encodeRequest,
	decodeResponse,
	isCompletedRefusal,
	ownerBytes,
	nonceBytes,
	matchesOwner,
	refusal,
	LINEAR_MEMORY_BYTES,
	OUTPUT_BYTES_MAX,
	INPUT_BYTES_MAX,
	REQUEST_HEADER,
} from './codec-worker-protocol.js';
let reserved = 0;
let quarantined = null;
export function codecWorkerBudget() {
	return Object.freeze({
		reservedBytes: reserved,
		pendingJobs: reserved ? 1 : 0,
		quarantined: quarantined !== null,
		quarantineReason: quarantined,
	});
}
function reserve(bytes) {
	if (reserved)
		throw refusal(
			'ResourceLimit',
			'A codec Worker result is still admitted',
		);
	reserved = bytes;
	let released = false;
	return {
		release() {
			if (!released && !quarantined) {
				released = true;
				reserved = 0;
			}
		},
		quarantine() {
			if (!released) quarantined = 'CodecFailed';
		},
	};
}
async function decodeAdmitted({
	source,
	sourceLength,
	readSource,
	owner,
	nonce,
	codec,
	pixels,
	signal,
	currentOwner,
	applicationVersion,
	deadlineMs = 10000,
	createWorker = () =>
		new Worker(new URL('./codec-worker-runtime.js', import.meta.url), {
			type: 'module',
		}),
}) {
	if (
		typeof applicationVersion !== 'string' ||
		!/^[0-9]+\.[0-9]+\.[0-9]+(?:[-+][0-9A-Za-z.+-]+)?$/.test(
			applicationVersion,
		) ||
		applicationVersion.length > 64
	)
		throw refusal(
			'InputSchema',
			'Codec Worker needs the captured Rust application version',
		);
	if (
		!Number.isInteger(deadlineMs) ||
		deadlineMs < 1 ||
		deadlineMs > 120000 ||
		typeof currentOwner !== 'function'
	)
		throw refusal('InputSchema', 'Invalid codec Worker lifetime policy');
	if (
		!Number.isSafeInteger(sourceLength) ||
		sourceLength < 1 ||
		sourceLength > INPUT_BYTES_MAX
	)
		throw refusal('ResourceLimit', 'Codec source allowance');
	if (signal?.aborted)
		throw refusal('OperationCancelled', 'Codec Worker cancelled');
	const started = performance.now();
	const capturedOwner = Object.freeze({ ...owner });
	const expectedOwner = ownerBytes(capturedOwner);
	if (nonce !== undefined) nonceBytes(nonce);
	const lease = reserve(
		LINEAR_MEMORY_BYTES +
			sourceLength * 3 +
			REQUEST_HEADER +
			OUTPUT_BYTES_MAX * 2 +
			65 * 1024 * 1024 +
			65536,
	);
	const release = lease.release;
	let worker;
	let attemptedWorker = false;
	let captured;
	let responseBytes;
	let completed = false;
	let unknownAsync = false;
	const cancelled = () =>
		signal?.reason?.code === 'ConflictStaleBase'
			? refusal('ConflictStaleBase', 'Codec Worker owner changed')
			: refusal('OperationCancelled', 'Codec Worker cancelled');
	try {
		function check() {
			if (signal?.aborted) throw cancelled();
			if (!matchesOwner(currentOwner(), expectedOwner))
				throw refusal(
					'ConflictStaleBase',
					'Codec Worker owner changed',
				);
			if (performance.now() - started >= deadlineMs)
				throw refusal(
					'ResourceLimit',
					'Codec Worker deadline exceeded',
				);
		}
		// File reads and WebCrypto digests have no join/cancel API. Return logical
		// cancellation promptly, retaining the whole declaration if still pending.
		async function awaitBounded(operation) {
			let pending = true;
			const settled = Promise.resolve(operation).then(
				(value) => {
					pending = false;
					return value;
				},
				(error) => {
					pending = false;
					throw error;
				},
			);
			let interval, timer, abort;
			const stopped = new Promise((_, reject) => {
				abort = () => {
					try {
						check();
					} catch (error) {
						reject(error);
					}
				};
				interval = setInterval(abort, 10);
				timer = setTimeout(
					abort,
					Math.max(0, deadlineMs - (performance.now() - started)),
				);
				signal?.addEventListener('abort', abort, { once: true });
				abort();
			});
			try {
				return await Promise.race([settled, stopped]);
			} catch (error) {
				if (pending) unknownAsync = true;
				throw error;
			} finally {
				clearInterval(interval);
				clearTimeout(timer);
				signal?.removeEventListener('abort', abort);
			}
		}
		check();
		if (readSource) source = await awaitBounded(readSource());
		if (!(source instanceof Uint8Array) || source.length !== sourceLength)
			throw refusal('ResourceLimit', 'Codec source length changed');
		captured = await awaitBounded(
			capture(
				source,
				capturedOwner,
				codec,
				pixels,
				globalThis.crypto,
				nonce,
			),
		);
		check();
		attemptedWorker = true;
		worker = createWorker();
		const frame = await new Promise((resolve, reject) => {
			let timer;
			function cleanup() {
				clearInterval(timer);
				signal?.removeEventListener('abort', cancel);
			}
			function fail(error) {
				cleanup();
				reject(error);
			}
			function cancel() {
				fail(cancelled());
			}
			timer = setInterval(() => {
				try {
					check();
				} catch (error) {
					fail(error);
				}
			}, 10);
			signal?.addEventListener('abort', cancel, { once: true });
			worker.onerror = () =>
				fail(refusal('CodecFailed', 'Isolated codec Worker failed'));
			worker.onmessageerror = worker.onerror;
			worker.onmessage = ({ data }) => {
				try {
					check();
					if (
						data?.kind !== 'result' ||
						!(data.bytes instanceof ArrayBuffer)
					)
						throw refusal(
							'CodecFailed',
							'Isolated codec Worker returned no valid result',
						);
					responseBytes = new Uint8Array(data.bytes);
					const frame = decodeResponse(
						responseBytes,
						captured,
						currentOwner(),
						applicationVersion,
					);
					completed = true;
					cleanup();
					resolve(frame);
				} catch (error) {
					fail(error);
				}
			};
			const request = encodeRequest(captured);
			try {
				worker.postMessage({ kind: 'decode', bytes: request.buffer }, [
					request.buffer,
				]);
			} catch (error) {
				fail(error);
			}
		});
		// Keep admission charged until the caller consumes/discards this private
		// result. A retained document needs a separate resident lease first.
		return Object.freeze({
			...frame,
			release,
			responseBytes,
			requestHeader: captured.header.slice(),
			sourceSha256: captured.header.slice(80, 112),
			quarantine() {
				lease.quarantine();
			},
		});
	} catch (error) {
		if (
			unknownAsync ||
			(attemptedWorker &&
				!completed &&
				!isCompletedRefusal(error, captured))
		) {
			// terminate() is not a join. Retain this module's charge forever,
			// refusing later jobs until realm teardown rather than inventing
			// a timer-based reclamation acknowledgement.
			quarantined = error?.code ?? 'CodecFailed';
		}
		worker?.terminate();
		worker = null;
		if (!quarantined) release();
		throw error;
	} finally {
		worker?.terminate();
	}
}

export function decodeInWorker(options) {
	if (!(options.source instanceof Uint8Array))
		return Promise.reject(
			refusal('InputSchema', 'Codec source must be bytes'),
		);
	return decodeAdmitted({ ...options, sourceLength: options.source.length });
}
export function decodeFileInWorker({ file, ...options }) {
	if (!(file instanceof File))
		return Promise.reject(
			refusal('InputSchema', 'Codec source must be a File'),
		);
	return decodeAdmitted({
		...options,
		sourceLength: file.size,
		readSource: async () => new Uint8Array(await file.arrayBuffer()),
	});
}
