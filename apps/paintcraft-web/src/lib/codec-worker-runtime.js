// One isolated job per Worker. Never imported by the editor's main Wasm module.
import initialize, {
	application_version,
	protocol_version,
	linear_memory_ceiling,
	decode_request,
} from '../../codec-worker-pkg/paintcraft_web_codec_worker.js';
import {
	INPUT_BYTES_MAX,
	REQUEST_HEADER,
	LINEAR_MEMORY_BYTES,
	OUTPUT_BYTES_MAX,
} from './codec-worker-protocol.js';
let used = false;
self.onmessage = async ({ data }) => {
	if (used) return;
	used = true;
	try {
		const wasm = await initialize();
		if (
			protocol_version() !== 2 ||
			linear_memory_ceiling() !== LINEAR_MEMORY_BYTES
		)
			throw new Error('Codec Worker contract mismatch');
		if (data?.kind === 'probe-memory') {
			const initialBytes = wasm.memory.buffer.byteLength;
			let growthRefused = false;
			try {
				wasm.memory.grow(LINEAR_MEMORY_BYTES / 65536);
			} catch (error) {
				if (!(error instanceof RangeError)) throw error;
				growthRefused = true;
			}
			self.postMessage({
				kind: 'memory',
				initialBytes,
				growthRefused,
				version: application_version(),
			});
			return;
		}
		if (
			data?.kind !== 'decode' ||
			!(data.bytes instanceof ArrayBuffer) ||
			data.bytes.byteLength < REQUEST_HEADER ||
			data.bytes.byteLength > INPUT_BYTES_MAX + REQUEST_HEADER
		)
			throw new Error('Codec Worker input allowance');
		const output = decode_request(new Uint8Array(data.bytes));
		if (output.byteLength > OUTPUT_BYTES_MAX)
			throw new Error('Codec Worker output allowance');
		self.postMessage({ kind: 'result', bytes: output.buffer }, [
			output.buffer,
		]);
	} catch {
		// A Wasm allocator/panic trap destroys only this isolated instance. There
		// is no trustworthy output to publish or native heap exit status here.
		self.postMessage({
			kind: 'failure',
			code: 'CodecFailed',
			message: 'Isolated codec Worker failed',
		});
	}
};
