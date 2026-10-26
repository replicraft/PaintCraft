// Internal primitive only: public HEIF/JXL reader routes remain disabled.
import {
	capture,
	encodeRequest,
	decodeResponse,
	ownerBytes,
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
	return () => {
		if (!released && !quarantined) {
			released = true;
			reserved = 0;
		}
	};
}
export async function decodeInWorker({
	source,
	owner,
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
		!(source instanceof Uint8Array) ||
		!source.length ||
		source.length > INPUT_BYTES_MAX
	)
		throw refusal('ResourceLimit', 'Codec source allowance');
	if (signal?.aborted)
		throw refusal('OperationCancelled', 'Codec Worker cancelled');
	const started = performance.now();
	const expectedOwner = ownerBytes(owner);
	const release = reserve(
		LINEAR_MEMORY_BYTES +
			source.length * 2 +
			REQUEST_HEADER +
			OUTPUT_BYTES_MAX +
			65536,
	);
	let worker;
	let completed = false;
	try {
		const captured = await capture(source, owner, codec, pixels);
		function check() {
			if (signal?.aborted)
				throw refusal('OperationCancelled', 'Codec Worker cancelled');
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
		check();
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
				fail(refusal('OperationCancelled', 'Codec Worker cancelled'));
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
					const frame = decodeResponse(
						new Uint8Array(data.bytes),
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
		return Object.freeze({ ...frame, release });
	} catch (error) {
		if (worker && !completed && !error?.codecCompleted) {
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
