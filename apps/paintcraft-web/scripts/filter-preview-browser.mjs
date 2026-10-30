// Exercise the actual built persistent renderer, independently of editor UI.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const app = fileURLToPath(new URL('../', import.meta.url));
const root = path.resolve(app, '../..');
const dist = path.join(app, 'dist');
const fixtureRoot = path.join(
	root,
	'crates/services/paintcraft-formats/tests/browser-filter-export-fixtures',
);
assert.ok(process.env.CODEC_EVIDENCE_DIR && process.env.PLAYWRIGHT_MODULE);
fs.mkdirSync(process.env.CODEC_EVIDENCE_DIR, { recursive: true });
const out = fs.mkdtempSync(
	path.join(process.env.CODEC_EVIDENCE_DIR, 'attempt-'),
);
const sha = (bytes) => createHash('sha256').update(bytes).digest('hex');
const oracle = JSON.parse(
	fs.readFileSync(path.join(fixtureRoot, 'oracle.json')),
);
const row = oracle.fixtures.find((f) => f.file === 'invert.pcnd');
const bytes = fs.readFileSync(path.join(fixtureRoot, row.file));
assert.equal(sha(bytes), row.sha256);
const runtimes = fs
	.readdirSync(path.join(dist, '_astro'))
	.filter((n) => /^filter-preview-runtime-.*\.js$/.test(n));
assert.equal(runtimes.length, 1, 'Exactly one production render runtime');
const evidence = {
	status: 'started',
	scope: 'Actual production persistent Worker; editor UI, latency, RSS and physical reclamation are separate qualifications.',
	assets: {},
	fixture: row,
	startedAt: new Date().toISOString(),
};
function inventory(folder) {
	for (const entry of fs.readdirSync(folder, { withFileTypes: true })) {
		const file = path.join(folder, entry.name);
		if (entry.isDirectory()) inventory(file);
		else
			evidence.assets[path.relative(dist, file)] = {
				bytes: fs.statSync(file).size,
				sha256: sha(fs.readFileSync(file)),
			};
	}
}
inventory(dist);
fs.copyFileSync(
	fileURLToPath(import.meta.url),
	path.join(out, 'driver-source.mjs'),
);
let browser, server;
try {
	server = http.createServer((request, response) => {
		const pathname = new URL(request.url, 'http://localhost').pathname;
		if (pathname === '/__probe') {
			response.setHeader('Content-Type', 'text/html');
			response.end(
				'<!doctype html><title>Persistent render qualification</title>',
			);
			return;
		}
		if (pathname === '/__fixture') {
			response.setHeader('Content-Type', 'application/octet-stream');
			response.end(bytes);
			return;
		}
		const file = path.resolve(dist, `.${pathname}`);
		if (
			!file.startsWith(`${dist}/`) ||
			!fs.existsSync(file) ||
			!fs.statSync(file).isFile()
		) {
			response.writeHead(404);
			response.end();
			return;
		}
		response.setHeader(
			'Content-Type',
			{
				'.js': 'text/javascript',
				'.wasm': 'application/wasm',
				'.json': 'application/json',
			}[path.extname(file)] ?? 'application/octet-stream',
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
	evidence.cases = await page.evaluate(
		async ({ runtime, oracle, fixture }) => {
			const source = new Uint8Array(
				await (await fetch('/__fixture')).arrayBuffer(),
			);
			const owner = {
				session: '44444444-4444-4444-8444-444444444444',
				state: oracle.state,
				epoch: '0',
				frontend: 3,
			};
			const plan = {
				owner,
				draft: '55555555-5555-4555-8555-555555555555',
				document: oracle.document,
				sourceSha256: fixture.sha256,
				size: [2, 1],
				layers: 1,
				residentBytes: 256 * 256 * 4,
				metadataBytes: 0,
				metadataCharge: 0,
				filterDigest: fixture.filter_inventory_sha256,
			};
			const layer = '22222222-2222-4222-8222-222222222222';
			const nil = '00000000-0000-0000-0000-000000000000';
			const view = { region: [0, 0, 2, 1], level: 0 };
			const cases = [];
			const canonical = (value) => {
				if (Array.isArray(value)) return value.map(canonical);
				if (value !== null && typeof value === 'object')
					return Object.fromEntries(
						Object.keys(value)
							.sort()
							.map((key) => [key, canonical(value[key])]),
					);
				return value;
			};
			function equal(actual, expected, label) {
				if (
					JSON.stringify(canonical(actual)) !==
					JSON.stringify(canonical(expected))
				)
					throw new Error(
						`${label}: ${JSON.stringify(actual)} != ${JSON.stringify(expected)}`,
					);
			}
			function client() {
				const worker = new Worker(`/_astro/${runtime}`, {
					type: 'module',
				});
				const pending = new Map();
				let sequence = 0;
				worker.onmessage = ({ data }) => {
					const p = pending.get(data?.id);
					if (p) {
						pending.delete(data.id);
						clearTimeout(p.timer);
						p.resolve(data);
					}
				};
				worker.onerror = (event) => {
					for (const p of pending.values()) {
						clearTimeout(p.timer);
						p.reject(new Error(event.message));
					}
					pending.clear();
				};
				return {
					worker,
					send(kind, body, transfer = []) {
						const id = ++sequence;
						return new Promise((resolve, reject) => {
							const timer = setTimeout(() => {
								pending.delete(id);
								reject(new Error(`Timed out ${kind}`));
							}, 30000);
							pending.set(id, { resolve, reject, timer });
							worker.postMessage({ kind, id, ...body }, transfer);
						});
					},
				};
			}
			const service = client();
			try {
				const copied = source.slice();
				const ready = await service.send(
					'open',
					{ plan, source: copied.buffer },
					[copied.buffer],
				);
				equal(ready.kind, 'ready', 'Actual source ready');
				for (const key of Object.keys(plan))
					equal(ready.binding[key], plan[key], `Ready ${key}`);
				cases.push({ name: 'open-source-once', kind: ready.kind });
				const request = (
					generation,
					operation = null,
					render = true,
				) => ({
					owner,
					draft: plan.draft,
					sourceSha256: plan.sourceSha256,
					generation: String(generation),
					operation,
					target: operation ? layer : null,
					view,
					render,
				});
				const update = async (name, req, expected) => {
					const result = await service.send('update', {
						request: req,
					});
					equal(result.kind, req.render ? 'frame' : 'prepared', name);
					for (const key of [
						'owner',
						'draft',
						'sourceSha256',
						'generation',
						'target',
						'view',
					])
						equal(result[key], req[key], `${name} ${key}`);
					const operationHash = await crypto.subtle.digest(
						'SHA-256',
						new TextEncoder().encode(
							JSON.stringify(canonical(req.operation)),
						),
					);
					equal(
						result.operationSha256,
						Array.from(new Uint8Array(operationHash), (b) =>
							b.toString(16).padStart(2, '0'),
						).join(''),
						`${name} operation digest`,
					);
					equal(
						[
							result.width,
							result.height,
							result.region,
							result.level,
						],
						[2, 1, [0, 0, 2, 1], 0],
						`${name} geometry`,
					);
					if (req.render)
						equal(
							Array.from(result.pixels),
							expected,
							`${name} independent pixels`,
						);
					else
						equal(
							result.pixels,
							undefined,
							`${name} no raster frame`,
						);
					cases.push({
						name,
						kind: result.kind,
						generation: result.generation,
						pixels: result.pixels
							? Array.from(result.pixels)
							: undefined,
					});
				};
				await update(
					'base-active-invert',
					request(1),
					fixture.expected_rgba,
				);
				const op = (name, fields) => ({
					op: name,
					layer_ref: { id: layer },
					...fields,
				});
				await update(
					'disable-private-filter',
					request(
						2,
						op('filter.set_enabled', {
							filter_ref: { id: nil },
							enabled: false,
						}),
					),
					oracle.raw_rgba,
				);
				await update(
					'brightness-replaces-invert',
					request(
						3,
						op('filter.update', {
							filter_ref: { id: nil },
							descriptor: {
								kind: 'brightness-contrast-v1',
								brightness: 25,
								contrast: 0,
							},
						}),
					),
					[64, 64, 64, 255, 255, 255, 255, 255],
				);
				await update(
					'preview-off-admits-without-frame',
					request(
						4,
						op('filter.update', {
							filter_ref: { id: nil },
							descriptor: {
								kind: 'brightness-contrast-v1',
								brightness: -50,
								contrast: 0,
							},
						}),
						false,
					),
				);
				const bad = await service.send('update', {
					request: request(
						5,
						op('filter.set_enabled', {
							filter_ref: { ref: 'unknown' },
							enabled: false,
						}),
					),
				});
				equal(bad.kind, 'error', 'Unresolved aliases refused');
				cases.push({
					name: 'unresolved-filter-refusal',
					kind: bad.kind,
					code: bad.code,
				});
				await update(
					'base-stays-immutable-after-drafts',
					request(6),
					fixture.expected_rgba,
				);
				await update(
					'second-invert-ordered-once',
					request(
						7,
						op('filter.add', {
							ref: 'second',
							descriptor: { kind: 'invert-rgb-v1' },
							enabled: true,
						}),
					),
					oracle.raw_rgba,
				);
				const retired = await service.send('cancel', {
					generation: '7',
				});
				equal(
					[retired.kind, retired.generation],
					['retired', '7'],
					'Explicit retirement',
				);
				cases.push({
					name: 'ordinary-retirement-ack',
					kind: retired.kind,
				});
				await update(
					'base-reuses-open-source-after-cancel',
					request(8),
					fixture.expected_rgba,
				);
				const closed = await service.send('close', {});
				equal(closed.kind, 'closed', 'Source close acknowledged');
				cases.push({ name: 'close-source-ack', kind: closed.kind });
			} finally {
				service.worker.terminate();
			}
			for (const [name, changed, expected] of [
				[
					'wrong-source-sha',
					{ sourceSha256: '00'.repeat(32) },
					'conflict.stale_base',
				],
				['wrong-canvas-facts', { size: [3, 1] }, 'conflict.stale_base'],
				[
					'predecode-resource-refusal',
					{ layers: 100000 },
					'resource.limit',
				],
			]) {
				const negative = client();
				try {
					const copied = source.slice();
					const result = await negative.send(
						'open',
						{
							plan: { ...plan, ...changed },
							source: copied.buffer,
						},
						[copied.buffer],
					);
					equal(
						[result.kind, result.code],
						['error', expected],
						name,
					);
					cases.push({ name, kind: result.kind, code: result.code });
				} finally {
					negative.worker.terminate();
				}
			}
			return cases;
		},
		{ runtime: runtimes[0], oracle, fixture: row },
	);
	for (const [name, record] of Object.entries(evidence.assets))
		assert.equal(
			sha(fs.readFileSync(path.join(dist, name))),
			record.sha256,
		);
	assert.equal(
		sha(fs.readFileSync(path.join(fixtureRoot, row.file))),
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
			cases: evidence.cases?.length ?? 0,
		}),
	);
}
