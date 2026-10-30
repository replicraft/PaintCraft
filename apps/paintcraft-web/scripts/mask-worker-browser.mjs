// Actual built persistentRenderer2 + codecExport4, independent masked planes.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const app = fileURLToPath(new URL('../', import.meta.url));
const root = path.resolve(app, '../..'),
	dist = path.join(app, 'dist');
const fixtures = path.join(
	root,
	'crates/services/paintcraft-formats/tests/browser-mask-export-fixtures',
);
assert.ok(process.env.CODEC_EVIDENCE_DIR && process.env.PLAYWRIGHT_MODULE);
fs.mkdirSync(process.env.CODEC_EVIDENCE_DIR, { recursive: true });
const out = fs.mkdtempSync(
	path.join(process.env.CODEC_EVIDENCE_DIR, 'attempt-'),
);
const sha = (b) => createHash('sha256').update(b).digest('hex');
const oracle = JSON.parse(fs.readFileSync(path.join(fixtures, 'oracle.json')));
const assets = {};
function inventory(folder) {
	for (const e of fs.readdirSync(folder, { withFileTypes: true })) {
		const p = path.join(folder, e.name);
		if (e.isDirectory()) inventory(p);
		else if (e.isFile())
			assets[path.relative(dist, p)] = {
				bytes: fs.statSync(p).size,
				sha256: sha(fs.readFileSync(p)),
			};
	}
}
inventory(dist);
const runtime = (prefix) => {
	const rows = Object.keys(assets).filter(
		(p) => p.startsWith(`_astro/${prefix}-`) && p.endsWith('.js'),
	);
	assert.equal(rows.length, 1);
	return `/${rows[0]}`;
};
const renderRuntime = runtime('filter-preview-runtime'),
	codecRuntime = runtime('codec-worker-runtime');
for (const row of oracle.fixtures)
	assert.equal(
		sha(fs.readFileSync(path.join(fixtures, row.file))),
		row.sha256,
	);
const evidence = {
	status: 'started',
	startedAt: new Date().toISOString(),
	assets,
	oracle,
	cases: [],
	scope: 'Actual built Worker2/Export4 with independent PCND4/appearance and Pillow decode; UI consent/revocation, latency, RSS and hardware remain separate.',
};
fs.copyFileSync(
	fileURLToPath(import.meta.url),
	path.join(out, 'executed-driver.mjs'),
);
fs.copyFileSync(
	path.join(fixtures, 'oracle.json'),
	path.join(out, 'oracle.json'),
);
let server, browser;
try {
	server = http.createServer((request, response) => {
		const pathname = new URL(request.url, 'http://localhost').pathname;
		if (pathname === '/__probe') {
			response.setHeader('Content-Type', 'text/html');
			response.end(
				'<!doctype html><title>Mask Worker qualification</title>',
			);
			return;
		}
		if (pathname.startsWith('/__fixture/')) {
			const name = pathname.slice('/__fixture/'.length);
			if (!oracle.fixtures.some((r) => r.file === name)) {
				response.writeHead(404).end();
				return;
			}
			response.end(fs.readFileSync(path.join(fixtures, name)));
			return;
		}
		const file = path.resolve(dist, `.${pathname}`);
		if (
			!file.startsWith(`${dist}/`) ||
			!fs.existsSync(file) ||
			!fs.statSync(file).isFile()
		) {
			response.writeHead(404).end();
			return;
		}
		response.setHeader(
			'Content-Type',
			{ '.js': 'text/javascript', '.wasm': 'application/wasm' }[
				path.extname(file)
			] ?? 'application/octet-stream',
		);
		fs.createReadStream(file).pipe(response);
	});
	await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
	const { chromium } = await import(
		pathToFileURL(process.env.PLAYWRIGHT_MODULE).href
	);
	browser = await chromium.launch({
		channel: 'chrome',
		headless: true,
		args: ['--enable-unsafe-swiftshader'],
	});
	evidence.browser = browser.version();
	const page = await browser.newPage();
	await page.goto(`http://127.0.0.1:${server.address().port}/__probe`);
	for (const row of oracle.fixtures) {
		const results = await page.evaluate(
			async ({ row, oracle, renderRuntime, codecRuntime }) => {
				const equal = (a, b, label) => {
					if (
						JSON.stringify(canonical(a)) !==
						JSON.stringify(canonical(b))
					)
						throw Error(
							`${label}: ${JSON.stringify(a)} != ${JSON.stringify(b)}`,
						);
				};
				const canonical = (v) =>
					Array.isArray(v)
						? v.map(canonical)
						: v && typeof v === 'object'
							? Object.fromEntries(
									Object.keys(v)
										.sort()
										.map((k) => [k, canonical(v[k])]),
								)
							: v;
				const sha = async (b) =>
					Array.from(
						new Uint8Array(
							await crypto.subtle.digest('SHA-256', b),
						),
						(v) => v.toString(16).padStart(2, '0'),
					).join('');
				const hex = (s) =>
					Uint8Array.from(s.match(/../g), (b) => parseInt(b, 16));
				const uuid = (s) => hex(s.replaceAll('-', ''));
				const source = new Uint8Array(
					await (await fetch(`/__fixture/${row.file}`)).arrayBuffer(),
				);
				equal(await sha(source), row.sha256, 'independent source SHA');
				const owner = {
					session: '44444444-4444-4444-8444-444444444444',
					state: oracle.state,
					epoch: '9',
					frontend: 3,
				};
				const view = { region: [0, 0, 2, 1], level: 0 },
					draft = '55555555-5555-4555-8555-555555555555';
				function persistent() {
					const worker = new Worker(renderRuntime, {
						type: 'module',
					});
					let id = 0;
					const waiting = new Map();
					worker.onmessage = ({ data }) => {
						const p = waiting.get(data?.id);
						if (p) {
							waiting.delete(data.id);
							clearTimeout(p.timer);
							p.resolve(data);
						}
					};
					worker.onerror = (event) => {
						for (const p of waiting.values()) {
							clearTimeout(p.timer);
							p.reject(Error(event.message));
						}
						waiting.clear();
					};
					return {
						worker,
						send(kind, body) {
							return new Promise((resolve, reject) => {
								const next = ++id;
								const timer = setTimeout(
									() =>
										reject(
											Error(`Render ${kind} deadline`),
										),
									30000,
								);
								waiting.set(next, { resolve, reject, timer });
								worker.postMessage({ id: next, kind, ...body });
							});
						},
					};
				}
				const facts = {
					maskCount: 1,
					maskBands: row.bands,
					maskRuns: row.runs,
					maskDecodeBytes: row.bands ? 2 * 1024 * 1024 : 0,
				};
				const plan = {
					owner,
					draft,
					document: oracle.document,
					sourceSha256: row.sha256,
					size: [2, 1],
					layers: 1,
					residentBytes: 262144,
					metadataBytes: row.metadata ? oracle.profile_bytes : 0,
					metadataCharge: row.metadata ? 1048576 : 0,
					filterDigest: row.filter_inventory_sha256,
					...facts,
				};
				const results = [],
					service = persistent();
				try {
					const ready = await service.send('open', {
						plan,
						source: source.slice().buffer,
					});
					equal(ready.kind, 'ready', 'PCND4 ready');
					equal(
						canonical(ready.binding),
						canonical(plan),
						'ready exact facts',
					);
					const request = (
						generation,
						operation = null,
						render = true,
					) => ({
						owner,
						draft,
						sourceSha256: row.sha256,
						generation: String(generation),
						operation,
						target: operation ? oracle.layer : null,
						view,
						render,
					});
					const frame = async (req, expected) => {
						const result = await service.send('update', {
							request: req,
						});
						equal(
							result.kind,
							req.render ? 'frame' : 'prepared',
							'frame kind',
						);
						for (const key of [
							'owner',
							'draft',
							'sourceSha256',
							'generation',
							'target',
							'view',
						])
							equal(result[key], req[key], `frame ${key}`);
						equal(
							result.operationSha256,
							await sha(
								new TextEncoder().encode(
									JSON.stringify(canonical(req.operation)),
								),
							),
							'operation digest',
						);
						equal(
							[
								result.width,
								result.height,
								result.region,
								result.level,
							],
							[2, 1, [0, 0, 2, 1], 0],
							'geometry',
						);
						if (req.render)
							equal(
								Array.from(result.pixels),
								expected,
								'independent masked pixels',
							);
						else
							equal(
								result.pixels,
								undefined,
								'PREPARED has no raster',
							);
					};
					await frame(request(1), row.expected_rgba);
					const operation = {
						op: 'filter.update',
						layer_ref: { id: oracle.layer },
						filter_ref: {
							id: '00000000-0000-0000-0000-000000000000',
						},
						descriptor: {
							kind: 'brightness-contrast-v1',
							brightness: 100,
							contrast: 0,
						},
					};
					const after = row.file.startsWith('fractional')
						? [128, 128, 128, 255, 255, 255, 255, 255]
						: row.file.startsWith('zero') ||
								row.file.startsWith('filter-disabled')
							? oracle.raw_rgba
							: [255, 255, 255, 255, 255, 255, 255, 255];
					await frame(request(2, operation), after);
					await frame(request(3, operation, false));
					const remove = { ...operation, op: 'filter.mask.remove' };
					delete remove.descriptor;
					const refused = await service.send('update', {
						request: request(4, remove),
					});
					equal(
						[refused.kind, refused.code, refused.completed],
						['error', 'input.schema', true],
						'mask control remains outside4KiB descriptor grammar',
					);
					await frame(request(5), row.expected_rgba);
					const retired = await service.send('cancel', {
						generation: '6',
					});
					equal(
						[retired.kind, retired.generation],
						['retired', '6'],
						'exact retirement',
					);
					await frame(request(7), row.expected_rgba);
					equal(
						(await service.send('close', {})).kind,
						'closed',
						'source closed ack',
					);
					results.push({
						kind: 'persistent2',
						file: row.file,
						pixels: row.expected_rgba,
						descriptorPixels: after,
						prepared: true,
						refusedMaskOp: refused.code,
						sourceOpens: 1,
					});
				} finally {
					service.worker.terminate();
				}
				async function encoded(kind, bytes) {
					const worker = new Worker(codecRuntime, { type: 'module' });
					try {
						return await new Promise((resolve, reject) => {
							const timer = setTimeout(
								() => reject(Error('Export deadline')),
								30000,
							);
							worker.onmessage = ({ data }) => {
								clearTimeout(timer);
								resolve(data);
							};
							worker.onerror = (e) => {
								clearTimeout(timer);
								reject(Error(e.message));
							};
							worker.postMessage(
								{ kind: `${kind}-export`, bytes: bytes.buffer },
								[bytes.buffer],
							);
						});
					} finally {
						worker.terminate();
					}
				}
				for (const kind of ['dds', 'msp'])
					for (const fault of [
						'none',
						'refuse',
						...(row.bands ? ['underquote'] : []),
						...(row.metadata ? ['metadata'] : []),
					]) {
						const h = new Uint8Array(256),
							v = new DataView(h.buffer),
							text = new TextEncoder();
						h.set(
							text.encode(
								kind === 'dds' ? 'PCDDSQ04' : 'PCMSPQ04',
							),
						);
						v.setUint16(8, 4);
						v.setUint32(12, source.length);
						v.setUint32(16, 2);
						v.setUint32(20, 1);
						h.set(uuid(owner.session), 24);
						h.set(uuid(owner.state), 40);
						v.setBigUint64(56, 9n);
						h[64] = 3;
						h.set(uuid(draft), 65);
						h.set(hex(row.sha256), 81);
						if (kind === 'dds') {
							h.set([0, 0, 0, 1, 0, 0, 0, 0], 113);
							h[121] = 12;
						} else {
							h[113] = 2;
							h[114] = 12;
						}
						h[122] = fault === 'refuse' ? 0 : 1;
						h.set(hex(row.filter_inventory_sha256), 128);
						h.set(uuid(oracle.document), 160);
						h[209] = kind === 'dds' ? 1 : 2;
						h[210] = 1;
						if (row.metadata) {
							h.set(hex(oracle.attachment_sha256), 176);
							h[208] = 3;
							v.setUint32(212, oracle.metadata_wire_bytes);
							v.setBigUint64(216, 1048576n);
						}
						for (const [at, value] of [
							[224, 1],
							[232, row.bands],
							[240, row.runs],
							[
								248,
								fault === 'underquote'
									? 1
									: facts.maskDecodeBytes,
							],
						])
							v.setBigUint64(at, BigInt(value), true);
						if (fault === 'metadata') h[176] ^= 1;
						const request = new Uint8Array(256 + source.length);
						request.set(h);
						request.set(source, 256);
						const response = await encoded(kind, request);
						equal(
							response.kind,
							'result',
							'completed bound export response',
						);
						const b = new Uint8Array(response.bytes),
							r = new DataView(b.buffer),
							slice = (x, a, z) => Array.from(x.subarray(a, z));
						equal(
							new TextDecoder().decode(b.subarray(0, 8)),
							kind === 'dds' ? 'PCDDSR04' : 'PCMSPR04',
							'response magic',
						);
						equal(r.getUint16(8), 4, 'response version');
						for (const [a, z, start] of [
							[32, 73, 24],
							[73, 89, 65],
							[89, 121, 81],
							[288, 320, 128],
							[320, 384, 160],
							[384, 416, 224],
						])
							equal(
								slice(b, a, z),
								slice(h, start, start + z - a),
								`response echo${a}`,
							);
						equal(
							[
								r.getUint32(12),
								r.getUint32(16),
								r.getUint32(20),
								b[265],
								b[266],
							],
							[2, 1, source.length, 12, h[122]],
							'source geometry/intent',
						);
						const wanted =
							fault === 'refuse'
								? 1
								: fault === 'underquote'
									? 3
									: fault === 'metadata'
										? 2
										: kind === 'msp' && !row.msp_supported
											? 1
											: 0;
						equal(b[10], wanted, 'exact refusal or success');
						const n = r.getUint32(24),
							previewSize = r.getUint16(28) * r.getUint16(30) * 4,
							detail = r.getUint16(263);
						equal(
							b.length,
							416 + n + previewSize + detail,
							'exact response extent',
						);
						const output = b.subarray(416, 416 + n),
							preview = b.subarray(
								416 + n,
								416 + n + previewSize,
							);
						equal(
							await sha(output),
							Array.from(b.subarray(129, 161), (v) =>
								v.toString(16).padStart(2, '0'),
							).join(''),
							'encoded SHA',
						);
						equal(
							await sha(preview),
							Array.from(b.subarray(161, 193), (v) =>
								v.toString(16).padStart(2, '0'),
							).join(''),
							'preview SHA',
						);
						if (wanted === 0) {
							equal(
								Array.from(preview),
								row.expected_rgba,
								'literal export pixels',
							);
							equal(
								b[193],
								(kind === 'dds' ? 64 : 8) |
									(row.metadata
										? kind === 'dds'
											? 32
											: 4
										: 0),
								'independent bake/privacy losses',
							);
						} else
							equal(
								[n, previewSize, b[193]],
								[0, 0, 0],
								'refusal has no artifact',
							);
						results.push({
							kind: `export4-${kind}`,
							file: row.file,
							fault,
							status: b[10],
							pixels: Array.from(preview),
							encoded: Array.from(output),
							encodedSha256: await sha(output),
							expected: row.expected_rgba,
						});
					}
				return results;
			},
			{ row, oracle, renderRuntime, codecRuntime },
		);
		for (const result of results) {
			if (result.encoded?.length) {
				const extension = result.kind.endsWith('dds') ? 'dds' : 'msp';
				const file = `${row.file}-${result.fault}.${extension}`;
				fs.writeFileSync(
					path.join(out, file),
					Buffer.from(result.encoded),
				);
				result.artifact = file;
				delete result.encoded;
			}
			evidence.cases.push(result);
		}
		fs.writeFileSync(
			path.join(out, 'production.json'),
			JSON.stringify(evidence, null, 4),
		);
	}
	const outputs = evidence.cases.filter((r) => r.artifact);
	const python =
		process.env.PILLOW_PYTHON ??
		'/Library/Frameworks/Python.framework/Versions/3.12/bin/python3.12';
	const probe = spawnSync(
		python,
		[
			'-B',
			'-c',
			'import json,sys,PIL;from PIL import Image;print(json.dumps({"version":PIL.__version__,"images":[{"file":p,"size":list(Image.open(p).size),"rgba":list(Image.open(p).convert("RGBA").tobytes())} for p in sys.argv[1:]]}))',
			...outputs.map((r) => path.join(out, r.artifact)),
		],
		{ encoding: 'utf8' },
	);
	assert.equal(probe.status, 0, probe.stderr);
	const decoded = JSON.parse(probe.stdout);
	evidence.independentPillow = decoded.version;
	assert.equal(decoded.images.length, outputs.length);
	for (let i = 0; i < outputs.length; i++) {
		assert.deepEqual(decoded.images[i].size, [2, 1]);
		assert.deepEqual(decoded.images[i].rgba, outputs[i].expected);
	}
	for (const [name, item] of Object.entries(assets))
		assert.equal(sha(fs.readFileSync(path.join(dist, name))), item.sha256);
	for (const row of oracle.fixtures)
		assert.equal(
			sha(fs.readFileSync(path.join(fixtures, row.file))),
			row.sha256,
		);
	evidence.status = 'passed';
} catch (error) {
	evidence.status = 'failed';
	evidence.error = String(error.stack ?? error);
	process.exitCode = 1;
} finally {
	await browser?.close();
	if (server) await new Promise((resolve) => server.close(resolve));
	evidence.finishedAt = new Date().toISOString();
	fs.writeFileSync(
		path.join(out, 'production.json'),
		`${JSON.stringify(evidence, null, 4)}\n`,
	);
	console.log(
		JSON.stringify({
			status: evidence.status,
			out,
			cases: evidence.cases.length,
		}),
	);
}
