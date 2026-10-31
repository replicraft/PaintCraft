// Real frozen production renderWorker3: independent PCND5 sources and Box planes.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import {
	nativeSource,
	rendered,
	spatialPlan,
} from './lib/spatial-worker-fixtures.mjs';
import { assetInventory, project, sha } from './lib/studio-filter-oracle.mjs';

const app = fileURLToPath(new URL('../', import.meta.url)),
	root = path.resolve(app, '../..'),
	dist = path.join(app, 'dist');
const folder = path.join(
	root,
	'crates/services/paintcraft-formats/tests/pcnd-spatial-fixtures',
);
assert.ok(process.env.CODEC_EVIDENCE_DIR && process.env.PLAYWRIGHT_MODULE);
fs.mkdirSync(process.env.CODEC_EVIDENCE_DIR, { recursive: true });
const out = fs.mkdtempSync(
	path.join(process.env.CODEC_EVIDENCE_DIR, 'attempt-'),
);
const oracle = JSON.parse(fs.readFileSync(path.join(folder, 'oracle.json'))),
	assets = assetInventory(dist);
const runtimes = Object.keys(assets).filter((p) =>
	/^_astro\/filter-preview-runtime-.*\.js$/.test(p),
);
assert.equal(runtimes.length, 1);
const sources = new Map(),
	cases = [];
for (const row of oracle.fixtures.filter((r) => r.accepted)) {
	const source = fs.readFileSync(path.join(folder, row.file));
	assert.equal(sha(source), row.sha256);
	const doc = project(source),
		candidate = structuredClone(doc),
		filter = candidate.layers[0].filters.find((f) => f.kind === 7);
	filter.radius = 1;
	const operation = {
		op: 'filter.update',
		layer_ref: { id: doc.layers[0].id },
		filter_ref: { id: filter.id },
		descriptor: { kind: 'box-blur-v1', radius: 1 },
	};
	sources.set(row.file, source);
	cases.push({
		name: row.file,
		plan: spatialPlan(source, doc, row.stack_sha256),
		layer: doc.layers[0].id,
		operation,
		base: rendered(doc),
		updated: rendered(candidate),
	});
}
const native = nativeSource(oracle),
	nativeDoc = project(native.source);
sources.set('native-cancel.pcnd', native.source);
fs.writeFileSync(path.join(out, 'native-cancel.pcnd'), native.source);
cases.push({
	name: 'native-cancel.pcnd',
	plan: spatialPlan(native.source, nativeDoc, native.stack, 0),
	layer: oracle.layer,
	native: true,
	operation: {
		op: 'filter.add',
		layer_ref: { id: oracle.layer },
		ref: 'box',
		descriptor: { kind: 'box-blur-v1', radius: 2 },
		enabled: true,
	},
	base: [20, 40, 60, 255],
	updated: [20, 40, 60, 255],
});
const evidence = {
	status: 'started',
	scope: 'Finite raw production Worker protocol3/source-once/typed-refusal/retirement proof; no UI/scheduling latency or cancellation-step-count claim',
	assets,
	cases: [],
	sources: {},
	startedAt: new Date().toISOString(),
};
for (const file of [
	fileURLToPath(import.meta.url),
	path.join(app, 'scripts/lib/spatial-worker-fixtures.mjs'),
	path.join(app, 'scripts/lib/studio-filter-oracle.mjs'),
	path.join(app, 'scripts/lib/studio-box-oracle.mjs'),
	path.join(folder, 'oracle.json'),
]) {
	evidence.sources[path.relative(root, file)] = sha(fs.readFileSync(file));
	fs.copyFileSync(file, path.join(out, `executed-${path.basename(file)}`));
}
fs.copyFileSync(
	fileURLToPath(import.meta.url),
	path.join(out, 'executed-driver.mjs'),
);
let server, browser;
try {
	server = http.createServer((req, res) => {
		const pathname = new URL(req.url, 'http://localhost').pathname;
		if (pathname === '/__probe')
			return res.end(
				'<!doctype html><title>Independent Box Worker3</title>',
			);
		if (pathname.startsWith('/__source/')) {
			const source = sources.get(pathname.slice(10));
			return source ? res.end(source) : res.writeHead(404).end();
		}
		const file = path.resolve(dist, `.${pathname}`);
		if (
			!file.startsWith(`${dist}/`) ||
			!fs.existsSync(file) ||
			!fs.statSync(file).isFile()
		)
			return res.writeHead(404).end();
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
	for (const spec of cases) {
		const result = await page.evaluate(
			async ({ spec, runtime }) => {
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
				const digest = async (bytes) =>
					[
						...new Uint8Array(
							await crypto.subtle.digest('SHA-256', bytes),
						),
					]
						.map((b) => b.toString(16).padStart(2, '0'))
						.join('');
				const worker = new Worker(runtime, { type: 'module' }),
					pending = new Map(),
					events = [];
				let sequence = 0;
				worker.onmessage = ({ data }) => {
					events.push(data);
					const p = pending.get(data.id);
					if (p) {
						pending.delete(data.id);
						clearTimeout(p.timer);
						p.resolve(data);
					}
				};
				worker.onerror = (e) => {
					for (const p of pending.values()) {
						clearTimeout(p.timer);
						p.reject(Error(e.message));
					}
					pending.clear();
				};
				const send = (kind, body = {}, transfer = []) => {
					const id = ++sequence;
					return new Promise((resolve, reject) => {
						const timer = setTimeout(() => {
							pending.delete(id);
							reject(Error(`Worker ${kind} deadline`));
						}, 30000);
						pending.set(id, { resolve, reject, timer });
						worker.postMessage({ kind, id, ...body }, transfer);
					});
				};
				const source = new Uint8Array(
						await (
							await fetch(`/__source/${spec.name}`)
						).arrayBuffer(),
					),
					checks = [];
				equal(
					await digest(source),
					spec.plan.sourceSha256,
					'independent whole source SHA',
				);
				const request = (
					generation,
					operation = null,
					render = true,
				) => ({
					owner: spec.plan.owner,
					draft: spec.plan.draft,
					sourceSha256: spec.plan.sourceSha256,
					generation: String(generation),
					operation,
					target: operation ? spec.layer : null,
					view: {
						region: [0, 0, ...spec.plan.size],
						level: spec.native ? 8 : 0,
					},
					render,
				});
				async function check(reply, req, pixels) {
					equal(
						reply.kind,
						req.render ? 'frame' : 'prepared',
						'complete kind',
					);
					for (const name of [
						'owner',
						'draft',
						'sourceSha256',
						'generation',
						'target',
						'view',
					])
						equal(reply[name], req[name], `exact ${name}`);
					equal(
						reply.operationSha256,
						await digest(
							new TextEncoder().encode(
								JSON.stringify(canonical(req.operation)),
							),
						),
						'canonical operation SHA',
					);
					equal(
						reply.region,
						[0, 0, ...spec.plan.size],
						'native ROI',
					);
					equal(reply.level, req.view.level, 'LOD');
					equal(
						[reply.width, reply.height],
						spec.native ? [1, 1] : spec.plan.size,
						'frame shape',
					);
					if (req.render)
						equal(
							[...reply.pixels],
							pixels,
							'every independent RGBA sample',
						);
					else
						equal(reply.pixels, undefined, 'no unrequested pixels');
					checks.push({
						kind: reply.kind,
						generation: req.generation,
						operationSha256: reply.operationSha256,
						sourceSha256: reply.sourceSha256,
						region: reply.region,
						level: reply.level,
						pixels: req.render ? [...reply.pixels] : null,
					});
				}
				try {
					const copy = source.slice(),
						ready = await send(
							'open',
							{ source: copy.buffer, plan: spec.plan },
							[copy.buffer],
						);
					equal(ready.kind, 'ready', 'OPEN');
					equal(
						ready.binding,
						spec.plan,
						'complete decoded READY facts',
					);
					checks.push({ kind: 'ready', binding: ready.binding });
					for (const [generation, operation, render, pixels] of [
						[1, null, true, spec.base],
						[2, spec.operation, false, null],
						[3, spec.operation, true, spec.updated],
					]) {
						const req = request(generation, operation, render);
						await check(
							await send('update', { request: req }),
							req,
							pixels,
						);
					}
					const forged = request(4, spec.operation);
					forged.owner = { ...forged.owner, epoch: '1' };
					const stale = await send('update', { request: forged });
					equal(stale.kind, 'error', 'wrong owner refusal');
					equal(
						stale.code,
						'conflict.stale_base',
						'typed owner refusal',
					);
					checks.push({ kind: 'stale-refused', code: stale.code });
					if (spec.native) {
						// Runtime cancellation may retire queued work or work between native
						// tiles. Rust tests separately prove exactly one native tile per step.
						const active = send('update', {
							request: request(4, spec.operation),
						});
						active.catch(() => {});
						const cancel = await send('cancel', {
								generation: '4',
							}),
							retired = await active;
						equal(cancel.kind, 'retired', 'cancel acknowledgement');
						equal(cancel.generation, '4', 'cancel generation');
						equal(
							retired.kind,
							'retired',
							'no partial native LOD frame published',
						);
						checks.push({
							kind: 'native-lod-retired',
							generation: '4',
						});
					}
					const req = request(5, null, true);
					await check(
						await send('update', { request: req }),
						req,
						spec.base,
					);
					equal(
						events.filter((e) => e.kind === 'ready').length,
						1,
						'one source decode/READY lifecycle',
					);
					equal(
						(await send('close')).kind,
						'closed',
						'real close acknowledgement',
					);
					equal(
						await digest(source),
						spec.plan.sourceSha256,
						'original transferred source retained exactly',
					);
					return checks;
				} finally {
					worker.terminate();
					for (const p of pending.values()) {
						clearTimeout(p.timer);
						p.reject(Error('Probe disposed'));
					}
					pending.clear();
				}
			},
			{ spec, runtime: `/${runtimes[0]}` },
		);
		evidence.cases.push({
			name: spec.name,
			plan: spec.plan,
			checks: result,
		});
		fs.writeFileSync(
			path.join(out, 'progress.json'),
			JSON.stringify(evidence, null, 2),
		);
	}
	// Closed legacy source2 must refuse the same actual container5 before decode.
	const spec = cases[0];
	evidence.legacyRefusal = await page.evaluate(
		async ({ spec, runtime }) => {
			const source = await (
					await fetch(`/__source/${spec.name}`)
				).arrayBuffer(),
				plan = { ...spec.plan };
			for (const key of [
				'protocol',
				'spatialCount',
				'workspaceBytes',
				'workLimit',
			])
				delete plan[key];
			const worker = new Worker(runtime, { type: 'module' });
			let timer;
			try {
				const result = await new Promise((resolve, reject) => {
					timer = setTimeout(
						() => reject(Error('Legacy refusal deadline')),
						30000,
					);
					worker.onmessage = ({ data }) => resolve(data);
					worker.onerror = (e) => reject(Error(e.message));
					worker.postMessage({ kind: 'open', id: 1, plan, source }, [
						source,
					]);
				});
				if (
					result.kind !== 'error' ||
					result.code !== 'capability.unsupported'
				)
					throw Error(JSON.stringify(result));
				return result;
			} finally {
				clearTimeout(timer);
				worker.terminate();
			}
		},
		{ spec, runtime: `/${runtimes[0]}` },
	);
	assert.deepEqual(assetInventory(dist), assets);
	evidence.status = 'passed';
} catch (error) {
	evidence.status = 'failed';
	evidence.error = String(error.stack ?? error);
	process.exitCode = 1;
} finally {
	try {
		await browser?.close();
	} finally {
		try {
			if (server?.listening)
				await new Promise((resolve) => server.close(resolve));
		} finally {
			evidence.finishedAt = new Date().toISOString();
			fs.writeFileSync(
				path.join(out, 'final.json'),
				JSON.stringify(evidence, null, 2),
			);
			console.log(
				JSON.stringify({
					out,
					status: evidence.status,
					sourceCases: evidence.cases.length,
				}),
			);
		}
	}
}
