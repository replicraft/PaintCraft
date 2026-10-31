import * as ddsSpatial from './dds-spatial-worker-protocol.js';
import * as mspSpatial from './msp-spatial-worker-protocol.js';
import * as ddsCatalog from './dds-catalog-worker-protocol.js';
import * as mspCatalog from './msp-catalog-worker-protocol.js';
// Isolated browser service; UI reader admission is qualified separately.
import {
	capture,
	decodeIntegerResponse,
	decodeResponse,
	encodeRequest,
	INPUT_BYTES_MAX,
	isCompletedRefusal,
	LINEAR_MEMORY_BYTES,
	matchesOwner,
	nonceBytes,
	OUTPUT_BYTES_MAX,
	ownerBytes,
	REQUEST_HEADER,
	refusal,
} from './codec-worker-protocol.js';
import {
	captureDds,
	DDS_RESERVATION,
	DDS_SOURCE_MAX,
	decodeDdsResponse,
	encodeDdsRequest,
	freezeDdsPlan,
	isCompletedDdsRefusal,
} from './dds-worker-protocol.js';
import {
	captureMsp,
	decodeMspResponse,
	encodeMspRequest,
	freezeMspPlan,
	isCompletedMspRefusal,
	MSP_RESERVATION,
	MSP_SOURCE_MAX,
} from './msp-worker-protocol.js';
import {
	captureTiff,
	decodeTiffResponse,
	encodeTiffRequest,
	freezeTiffPlan,
	isCompletedTiffRefusal,
	TIFF_RESERVATION,
	TIFF_SOURCE_MAX,
} from './tiff-page-worker-protocol.js';

const DECODE = Object.freeze({
	kind: 'decode',
	inputMax: INPUT_BYTES_MAX,
	capture: (source, owner, codec, pixels, nonce) =>
		capture(source, owner, codec, pixels, globalThis.crypto, nonce),
	encode: encodeRequest,
	decode: decodeResponse,
	completed: isCompletedRefusal,
	digest: (captured) => captured.header.slice(80, 112),
	quote: (length) =>
		LINEAR_MEMORY_BYTES +
		length * 3 +
		REQUEST_HEADER +
		OUTPUT_BYTES_MAX * 2 +
		65 * 1024 * 1024 +
		65536,
});
const INTEGER_DECODE = Object.freeze({
	...DECODE,
	decode: decodeIntegerResponse,
	// The isolated u16 RGBA source plane may coexist with its RGBA8 output.
	quote: (length) => DECODE.quote(length) + 32 * 1024 * 1024,
});
const DDS = Object.freeze({
	kind: 'dds-export',
	inputMax: DDS_SOURCE_MAX,
	capture: (source, owner, _codec, _pixels, nonce, plan) =>
		captureDds(source, owner, plan, nonce),
	encode: encodeDdsRequest,
	decode: decodeDdsResponse,
	completed: isCompletedDdsRefusal,
	digest: (captured) => captured.header.slice(81, 113),
	quote: () => DDS_RESERVATION,
});
const MSP = Object.freeze({
	kind: 'msp-export',
	inputMax: MSP_SOURCE_MAX,
	capture: (source, owner, _codec, _pixels, nonce, plan) =>
		captureMsp(source, owner, plan, nonce),
	encode: encodeMspRequest,
	decode: decodeMspResponse,
	completed: isCompletedMspRefusal,
	digest: (captured) => captured.header.slice(81, 113),
	quote: () => MSP_RESERVATION,
});
const DDS_SPATIAL = Object.freeze({
	...DDS,
	kind: 'dds-spatial-export',
	capture: (source, owner, _codec, _pixels, nonce, plan) =>
		ddsSpatial.captureDds(source, owner, plan, nonce),
	encode: ddsSpatial.encodeDdsRequest,
	decode: ddsSpatial.decodeDdsResponse,
	completed: ddsSpatial.isCompletedDdsRefusal,
	finalize: ddsSpatial.freezeDdsPlan,
});
const MSP_SPATIAL = Object.freeze({
	...MSP,
	kind: 'msp-spatial-export',
	capture: (source, owner, _codec, _pixels, nonce, plan) =>
		mspSpatial.captureMsp(source, owner, plan, nonce),
	encode: mspSpatial.encodeMspRequest,
	decode: mspSpatial.decodeMspResponse,
	completed: mspSpatial.isCompletedMspRefusal,
	finalize: mspSpatial.freezeMspPlan,
});
const DDS_CATALOG = Object.freeze({
	...DDS,
	kind: 'dds-catalog-export',
	capture: (source, owner, _codec, _pixels, nonce, plan) =>
		ddsCatalog.captureDds(source, owner, plan, nonce),
	encode: ddsCatalog.encodeDdsRequest,
	decode: ddsCatalog.decodeDdsResponse,
	completed: ddsCatalog.isCompletedDdsRefusal,
	finalize: ddsCatalog.freezeDdsPlan,
});
const MSP_CATALOG = Object.freeze({
	...MSP,
	kind: 'msp-catalog-export',
	capture: (source, owner, _codec, _pixels, nonce, plan) =>
		mspCatalog.captureMsp(source, owner, plan, nonce),
	encode: mspCatalog.encodeMspRequest,
	decode: mspCatalog.decodeMspResponse,
	completed: mspCatalog.isCompletedMspRefusal,
	finalize: mspCatalog.freezeMspPlan,
});
const TIFF = Object.freeze({
	kind: 'tiff-pages',
	inputMax: TIFF_SOURCE_MAX,
	capture: (source, owner, _codec, _pixels, nonce, plan) =>
		captureTiff(source, owner, plan, nonce),
	encode: encodeTiffRequest,
	decode: decodeTiffResponse,
	completed: isCompletedTiffRefusal,
	digest: (captured) => captured.header.slice(81, 113),
	quote: () => TIFF_RESERVATION,
});
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
async function runAdmitted(
	{
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
	},
	operation = DECODE,
	admittedLease,
	admittedAt,
	operationPlan,
) {
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
		sourceLength > operation.inputMax
	)
		throw refusal('ResourceLimit', 'Codec source allowance');
	if (signal?.aborted)
		throw refusal('OperationCancelled', 'Codec Worker cancelled');
	const started = admittedAt ?? performance.now();
	const capturedOwner = Object.freeze({ ...owner });
	const expectedOwner = ownerBytes(capturedOwner);
	if (nonce !== undefined) nonceBytes(nonce);
	const lease = admittedLease ?? reserve(operation.quote(sourceLength));
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
			operation.capture(
				source,
				capturedOwner,
				codec,
				pixels,
				nonce,
				operationPlan,
			),
		);
		check();
		attemptedWorker = true;
		worker = createWorker();
		const frame = await new Promise((resolve, reject) => {
			let timer;
			let received = false,
				settled = false;
			function cleanup() {
				clearInterval(timer);
				signal?.removeEventListener('abort', cancel);
			}
			function fail(error) {
				if (settled) return;
				settled = true;
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
			worker.onmessage = async (event) => {
				if (received || settled) return;
				received = true;
				try {
					const { data } = event;
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
					const decoded = operation.decode(
						responseBytes,
						captured,
						currentOwner(),
						applicationVersion,
						awaitBounded,
					);
					// Decoder2 is synchronous: preserve its original completion
					// boundary, without a new await/late-abort race.
					const frame =
						operation === DECODE ? decoded : await decoded;
					if (settled) return;
					if (operation !== DECODE) check();
					completed = true;
					settled = true;
					cleanup();
					resolve(frame);
				} catch (error) {
					fail(error);
				}
			};
			const request = operation.encode(captured);
			try {
				worker.postMessage(
					{ kind: operation.kind, bytes: request.buffer },
					[request.buffer],
				);
			} catch (error) {
				fail(error);
			}
		});
		// Keep admission charged until the caller consumes/discards this private
		// result. A retained document needs a separate resident lease first.
		return Object.freeze({
			...(operation === DECODE || operation === INTEGER_DECODE
				? frame
				: {}),
			release,
			// Export encoded-file validation remains mandatory in Rust before
			// marking the caller lease valid. Never publish from JS slices alone.
			requiresRustValidation: operation !== DECODE,
			responseBytes,
			requestHeader: captured.header.slice(),
			sourceSha256: operation.digest(captured),
			quarantine() {
				lease.quarantine();
			},
		});
	} catch (error) {
		if (
			unknownAsync ||
			(attemptedWorker &&
				!completed &&
				!operation.completed(error, captured))
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
	return runAdmitted(
		{ ...options, sourceLength: options.source.length },
		options.codec === 9 ? INTEGER_DECODE : DECODE,
	);
}
export function decodeFileInWorker({ file, ...options }) {
	if (!(file instanceof File))
		return Promise.reject(
			refusal('InputSchema', 'Codec source must be a File'),
		);
	return runAdmitted(
		{
			...options,
			sourceLength: file.size,
			readSource: async () => new Uint8Array(await file.arrayBuffer()),
		},
		options.codec === 9 ? INTEGER_DECODE : DECODE,
	);
}
export function tiffFileInWorker({ file, plan, ...options }) {
	if (!(file instanceof File))
		return Promise.reject(
			refusal('InputSchema', 'TIFF source must be a File'),
		);
	const frozen = freezeTiffPlan(plan);
	return runAdmitted(
		{
			...options,
			sourceLength: file.size,
			readSource: async () => new Uint8Array(await file.arrayBuffer()),
		},
		TIFF,
		undefined,
		undefined,
		frozen,
	);
}

// Admission precedes Rust snapshot serialization and every JS snapshot copy.
// An unused ticket retains its declaration until the trusted host drops its
// snapshots and calls release(). A timer/abort alone cannot prove that lifetime.
export function admitDdsInWorker(options) {
	return admitExportInWorker(
		options,
		options.plan?.protocolVersion === 6
			? DDS_CATALOG
			: options.plan?.protocolVersion === 5
				? DDS_SPATIAL
				: DDS,
		freezeDdsPlan,
	);
}
export function admitMspInWorker(options) {
	return admitExportInWorker(
		options,
		options.plan?.protocolVersion === 6
			? MSP_CATALOG
			: options.plan?.protocolVersion === 5
				? MSP_SPATIAL
				: MSP,
		freezeMspPlan,
	);
}
function admitExportInWorker(options, operation, freezePlan) {
	const protocol = options.plan?.protocolVersion ?? 4;
	if (![4, 5, 6].includes(protocol))
		throw refusal('InputSchema', 'Unsupported export protocol version');
	const plan = freezePlan(options.plan);
	ownerBytes(options.owner);
	if (options.nonce !== undefined) nonceBytes(options.nonce);
	const version = options.applicationVersion;
	const deadline = options.deadlineMs ?? 10000;
	if (
		typeof version !== 'string' ||
		!/^[0-9]+\.[0-9]+\.[0-9]+(?:[-+][0-9A-Za-z.+-]+)?$/.test(version) ||
		version.length > 64 ||
		typeof options.currentOwner !== 'function' ||
		!Number.isInteger(deadline) ||
		deadline < 1 ||
		deadline > 120000
	)
		throw refusal('InputSchema', 'Invalid export Worker lifetime/version');
	if (options.signal?.aborted)
		throw refusal('OperationCancelled', 'Codec Worker cancelled');
	if (!matchesOwner(options.currentOwner(), ownerBytes(options.owner)))
		throw refusal('ConflictStaleBase', 'Codec Worker owner changed');
	const captured = Object.freeze({
		...options,
		owner: Object.freeze({ ...options.owner }),
		nonce: options.nonce ?? crypto.randomUUID(),
	});
	const lease = reserve(operation.quote()),
		started = performance.now();
	let used = false;
	return Object.freeze({
		async run(source, finalizedPlan) {
			if (used)
				throw refusal(
					'InputSchema',
					'Export admission already consumed',
				);
			used = true;
			try {
				let finalPlan = plan;
				if (operation.finalize) {
					const exactPlan =
						finalizedPlan ??
						(typeof captured.finalizePlan === 'function'
							? captured.finalizePlan(source?.length)
							: undefined);
					finalPlan = operation.finalize(exactPlan);
					for (const key of [
						'width',
						'height',
						'sourceFlags',
						'bakeIntent',
						'version',
					])
						if (finalPlan[key] !== plan[key])
							throw refusal(
								'ConflictStaleBase',
								'Spatial export plan changed during snapshot',
							);
					for (const key of [
						'filterDigest',
						'metadataContext',
						'maskContext',
						'settings',
					]) {
						const before = plan[key],
							after = finalPlan[key];
						if (
							Boolean(before) !== Boolean(after) ||
							(before &&
								(before.length !== after.length ||
									before.some(
										(value, index) =>
											value !== after[index],
									)))
						)
							throw refusal(
								'ConflictStaleBase',
								'Spatial export authority changed during snapshot',
							);
					}
				}
				return await runAdmitted(
					{ ...captured, source, sourceLength: source?.length },
					operation,
					lease,
					started,
					finalPlan,
				);
			} catch (error) {
				lease.release();
				throw error;
			}
		},
		release() {
			if (!used) {
				used = true;
				lease.release();
			}
		},
	});
}
