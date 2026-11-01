import { validateCopySpec } from './filter-preview-copy.js';
import { sha } from './filter-preview-protocol.js';
import { encodeProgram } from './filter-preview-program-protocol.js';
import {
	equal7,
	fail7,
	shape7,
	validateCopyAdmitted7,
	verifyBinding7,
} from './filter-preview-appearance-protocol.js';
// Visual print/clipboard PNG only. Own-mask foreign bake remains closed.
export async function visualCopy7({
	channel,
	plan,
	request,
	copy,
	check,
	hold,
}) {
	const { output, selection } = copy;
	validateCopySpec(output, request, plan);
	if (
		!['print', 'clipboard'].includes(output.purpose) ||
		output.format !== 'png' ||
		output.frontend !== 'studio'
	)
		throw fail7('CapabilityUnsupported', 'Source7 visual PNG only');
	if (
		selection.length !== output.maskLength ||
		(await sha(selection)) !== output.maskSha256
	)
		throw fail7('ConflictStaleBase', 'Source7 selection digest');
	const outputSha256 = await sha(
		new TextEncoder().encode(encodeProgram(output, 4096)),
	);
	check();
	const admitted = await channel.send('copy-prepare', { request, output });
	check();
	validateCopyAdmitted7(admitted, request, plan, outputSha256);
	await verifyBinding7(admitted.binding, request, plan);
	check();
	const reply = await channel.send(
		'copy-input',
		{ outputSha256, mask: selection.buffer },
		[selection.buffer],
	);
	check();
	shape7(reply, [
		'kind',
		'outputVersion',
		'outputSha256',
		'bytesSha256',
		'mime',
		'binding',
		'bytes',
	]);
	if (
		reply.kind !== 'encoded-copy' ||
		reply.outputVersion !== 1 ||
		reply.outputSha256 !== outputSha256 ||
		reply.mime !== 'image/png' ||
		!equal7(reply.binding, admitted.binding) ||
		!(reply.bytes instanceof Uint8Array) ||
		!(reply.bytes.buffer instanceof ArrayBuffer) ||
		reply.bytes.byteOffset !== 0 ||
		reply.bytes.byteLength !== reply.bytes.buffer.byteLength ||
		!reply.bytes.length ||
		reply.bytes.length > 16777216 - output.maskLength ||
		typeof reply.bytesSha256 !== 'string' ||
		!/^[a-f0-9]{64}$/.test(reply.bytesSha256)
	)
		throw fail7('CodecFailed', 'Source7 encoded result');
	if ((await sha(reply.bytes)) !== reply.bytesSha256)
		throw fail7('CodecFailed', 'Source7 encoded SHA');
	// close/cancel must join this async digest and never publish a stale result.
	check();
	return hold(reply);
}
