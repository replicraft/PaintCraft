// Built persistent render Worker: independent six-kind PCND3/4 sources and planes.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import {
	pointExport4Request,
	pointExportFiles,
	runPointExport4,
} from './lib/point-export4.mjs';
import {
	assetInventory,
	evaluated,
	project,
	sha,
} from './lib/studio-filter-oracle.mjs';

const app = fileURLToPath(new URL('../', import.meta.url)),
	root = path.resolve(app, '../..');
const dist = path.join(app, 'dist');
const fixtures = path.join(
	root,
	'crates/services/paintcraft-formats/tests/pcnd-point-fixtures',
);
assert.ok(process.env.CODEC_EVIDENCE_DIR && process.env.PLAYWRIGHT_MODULE);
fs.mkdirSync(process.env.CODEC_EVIDENCE_DIR, { recursive: true });
const out = fs.mkdtempSync(
	path.join(process.env.CODEC_EVIDENCE_DIR, 'attempt-'),
);
const oracle = JSON.parse(fs.readFileSync(path.join(fixtures, 'oracle.json')));
const assets = assetInventory(dist);
const runtimes = Object.keys(assets).filter(
	(p) => p.startsWith('_astro/filter-preview-runtime-') && p.endsWith('.js'),
);
assert.equal(runtimes.length, 1);
const codecRuntimes = Object.keys(assets).filter(
	(p) => p.startsWith('_astro/codec-worker-runtime-') && p.endsWith('.js'),
);
assert.equal(codecRuntimes.length, 1);
const descriptors = [
	{ kind: 'black-white-rgb-v1', strength: 50 },
	{ kind: 'threshold-luma-v1', level: 128 },
	{ kind: 'posterize-rgb-v1', levels: 6 },
	{ kind: 'solarize-rgb-v1' },
	{ kind: 'brightness-contrast-v1', brightness: 10, contrast: -7 },
];
const typed = [
	{ kind: 3, strength: 50 },
	{ kind: 4, level: 128 },
	{ kind: 5, levels: 6 },
	{ kind: 6 },
	{ kind: 2, brightness: 10, contrast: -7 },
];
const evidence = {
	status: 'started',
	assets,
	cases: [],
	sources: {},
	startedAt: new Date().toISOString(),
};
for (const file of [
	fileURLToPath(import.meta.url),
	path.join(app, 'scripts/lib/studio-filter-oracle.mjs'),
	path.join(app, 'scripts/lib/point-export4.mjs'),
	path.join(fixtures, 'oracle.json'),
]) {
	evidence.sources[path.relative(root, file)] = sha(fs.readFileSync(file));
}
fs.copyFileSync(
	fileURLToPath(import.meta.url),
	path.join(out, 'executed-driver.mjs'),
);
let browser, server;
try {
	server = http.createServer((request, response) => {
		const pathname = new URL(request.url, 'http://127.0.0.1').pathname;
		if (pathname === '/__probe')
			return response.end(
				'<!doctype html><title>Independent point Worker qualification</title>',
			);
		if (pathname.startsWith('/__fixture/')) {
			const name = pathname.slice('/__fixture/'.length);
			if (!oracle.fixtures.some((r) => r.file === name))
				return response.writeHead(404).end();
			return response.end(fs.readFileSync(path.join(fixtures, name)));
		}
		const file = path.resolve(dist, `.${pathname}`);
		if (
			!file.startsWith(`${dist}/`) ||
			!fs.existsSync(file) ||
			!fs.statSync(file).isFile()
		)
			return response.writeHead(404).end();
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
		const source = fs.readFileSync(path.join(fixtures, row.file));
		assert.equal(sha(source), row.sha256);
		const updates = [];
		if (row.accepted) {
			const original = project(source);
			assert.deepEqual([...evaluated(original)], row.evaluated_rgba);
			for (let i = 0; i < descriptors.length; i++) {
				const doc = structuredClone(original);
				const first = doc.layers[0].filters[0];
				doc.layers[0].filters[0] = {
					id: first.id,
					enabled: first.enabled,
					mask: first.mask,
					...typed[i],
				};
				const pixels = [...evaluated(doc)];
				for (let p = 0; p < pixels.length; p += 4)
					if (pixels[p + 3] === 0) pixels.fill(0, p, p + 3);
				updates.push({ descriptor: descriptors[i], pixels });
			}
		}
		const results = await page.evaluate(
			async ({ row, oracle, updates, runtime }) => {
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
				const equal = (a, b, label) => {
					if (
						JSON.stringify(canonical(a)) !==
						JSON.stringify(canonical(b))
					)
						throw Error(
							`${label}: ${JSON.stringify(a)} != ${JSON.stringify(b)}`,
						);
				};
				const digest = async (value) =>
					[
						...new Uint8Array(
							await crypto.subtle.digest('SHA-256', value),
						),
					]
						.map((b) => b.toString(16).padStart(2, '0'))
						.join('');
				const source = new Uint8Array(
					await (await fetch(`/__fixture/${row.file}`)).arrayBuffer(),
				);
				equal(await digest(source), row.sha256, 'source SHA');
				const valid = row.accepted
					? row
					: oracle.fixtures.find((f) => f.accepted);
				const owner = {
					session: '44444444-4444-4444-8444-444444444444',
					state: oracle.state,
					epoch: '9',
					frontend: 3,
				};
				const plan = {
					owner,
					draft: '55555555-5555-4555-8555-555555555555',
					document: oracle.document,
					sourceSha256: row.sha256,
					size: [oracle.width, oracle.height],
					layers: 1,
					residentBytes: 262144,
					metadataBytes: row.attachment ? oracle.profile_bytes : 0,
					metadataCharge: row.attachment ? 1048576 : 0,
					filterDigest: valid.filter_inventory_sha256,
					maskCount: valid.mask_count,
					maskBands: valid.bands,
					maskRuns: valid.runs,
					maskDecodeBytes: valid.bands ? 2097152 : 0,
				};
				const worker = new Worker(runtime, { type: 'module' }),
					waiting = new Map();
				let id = 0;
				worker.onmessage = ({ data }) => {
					const item = waiting.get(data?.id);
					if (item) {
						waiting.delete(data.id);
						clearTimeout(item.timer);
						item.resolve(data);
					}
				};
				worker.onerror = (e) => {
					for (const item of waiting.values()) {
						clearTimeout(item.timer);
						item.reject(Error(e.message));
					}
					waiting.clear();
				};
				const send = (kind, body) =>
					new Promise((resolve, reject) => {
						const next = ++id,
							timer = setTimeout(() => {
								waiting.delete(next);
								reject(Error(`${kind} deadline`));
							}, 30000);
						waiting.set(next, { resolve, reject, timer });
						worker.postMessage({ id: next, kind, ...body });
					});
				const cases = [];
				try {
					const ready = await send('open', {
						plan,
						source: source.slice().buffer,
					});
					if (!row.accepted) {
						equal(
							[ready.kind, ready.code, ready.completed],
							['error', row.code, true],
							'strict source refusal',
						);
						return [
							{
								name: row.file,
								kind: 'refused',
								code: ready.code,
							},
						];
					}
					equal(ready.kind, 'ready', 'OPEN');
					equal(
						ready.binding,
						plan,
						'READY all source/resource/inventory facts',
					);
					let generation = 0;
					const view = {
						region: [0, 0, oracle.width, oracle.height],
						level: 0,
					};
					const request = (operation = null, render = true) => ({
						owner,
						draft: plan.draft,
						sourceSha256: row.sha256,
						generation: String(++generation),
						operation,
						target: operation ? oracle.layer : null,
						view,
						render,
					});
					const frame = async (req, pixels) => {
						const result = await send('update', { request: req });
						equal(
							result.kind,
							req.render ? 'frame' : 'prepared',
							'result kind',
						);
						for (const key of [
							'owner',
							'draft',
							'sourceSha256',
							'generation',
							'target',
							'view',
						])
							equal(result[key], req[key], `echo ${key}`);
						equal(
							result.operationSha256,
							await digest(
								new TextEncoder().encode(
									JSON.stringify(canonical(req.operation)),
								),
							),
							'operation SHA',
						);
						equal(
							[
								result.width,
								result.height,
								result.region,
								result.level,
							],
							[oracle.width, oracle.height, view.region, 0],
							'actual ROI',
						);
						if (req.render)
							equal(
								[...result.pixels],
								pixels,
								'independent full plane',
							);
						else
							equal(
								result.pixels,
								undefined,
								'PREPARED no raster',
							);
						cases.push({
							name: row.file,
							generation: req.generation,
							kind: result.kind,
							operation: req.operation,
							pixels: req.render ? [...result.pixels] : undefined,
						});
					};
					await frame(request(), row.rendered_rgba);
					for (const update of updates) {
						const operation = {
							op: 'filter.update',
							layer_ref: { id: oracle.layer },
							filter_ref: { id: row.filters[0].id },
							descriptor: update.descriptor,
						};
						await frame(request(operation, false));
						await frame(request(operation), update.pixels);
					}
					const invalid = {
						op: 'filter.update',
						layer_ref: { id: oracle.layer },
						filter_ref: { id: row.filters[0].id },
						descriptor: { kind: 'posterize-rgb-v1', levels: 1 },
					};
					const rejected = await send('update', {
						request: request(invalid),
					});
					equal(
						[rejected.kind, rejected.code, rejected.completed],
						['error', 'input.schema', true],
						'invalid parameter remains strict',
					);
					const foreign = request();
					foreign.owner = {
						...owner,
						state: '99999999-9999-4999-8999-999999999999',
					};
					const stale = await send('update', { request: foreign });
					equal(
						[stale.kind, stale.code, stale.completed],
						['error', 'conflict.stale_base', true],
						'foreign owner',
					);
					const cancel = String(++generation),
						retired = await send('cancel', { generation: cancel });
					equal(
						[retired.kind, retired.generation],
						['retired', cancel],
						'retirement exact',
					);
					await frame(request(), row.rendered_rgba);
					equal(
						(await send('close', {})).kind,
						'closed',
						'source release acknowledgement',
					);
					cases.push({
						name: row.file,
						sourceOpens: 1,
						invalidParameter: rejected.code,
						foreignOwner: stale.code,
						retired: cancel,
						closed: true,
					});
					return cases;
				} finally {
					for (const item of waiting.values())
						clearTimeout(item.timer);
					waiting.clear();
					worker.terminate();
				}
			},
			{ row, oracle, updates, runtime: `/${runtimes[0]}` },
		);
		evidence.cases.push(...results);
		fs.writeFileSync(
			path.join(out, 'production.json'),
			`${JSON.stringify(evidence, null, 4)}\n`,
		);
	}
	const exportResults = [];
	for (const file of pointExportFiles) {
		const row = oracle.fixtures.find((entry) => entry.file === file);
		assert.ok(row?.accepted && !row.attachment);
		const source = fs.readFileSync(path.join(fixtures, file));
		assert.equal(sha(source), row.sha256);
		for (const kind of ['dds', 'msp'])
			for (const bake of [false, true]) {
				const request = pointExport4Request(
					source,
					row,
					oracle,
					kind,
					bake,
				);
				const result = await page.evaluate(runPointExport4, {
					request: [...request],
					row,
					runtime: `/${codecRuntimes[0]}`,
					kind,
					bake,
				});
				if (result.encoded.length) {
					const artifact = `${file}-${kind}-bake.${kind}`;
					fs.writeFileSync(
						path.join(out, artifact),
						Buffer.from(result.encoded),
					);
					result.artifact = artifact;
				}
				delete result.encoded;
				exportResults.push(result);
				evidence.cases.push(result);
				fs.writeFileSync(
					path.join(out, 'production.json'),
					`${JSON.stringify(evidence, null, 4)}\n`,
				);
			}
	}
	assert.equal(exportResults.length, 32);
	const outputs = exportResults.filter((row) => row.artifact);
	assert.equal(
		outputs.length,
		9,
		'eight DDS plus one exact unmasked threshold MSP',
	);
	const python =
		process.env.PILLOW_PYTHON ??
		'/Library/Frameworks/Python.framework/Versions/3.12/bin/python3.12';
	const probe = spawnSync(
		python,
		[
			'-B',
			'-c',
			'import json,sys,PIL;from PIL import Image;print(json.dumps({"version":PIL.__version__,"images":[{"file":p,"size":list(Image.open(p).size),"rgba":list(Image.open(p).convert("RGBA").tobytes())} for p in sys.argv[1:]]}))',
			...outputs.map((row) => path.join(out, row.artifact)),
		],
		{ encoding: 'utf8' },
	);
	fs.writeFileSync(path.join(out, 'pillow.stdout'), probe.stdout ?? '');
	fs.writeFileSync(path.join(out, 'pillow.stderr'), probe.stderr ?? '');
	assert.equal(probe.status, 0, probe.stderr);
	const decoded = JSON.parse(probe.stdout);
	assert.equal(decoded.images.length, outputs.length);
	for (let i = 0; i < outputs.length; i++) {
		assert.deepEqual(decoded.images[i].size, [2, 1]);
		assert.deepEqual(decoded.images[i].rgba, outputs[i].expected);
	}
	evidence.independentPillow = decoded.version;
	assert.deepEqual(assetInventory(dist), assets);
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
	const cleanupError = (label, error) => {
		evidence.cleanupErrors ??= [];
		evidence.cleanupErrors.push({
			label,
			error: String(error.stack ?? error),
		});
		evidence.status = 'failed';
		process.exitCode = 1;
	};
	try {
		await browser
			?.close()
			.catch((error) => cleanupError('browser close', error));
	} finally {
		try {
			if (server) {
				server.closeAllConnections();
				await new Promise((resolve, reject) =>
					server.close((error) =>
						error ? reject(error) : resolve(),
					),
				).catch((error) => cleanupError('server close', error));
			}
		} finally {
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
	}
}
