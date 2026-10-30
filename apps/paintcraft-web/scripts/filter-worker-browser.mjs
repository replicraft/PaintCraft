// Actual built Worker. Editor consent and service quarantine have separate UI gates.
import assert from 'node:assert/strict';
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
	'crates/services/paintcraft-formats/tests/browser-filter-export-fixtures',
);
assert.ok(process.env.CODEC_EVIDENCE_DIR && process.env.PLAYWRIGHT_MODULE);
fs.mkdirSync(process.env.CODEC_EVIDENCE_DIR, { recursive: true });
const out = fs.mkdtempSync(
	path.join(process.env.CODEC_EVIDENCE_DIR, 'attempt-'),
);
const sha = (bytes) => createHash('sha256').update(bytes).digest('hex');
const oracle = JSON.parse(fs.readFileSync(path.join(fixtures, 'oracle.json')));
const version = JSON.parse(
	fs.readFileSync(path.join(app, 'package.json')),
).version;
const evidence = {
	status: 'started',
	sources: {},
	assets: {},
	cases: [],
	startedAt: new Date().toISOString(),
	scope: 'Actual built isolated Worker; UI consent/service quarantine separate. No RSS/forced-reclaim claim.',
};
function inventory(folder, target, prefix = '') {
	for (const entry of fs.readdirSync(folder, { withFileTypes: true })) {
		const file = path.join(folder, entry.name),
			name = prefix + entry.name;
		if (entry.isDirectory()) inventory(file, target, `${name}/`);
		else if (entry.isFile())
			target[name] = {
				bytes: fs.statSync(file).size,
				sha256: sha(fs.readFileSync(file)),
			};
	}
}
inventory(dist, evidence.assets);
inventory(fixtures, evidence.sources, 'fixtures/');
for (const name of [
	'apps/paintcraft-web/scripts/filter-worker-browser.mjs',
	'apps/paintcraft-web/src/lib/dds-worker-protocol.js',
	'apps/paintcraft-web/src/lib/msp-worker-protocol.js',
	'apps/paintcraft-web/src/lib/codec-worker-runtime.js',
	'apps/paintcraft-web/src/paintcraft-codec-worker/src/dds.rs',
	'apps/paintcraft-web/src/paintcraft-codec-worker/src/msp.rs',
	'crates/services/paintcraft-formats/src/export/filter_bake.rs',
	'crates/services/paintcraft-formats/src/export/web_snapshot.rs',
	'crates/services/paintcraft-formats/src/export/web_snapshot/resources.rs',
	...['dds', 'msp'].flatMap((k) =>
		['.rs', '/request.rs', '/response.rs', '/worker.rs'].map(
			(s) => `crates/services/paintcraft-formats/src/${k}/web_export${s}`,
		),
	),
])
	evidence.sources[name] = {
		sha256: sha(fs.readFileSync(path.join(root, name))),
	};
const runtime = fs
	.readdirSync(path.join(dist, '_astro'))
	.find((name) => /^codec-worker-runtime-.*\.js$/.test(name));
assert.ok(runtime, 'Production Worker runtime required');
evidence.runtime = runtime;
let server, browser;
try {
	server = http.createServer((req, res) => {
		const pathname = new URL(req.url, 'http://localhost').pathname;
		if (pathname === '/__probe') {
			res.setHeader('Content-Type', 'text/html');
			res.end('<!doctype html><title>Worker qualification</title>');
			return;
		}
		const base = pathname.startsWith('/__fixture/') ? fixtures : dist;
		const name = base === fixtures ? path.basename(pathname) : pathname;
		const file = path.resolve(base, `.${path.sep}${name}`);
		if (
			!file.startsWith(base + path.sep) ||
			!fs.existsSync(file) ||
			!fs.statSync(file).isFile()
		) {
			res.writeHead(404);
			res.end();
			return;
		}
		res.setHeader(
			'Content-Type',
			{ '.js': 'text/javascript', '.wasm': 'application/wasm' }[
				path.extname(file)
			] ?? 'application/octet-stream',
		);
		fs.createReadStream(file).pipe(res);
	});
	await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
	const { chromium } = await import(
		pathToFileURL(path.resolve(process.env.PLAYWRIGHT_MODULE)).href
	);
	browser = await chromium.launch({ channel: 'chrome', headless: true });
	evidence.browser = browser.version();
	const page = await browser.newPage();
	await page.goto(`http://127.0.0.1:${server.address().port}/__probe`);
	for (const kind of ['dds', 'msp'])
		for (const row of oracle.fixtures) {
			const source = fs.readFileSync(path.join(fixtures, row.file));
			assert.equal(sha(source), row.sha256);
			for (const fault of row.file === 'invert.pcnd'
				? [
						'none',
						'refuse',
						'digest',
						'state',
						'source-hash',
						'intent',
						'flags',
						'version',
						'target',
					]
				: ['none']) {
				const result = await page.evaluate(
					async ({ runtime, kind, row, oracle, fault }) => {
						const uuid = (text) =>
							Uint8Array.from(
								text.replaceAll('-', '').match(/../g),
								(s) => parseInt(s, 16),
							);
						const source = new Uint8Array(
							await (
								await fetch(`/__fixture/${row.file}`)
							).arrayBuffer(),
						);
						const h = new Uint8Array(224),
							view = new DataView(h.buffer);
						h.set(
							new TextEncoder().encode(
								kind === 'dds' ? 'PCDDSQ03' : 'PCMSPQ03',
							),
						);
						view.setUint16(8, 3);
						view.setUint32(12, source.length);
						view.setUint32(16, 2);
						view.setUint32(20, 1);
						h.set(uuid(oracle.document), 24);
						h.set(uuid(oracle.state), 40);
						view.setBigUint64(56, 9n);
						h[64] = 3;
						h.set(uuid('55555555-5555-4555-8555-555555555555'), 65);
						h.set(
							new Uint8Array(
								await crypto.subtle.digest('SHA-256', source),
							),
							81,
						);
						if (kind === 'dds') {
							h.set([0, 0, 0, 1, 0, 0, 0, 0], 113);
							h[121] = 4;
						} else {
							h[113] = 2;
							h[114] = 4;
						}
						h[122] = 1;
						h.set(
							Uint8Array.from(
								row.filter_inventory_sha256.match(/../g),
								(s) => parseInt(s, 16),
							),
							128,
						);
						h.set(uuid(oracle.document), 160);
						h[208] = row.metadata ? 3 : 2;
						h[209] = kind === 'dds' ? 1 : 2;
						h[210] = 1;
						if (row.metadata) {
							const start = 44,
								sourceView = new DataView(source.buffer);
							const extent =
								192 +
								Number(sourceView.getBigUint64(start + 112));
							h.set(
								source.subarray(start + 152, start + 184),
								176,
							);
							view.setUint32(212, extent);
							view.setBigUint64(216, BigInt(extent + 4096));
						}
						if (fault === 'refuse') h[122] = 0;
						if (fault === 'digest') h[128] ^= 1;
						if (fault === 'state') h[40] ^= 1;
						if (fault === 'source-hash') h[81] ^= 1;
						if (fault === 'intent') h[122] = 2;
						if (fault === 'flags')
							h[kind === 'dds' ? 121 : 114] = 8;
						if (fault === 'version') {
							h[7] = 50;
							view.setUint16(8, 2);
						}
						if (fault === 'target') h[209] = kind === 'dds' ? 2 : 1;
						const request = new Uint8Array(
							h.length + source.length,
						);
						request.set(h);
						request.set(source, h.length);
						const response = await new Promise(
							(resolve, reject) => {
								const worker = new Worker(
									`/_astro/${runtime}`,
									{ type: 'module' },
								);
								const timer = setTimeout(() => {
									worker.terminate();
									reject(
										Error(
											'Worker deadline; no reclaim claim',
										),
									);
								}, 30000);
								worker.onerror = (event) => {
									clearTimeout(timer);
									worker.terminate();
									reject(Error(event.message));
								};
								worker.onmessage = ({ data }) => {
									clearTimeout(timer);
									worker.terminate();
									resolve(data);
								};
								worker.postMessage(
									{
										kind: `${kind}-export`,
										bytes: request.buffer,
									},
									[request.buffer],
								);
							},
						);
						return {
							header: Array.from(h),
							responseKind: response.kind,
							response: response.bytes
								? Array.from(new Uint8Array(response.bytes))
								: null,
						};
					},
					{ runtime, kind, row, oracle, fault },
				);
				if (['intent', 'flags', 'version', 'target'].includes(fault))
					assert.equal(result.responseKind, 'failure');
				else {
					assert.equal(result.responseKind, 'result');
					const b = Buffer.from(result.response),
						h = Buffer.from(result.header);
					assert.equal(
						b.subarray(0, 8).toString(),
						kind === 'dds' ? 'PCDDSR03' : 'PCMSPR03',
					);
					assert.equal(b.readUInt16BE(8), 3);
					assert.equal(b[11], 0);
					assert.equal(b.readUInt32BE(12), 2);
					assert.equal(b.readUInt32BE(16), 1);
					assert.equal(b.readUInt32BE(20), source.length);
					assert.equal(b[265], h[kind === 'dds' ? 121 : 114]);
					if (kind === 'dds')
						assert.deepEqual(
							b.subarray(121, 129),
							h.subarray(113, 121),
						);
					else {
						assert.equal(b[121], 2);
						assert.deepEqual(b.subarray(122, 129), Buffer.alloc(7));
					}

					assert.deepEqual(b.subarray(32, 73), h.subarray(24, 65));
					assert.deepEqual(b.subarray(73, 89), h.subarray(65, 81));
					assert.deepEqual(b.subarray(89, 121), h.subarray(81, 113));
					assert.equal(b[266], h[122]);
					assert.deepEqual(b.subarray(267, 288), Buffer.alloc(21));
					assert.deepEqual(
						b.subarray(288, 320),
						h.subarray(128, 160),
					);
					assert.deepEqual(
						b.subarray(320, 384),
						h.subarray(160, 224),
					);
					assert.equal(
						b.subarray(199, 199 + b[198]).toString(),
						version,
					);
					assert.ok(b[198] > 0 && b[198] <= 64);
					assert.deepEqual(
						b.subarray(199 + b[198], 263),
						Buffer.alloc(64 - b[198]),
					);
					const length = b.readUInt32BE(24),
						previewSize =
							b.readUInt16BE(28) * b.readUInt16BE(30) * 4;
					const encoded = b.subarray(384, 384 + length),
						preview = b.subarray(
							384 + length,
							384 + length + previewSize,
						);
					assert.equal(
						sha(encoded),
						b.subarray(129, 161).toString('hex'),
					);
					assert.equal(
						sha(preview),
						b.subarray(161, 193).toString('hex'),
					);
					assert.equal(
						b.length,
						384 + length + previewSize + b.readUInt16BE(263),
					);
					if (fault !== 'none') {
						assert.equal(b[10], fault === 'refuse' ? 1 : 2);
						assert.equal(length, 0);
						assert.equal(previewSize, 0);
						assert.equal(b[193], 0);
						assert.deepEqual(b.subarray(194, 198), Buffer.alloc(4));
					} else {
						assert.equal(b[10], 0);
						assert.deepEqual(
							preview,
							Buffer.from(row.expected_rgba),
						);
						assert.equal(
							b[193],
							(kind === 'dds' ? 64 : 8) |
								(row.metadata ? (kind === 'dds' ? 32 : 4) : 0),
						);
						if (kind === 'dds') {
							assert.equal(
								encoded.subarray(0, 4).toString(),
								'DDS ',
							);
							assert.equal(encoded.readUInt32LE(12), 1);
							assert.equal(encoded.readUInt32LE(16), 2);
							assert.equal(
								encoded.subarray(84, 88).toString(),
								'DX10',
							);
							assert.equal(encoded.readUInt32LE(128), 29);
							assert.deepEqual(
								encoded.subarray(148),
								Buffer.from(row.expected_rgba),
							);
						} else {
							assert.equal(
								encoded.subarray(0, 4).toString(),
								'LinS',
							);
							assert.equal(encoded.readUInt16LE(4), 2);
							assert.equal(encoded.readUInt16LE(6), 1);
							assert.deepEqual(
								encoded.subarray(32),
								Buffer.from([2, 0, 1, row.enabled ? 128 : 64]),
							);
						}
					}
					const name = `${kind}-${row.file}-${fault}.response`;
					fs.writeFileSync(path.join(out, name), b);
					evidence.assets[`outputs/${name}`] = {
						bytes: b.length,
						sha256: sha(b),
					};
				}
				assert.equal(
					sha(fs.readFileSync(path.join(fixtures, row.file))),
					row.sha256,
				);
				evidence.cases.push({
					kind,
					file: row.file,
					fault,
					responseKind: result.responseKind,
					status: result.response?.[10] ?? null,
				});
				fs.writeFileSync(
					path.join(out, 'production.json'),
					JSON.stringify(evidence, null, 4),
				);
			}
		}
	evidence.status = 'passed';
} catch (error) {
	evidence.status = 'failed';
	evidence.error = String(error.stack ?? error);
	throw error;
} finally {
	if (browser) await browser.close();
	if (server) await new Promise((resolve) => server.close(resolve));
	evidence.finishedAt = new Date().toISOString();
	fs.writeFileSync(
		path.join(out, 'production.json'),
		JSON.stringify(evidence, null, 4) + '\n',
	);
	console.log(
		JSON.stringify({
			status: evidence.status,
			out,
			cases: evidence.cases.length,
		}),
	);
}
