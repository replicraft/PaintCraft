// One isolated job per Worker. Never imported by the editor's main Wasm module.
import initialize, {
	application_version,
	dds_export_protocol_version,
	dds_export_request,
	decode_request,
	linear_memory_ceiling,
	msp_export_protocol_version,
	msp_export_request,
	protocol_version,
	tiff_page_protocol_version,
	tiff_page_request,
} from '../../codec-worker-pkg/paintcraft_web_codec_worker.js';
import {
	INPUT_BYTES_MAX,
	LINEAR_MEMORY_BYTES,
	OUTPUT_BYTES_MAX,
	REQUEST_HEADER,
} from './codec-worker-protocol.js';
import {
	DDS_OUTPUT_MAX,
	DDS_REQUEST_HEADER,
	DDS_SOURCE_MAX,
} from './dds-worker-protocol.js';
import {
	MSP_OUTPUT_MAX,
	MSP_REQUEST_HEADER,
	MSP_SOURCE_MAX,
} from './msp-worker-protocol.js';
import {
	TIFF_OUTPUT_MAX,
	TIFF_REQUEST_HEADER,
	TIFF_SOURCE_MAX,
} from './tiff-page-worker-protocol.js';

const operations = Object.freeze({
	'tiff-pages': Object.freeze({
		header: TIFF_REQUEST_HEADER,
		input: TIFF_SOURCE_MAX,
		output: TIFF_OUTPUT_MAX,
		version: tiff_page_protocol_version,
		expectedVersion: 1,
		execute: tiff_page_request,
	}),
	decode: Object.freeze({
		header: REQUEST_HEADER,
		input: INPUT_BYTES_MAX,
		output: OUTPUT_BYTES_MAX,
		version: protocol_version,
		expectedVersion: 2,
		execute: decode_request,
	}),
	'dds-export': Object.freeze({
		header: DDS_REQUEST_HEADER,
		input: DDS_SOURCE_MAX,
		output: DDS_OUTPUT_MAX,
		version: dds_export_protocol_version,
		expectedVersion: 1,
		execute: dds_export_request,
	}),
	'msp-export': Object.freeze({
		header: MSP_REQUEST_HEADER,
		input: MSP_SOURCE_MAX,
		output: MSP_OUTPUT_MAX,
		version: msp_export_protocol_version,
		expectedVersion: 1,
		execute: msp_export_request,
	}),
});
let used = false;
self.onmessage = async (event) => {
	if (used) return;
	used = true;
	try {
		const { data } = event;
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
		const operation = Object.hasOwn(operations, data?.kind)
			? operations[data.kind]
			: undefined;
		if (
			!operation ||
			!(data.bytes instanceof ArrayBuffer) ||
			data.bytes.byteLength < operation.header ||
			data.bytes.byteLength > operation.input + operation.header
		)
			throw new Error('Codec Worker input allowance');
		if (operation.version() !== operation.expectedVersion)
			throw new Error('Codec Worker contract mismatch');
		const output = operation.execute(new Uint8Array(data.bytes));
		if (output.byteLength > operation.output)
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
