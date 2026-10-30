// Finite protocol4 oracle for independent 2x1, opaque, metadata-free point fixtures.
export const pointExportFiles = [
	'tag3-100.pcnd',
	'tag4-128.pcnd',
	'tag5-6.pcnd',
	'tag6-None.pcnd',
	'tag3-masked.pcnd',
	'tag4-masked.pcnd',
	'tag5-masked.pcnd',
	'tag6-masked.pcnd',
];
export function pointExport4Request(source, row, oracle, kind, bake) {
	if (
		!pointExportFiles.includes(row.file) ||
		!row.accepted ||
		row.attachment ||
		!['dds', 'msp'].includes(kind) ||
		typeof bake !== 'boolean'
	)
		throw Error('Closed point export specimen');
	const header = new Uint8Array(256),
		view = new DataView(header.buffer),
		text = new TextEncoder();
	const hex = (s) => Uint8Array.from(s.match(/../g), (b) => parseInt(b, 16));
	const uuid = (s) => hex(s.replaceAll('-', ''));
	header.set(text.encode(kind === 'dds' ? 'PCDDSQ04' : 'PCMSPQ04'));
	view.setUint16(8, 4);
	view.setUint32(12, source.length);
	view.setUint32(16, 2);
	view.setUint32(20, 1);
	header.set(uuid('44444444-4444-4444-8444-444444444444'), 24);
	header.set(uuid(oracle.state), 40);
	view.setBigUint64(56, 9n);
	header[64] = 3;
	header.set(uuid('55555555-5555-4555-8555-555555555555'), 65);
	header.set(hex(row.sha256), 81);
	const flags = 4 | (row.mask_count ? 8 : 0);
	if (kind === 'dds') {
		header.set([0, 0, 0, 1, 0, 0, 0, 0], 113);
		header[121] = flags;
	} else {
		header[113] = 2;
		header[114] = flags;
	}
	header[122] = Number(bake);
	header.set(hex(row.filter_inventory_sha256), 128);
	header.set(uuid(oracle.document), 160);
	header[209] = kind === 'dds' ? 1 : 2;
	header[210] = 1;
	for (const [at, value] of [
		[224, row.mask_count],
		[232, row.bands],
		[240, row.runs],
		[248, row.bands ? 2097152 : 0],
	])
		view.setBigUint64(at, BigInt(value), true);
	const request = new Uint8Array(256 + source.length);
	request.set(header);
	request.set(source, 256);
	return request;
}
// Passed directly to page.evaluate: no product imports, globals or helper closure.
export async function runPointExport4({ request, row, runtime, kind, bake }) {
	const equal = (a, b, label) => {
		if (JSON.stringify(a) !== JSON.stringify(b))
			throw Error(
				`${label}: ${JSON.stringify(a)} != ${JSON.stringify(b)}`,
			);
	};
	const sha = async (bytes) =>
		[...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))]
			.map((b) => b.toString(16).padStart(2, '0'))
			.join('');
	const input = Uint8Array.from(request),
		header = input.slice(0, 256),
		worker = new Worker(runtime, { type: 'module' });
	let timer;
	try {
		const response = await new Promise((resolve, reject) => {
			timer = setTimeout(() => reject(Error('Export4 deadline')), 30000);
			worker.onmessage = ({ data }) => resolve(data);
			worker.onerror = (event) => reject(Error(event.message));
			worker.postMessage(
				{ kind: `${kind}-export`, bytes: input.buffer },
				[input.buffer],
			);
		});
		equal(response.kind, 'result', 'completed bound export');
		const bytes = new Uint8Array(response.bytes),
			v = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
		const slice = (b, a, z) => [...b.subarray(a, z)];
		equal(
			new TextDecoder().decode(bytes.subarray(0, 8)),
			kind === 'dds' ? 'PCDDSR04' : 'PCMSPR04',
			'response magic',
		);
		equal(v.getUint16(8), 4, 'version');
		for (const [a, z, start] of [
			[32, 73, 24],
			[73, 89, 65],
			[89, 121, 81],
			[288, 320, 128],
			[320, 384, 160],
			[384, 416, 224],
		])
			equal(
				slice(bytes, a, z),
				slice(header, start, start + z - a),
				`field binding ${a}`,
			);
		equal(
			slice(bytes, 121, 129),
			kind === 'dds' ? slice(header, 113, 121) : [2, 0, 0, 0, 0, 0, 0, 0],
			'selected settings',
		);
		const flags = 4 | (row.mask_count ? 8 : 0);
		equal(
			[
				v.getUint32(12),
				v.getUint32(16),
				v.getUint32(20),
				bytes[265],
				bytes[266],
			],
			[2, 1, row.bytes, flags, Number(bake)],
			'geometry/source length/flags/intent',
		);
		const exactMsp = row.rendered_rgba.every((c, i, a) =>
			i % 4 === 3
				? c === 255
				: c === a[i - (i % 4)] && (c === 0 || c === 255),
		);
		const wanted = bake && (kind === 'dds' || exactMsp) ? 0 : 1;
		equal(bytes[10], wanted, 'typed success or CapabilityUnsupported');
		const size = v.getUint32(24),
			width = v.getUint16(28),
			height = v.getUint16(30),
			detail = v.getUint16(263),
			previewBytes = width * height * 4;
		equal(
			bytes.length,
			416 + size + previewBytes + detail,
			'exact response extent',
		);
		const encoded = bytes.slice(416, 416 + size),
			preview = bytes.slice(416 + size, 416 + size + previewBytes);
		const asHex = (b) =>
			[...b].map((n) => n.toString(16).padStart(2, '0')).join('');
		equal(
			await sha(encoded),
			asHex(bytes.subarray(129, 161)),
			'encoded SHA',
		);
		equal(
			await sha(preview),
			asHex(bytes.subarray(161, 193)),
			'preview SHA',
		);
		if (wanted === 0) {
			equal([width, height], [2, 1], 'preview geometry');
			equal([...preview], row.rendered_rgba, 'independent full plane');
			equal(
				bytes[193],
				kind === 'dds' ? 64 : 8,
				'explicit filters-baked loss',
			);
		} else {
			equal(
				[size, previewBytes, bytes[193]],
				[0, 0, 0],
				'refused has no artifact',
			);
			if (detail === 0) throw Error('Refusal lacks typed diagnostic');
		}
		return {
			kind: `export4-${kind}`,
			file: row.file,
			bake,
			status: bytes[10],
			code: wanted ? 'capability.unsupported' : null,
			encoded: [...encoded],
			encodedSha256: await sha(encoded),
			preview: [...preview],
			expected: row.rendered_rgba,
			requestSha256: await sha(Uint8Array.from(request)),
			responseSha256: await sha(bytes),
		};
	} finally {
		clearTimeout(timer);
		worker.terminate();
	}
}
