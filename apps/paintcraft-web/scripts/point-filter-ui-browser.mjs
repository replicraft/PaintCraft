// Real Studio UI on immutable production assets. Calibration never counts as a pass.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { canonical, matchingPointPair } from './lib/point-ui-bindings.mjs';
import {
	assetInventory,
	canvasMarker,
	evaluated,
	project,
	rawSemantics,
	sha,
} from './lib/studio-filter-oracle.mjs';

const app = fileURLToPath(new URL('../', import.meta.url)),
	root = path.resolve(app, '../..'),
	dist = path.join(app, 'dist');
const legacyFixtures = path.join(
	root,
	'crates/services/paintcraft-formats/tests/pcnd-point-fixtures',
);
const boxCase = {
	kind: 'box',
	default_rgba: [153, 80, 0, 255, 102, 120, 0, 255],
	changed_rgba: [170, 67, 0, 255, 85, 133, 0, 255],
};
const kinds = {
	bw: { kind: 3, field: 'strength', initial: 100, value: 50 },
	threshold: { kind: 4, field: 'level', initial: 128, value: 200 },
	posterize: { kind: 5, field: 'levels', initial: 6, value: 2 },
	solarize: { kind: 6 },
	box: { kind: 7, field: 'radius', initial: 2, value: 1 },
};
const kind = process.env.POINT_UI_KIND,
	config = kinds[kind];
assert.ok(
	config && process.env.CODEC_EVIDENCE_DIR && process.env.PLAYWRIGHT_MODULE,
);
const phase = process.env.POINT_UI_PHASE ?? 'all';
assert.ok(['layout', 'menu', 'parameters', 'all'].includes(phase));
const name = process.env.POINT_UI_SOURCE ?? 'red-green.pcnd';
const fixtures =
	name === 'red-green.pcnd'
		? path.join(
				root,
				'crates/services/paintcraft-formats/tests/browser-point-ui-fixtures',
			)
		: legacyFixtures;
assert.ok(
	[
		'red-green.pcnd',
		'tag3-0.pcnd',
		'tag3-masked.pcnd',
		'source-all-six.pcnd',
	].includes(name),
);
const points = JSON.parse(process.env.POINT_UI_COORDINATES ?? '{}');
assert.ok(Object.keys(points).length <= 16);
for (const point of Object.values(points))
	assert.ok(
		Array.isArray(point) &&
			point.length === 2 &&
			point.every(Number.isFinite),
	);
fs.mkdirSync(process.env.CODEC_EVIDENCE_DIR, { recursive: true });
const out = fs.mkdtempSync(
	path.join(process.env.CODEC_EVIDENCE_DIR, 'attempt-'),
);
const assets = assetInventory(dist),
	source = fs.readFileSync(path.join(fixtures, name)),
	authored = project(source);
const evidence = {
	status: 'started',
	phase,
	kind,
	source: { name, sha256: sha(source) },
	points,
	assets,
	checks: [],
	startedAt: new Date().toISOString(),
};
const manifestPath = path.join(fixtures, 'oracle.json'),
	manifest = JSON.parse(fs.readFileSync(manifestPath));
const authoredFixture = manifest.fixtures.find(
	(row) => row.file === name && row.accepted,
);
assert.ok(authoredFixture);
assert.equal(sha(source), authoredFixture.sha256);
assert.deepEqual([...evaluated(authored)], authoredFixture.evaluated_rgba);
evidence.sources = {};
for (const file of [
	fileURLToPath(import.meta.url),
	path.join(app, 'scripts/lib/studio-filter-oracle.mjs'),
	path.join(app, 'scripts/lib/point-ui-bindings.mjs'),
	manifestPath,
])
	evidence.sources[path.relative(root, file)] = sha(fs.readFileSync(file));
fs.copyFileSync(
	fileURLToPath(import.meta.url),
	path.join(out, 'executed-driver.mjs'),
);
let server, browser, page;
try {
	server = http.createServer((request, response) => {
		const pathname = new URL(request.url, 'http://127.0.0.1').pathname,
			file = path.resolve(
				dist,
				`.${pathname === '/' ? '/index.html' : pathname}`,
			);
		if (
			!file.startsWith(`${dist}/`) ||
			!fs.existsSync(file) ||
			!fs.statSync(file).isFile()
		)
			return response.writeHead(404).end();
		response.setHeader(
			'Content-Type',
			{
				'.html': 'text/html',
				'.js': 'text/javascript',
				'.wasm': 'application/wasm',
				'.css': 'text/css',
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
	const url = `http://127.0.0.1:${server.address().port}/`;
	const settle = async () => {
		await page.waitForTimeout(400);
		await page.evaluate(
			() =>
				new Promise((resolve) =>
					requestAnimationFrame(() => requestAnimationFrame(resolve)),
				),
		);
	};
	const click = async (point) => {
		await page.mouse.click(...point);
		await settle();
	};
	const at = async (key) => {
		assert.ok(points[key], `Calibrate ${key}`);
		await click(points[key]);
	};
	const screenshot = async (label) =>
		page.screenshot({ path: path.join(out, `${label}.png`) });
	const sequence = () => page.evaluate(() => window.__pointSequence);
	const bind = async (since, bytes, state) => {
		const sourceSha256 = sha(bytes);
		await page.waitForFunction(
			({ since, sourceSha256, state }) =>
				window.__pointEvents.some(
					(e) =>
						e.sequence > since &&
						e.kind === 'open' &&
						e.plan?.sourceSha256 === sourceSha256 &&
						e.plan?.owner?.state === state,
				),
			{ since, sourceSha256, state },
			{ timeout: 60000 },
		);
		const captured = await page.evaluate(
			({ since, sourceSha256, state }) =>
				window.__pointEvents.find(
					(e) =>
						e.sequence > since &&
						e.kind === 'open' &&
						e.plan?.sourceSha256 === sourceSha256 &&
						e.plan?.owner?.state === state,
				),
			{ since, sourceSha256, state },
		);
		const plan = captured.plan;
		await page.waitForFunction(
			({ plan, workerId }) =>
				window.__pointEvents.some(
					(e) =>
						e.workerId === workerId &&
						e.kind === 'ready' &&
						JSON.stringify(window.__pointCanonical(e.binding)) ===
							JSON.stringify(window.__pointCanonical(plan)),
				),
			{ plan, workerId: captured.workerId },
			{ timeout: 60000 },
		);
		assert.equal(plan.document, authored.document);
		assert.deepEqual(plan.size, [2, 1]);
		assert.equal(plan.layers, 1);
		assert.equal(plan.owner.state, state);
		assert.equal(plan.owner.frontend, 3);
		if (kind === 'box') {
			assert.equal(plan.protocol, 3);
			assert.equal(
				plan.spatialCount,
				project(bytes)
					.layers.flatMap((l) => l.filters)
					.filter((f) => f.kind === 7).length,
			);
			assert.equal(plan.workspaceBytes, 64 * 1024 * 1024);
			assert.equal(plan.workLimit, 268435456);
		}
		assert.match(plan.owner.session, /^[0-9a-f-]{36}$/i);
		assert.match(plan.owner.epoch, /^(0|[1-9][0-9]*)$/);
		assert.match(plan.draft, /^[0-9a-f-]{36}$/i);
		return {
			owner: plan.owner,
			draft: plan.draft,
			sourceSha256,
			workerId: captured.workerId,
		};
	};
	const frame = async (pixels, since, binding, operation) => {
		const target = operation ? authored.layers[0].id : null,
			view = { region: [0, 0, 2, 1], level: 0 };
		const intended = {
			owner: binding.owner,
			draft: binding.draft,
			sourceSha256: binding.sourceSha256,
			target,
			operation,
			view,
			render: true,
		};
		await page.waitForFunction(
			({ since, intended, workerId }) =>
				window.__pointEvents.some((e) => {
					const r = e.request;
					if (!r || e.sequence <= since || e.workerId !== workerId)
						return false;
					const normalized = (v) =>
						Array.isArray(v)
							? v.map(normalized)
							: v && typeof v === 'object'
								? Object.fromEntries(
										Object.keys(v)
											.sort()
											.map((k) => [k, normalized(v[k])]),
									)
								: v;
					return (
						JSON.stringify(
							normalized({ ...r, generation: undefined }),
						) ===
						JSON.stringify(
							normalized({ ...intended, generation: undefined }),
						)
					);
				}),
			{ since, intended, workerId: binding.workerId },
			{ timeout: 60000 },
		);
		const events = await page.evaluate(() => window.__pointEvents);
		const request = events.findLast(
			(e) =>
				e.sequence > since &&
				e.workerId === binding.workerId &&
				e.request &&
				JSON.stringify(
					canonical({ ...e.request, generation: undefined }),
				) ===
					JSON.stringify(
						canonical({ ...intended, generation: undefined }),
					),
		).request;
		assert.match(request.generation, /^(0|[1-9][0-9]*)$/);
		const expected = {
			binding,
			operation,
			operationSha256: sha(
				Buffer.from(JSON.stringify(canonical(operation))),
			),
			generation: request.generation,
			pixels,
			since,
			target,
		};
		await page.waitForFunction(
			(expected) =>
				window.__pointMatchingPair(window.__pointEvents, expected),
			expected,
			{ timeout: 60000 },
		);
		const pair = await page.evaluate(
			(expected) =>
				window.__pointMatchingPair(window.__pointEvents, expected),
			expected,
		);
		assert.ok(pair);
		evidence.checks.push({
			kind: operation ? 'private-frame' : 'committed-base-frame',
			binding,
			operationSha256: expected.operationSha256,
			generation: expected.generation,
			target,
			pixels,
		});
		return pair;
	};
	const fresh = async () => {
		await page?.close();
		page = await browser.newPage({
			viewport: { width: 1360, height: 1000 },
			acceptDownloads: true,
		});
		await page.addInitScript({
			content: `window.__pointMatchingPair = ${matchingPointPair.toString()}; window.__pointCanonical = ${canonical.toString()};`,
		});
		await page.addInitScript(() => {
			localStorage.setItem('showWelcomeScreen', 'false');
			window.__pointEvents = [];
			window.__pointSequence = 0;
			window.__pointWorkerId = 0;
			const Original = window.Worker;
			window.Worker = class extends Original {
				constructor(...args) {
					super(...args);
					this.pointWorkerId = ++window.__pointWorkerId;
					this.addEventListener('message', ({ data }) => {
						if (window.__pointEvents.length < 1000)
							window.__pointEvents.push({
								sequence: ++window.__pointSequence,
								workerId: this.pointWorkerId,
								kind: data?.kind,
								binding: data?.binding,
								draft: data?.draft,
								view: data?.view,
								width: data?.width,
								height: data?.height,
								owner: data?.owner,
								generation: data?.generation,
								sourceSha256: data?.sourceSha256,
								operationSha256: data?.operationSha256,
								target: data?.target,
								region: data?.region,
								level: data?.level,
								pixels:
									data?.pixels instanceof Uint8Array &&
									data.pixels.length <= 4096
										? [...data.pixels]
										: undefined,
							});
					});
				}
				postMessage(value, transfer) {
					if (window.__pointEvents.length < 1000)
						window.__pointEvents.push({
							sequence: ++window.__pointSequence,
							workerId: this.pointWorkerId,
							request: value?.request,
							plan: value?.plan,
							kind: value?.kind,
						});
					return super.postMessage(value, transfer);
				}
			};
		});
		await page.goto(url, { waitUntil: 'networkidle' });
		await page.waitForFunction(
			() =>
				window.paintcraft && document.querySelector('#loading').hidden,
			null,
			{ timeout: 60000 },
		);
		await click(points.interface ?? [1273, 20]);
		await click(points.studio ?? [1160, 98]);
	};
	const open = async (file) => {
		const chooser = page.waitForEvent('filechooser');
		await click(points.file ?? [20, 46]);
		await click(points.open ?? [80, 94]);
		await (await chooser).setFiles(file);
		await settle();
	};
	const save = async (label) => {
		const pending = page.waitForEvent('download', { timeout: 15000 });
		await page.keyboard.press('Meta+s');
		const download = await pending;
		const file = path.join(out, `${label}.pcnd`);
		await download.saveAs(file);
		await settle();
		return fs.readFileSync(file);
	};
	const add = async () => {
		await at('add');
		await screenshot('add-menu');
		if (phase === 'menu') return false;
		await at('kind');
		await screenshot('parameters');
		return true;
	};
	const value = async () => {
		if (config.field) {
			assert.ok(points.value);
			await page.mouse.dblclick(...points.value);
			await page.keyboard.press('Meta+a');
			await page.keyboard.type(String(config.value));
			await page.keyboard.press('Enter');
			await settle();
		}
	};
	const candidate = (before, parameter) => {
		const doc = structuredClone(before),
			filter = { kind: config.kind, enabled: 1 };
		if (config.field) filter[config.field] = parameter;
		if (config.kind === 7) doc.version = 5;
		doc.layers[0].filters.push(filter);
		return [...evaluated(doc)];
	};
	await fresh();
	await open(path.join(fixtures, name));
	const before = await save('before'),
		beforeDoc = project(before),
		title = await page.title();
	assert.deepEqual(beforeDoc, authored);
	await screenshot('layout');
	const initialStamp = await sequence();
	const descriptor = (value) => ({
		kind: {
			3: 'black-white-rgb-v1',
			4: 'threshold-luma-v1',
			5: 'posterize-rgb-v1',
			6: 'solarize-rgb-v1',
			7: 'box-blur-v1',
		}[config.kind],
		...(config.field ? { [config.field]: value } : {}),
	});
	if (phase === 'layout' || !(await add()))
		evidence.status = 'calibration-only';
	else if (phase === 'parameters') evidence.status = 'calibration-only';
	else {
		const intended = (parameter) => ({
			op: 'filter.add',
			layer_ref: { id: authored.layers[0].id },
			ref: 'studio_filter',
			descriptor: descriptor(parameter),
			enabled: true,
		});
		let binding = await bind(initialStamp, before, beforeDoc.state);
		await frame(
			candidate(authored, config.initial),
			initialStamp,
			binding,
			intended(config.initial),
		);
		const defaultPixels = candidate(authored, config.initial);
		evidence.defaultCandidateCausal =
			JSON.stringify(defaultPixels) !==
			JSON.stringify(authoredFixture.evaluated_rgba);
		if (name === 'red-green.pcnd') {
			const independent =
				kind === 'box'
					? boxCase
					: manifest.cases.find((entry) => entry.kind === kind);
			assert.ok(independent);
			assert.deepEqual(defaultPixels, independent.default_rgba);
			assert.equal(
				evidence.defaultCandidateCausal,
				true,
				'Default changes the independent source',
			);
			assert.notDeepEqual(
				defaultPixels.slice(0, 3),
				defaultPixels.slice(4, 7),
			);
			await settle();
			await page.mouse.move(800, 300);
			evidence.defaultMarker = await canvasMarker(
				await page.screenshot(),
				[defaultPixels.slice(0, 3), defaultPixels.slice(4, 7)],
			);
			await screenshot('default-causal-preview');
		}
		let stamp = await sequence();
		await value();
		const expected = candidate(authored, config.value);
		if (name === 'red-green.pcnd')
			assert.deepEqual(
				expected,
				(kind === 'box'
					? boxCase
					: manifest.cases.find((entry) => entry.kind === kind)
				).changed_rgba,
			);
		await frame(
			expected,
			config.field ? stamp : initialStamp,
			binding,
			intended(config.value),
		);
		await screenshot('private');
		const colors = [expected.slice(0, 3), expected.slice(4, 7)];
		evidence.changedCandidateCausal =
			JSON.stringify(expected) !==
			JSON.stringify([...evaluated(authored)]);
		if (colors[0].join(',') !== colors[1].join(',')) {
			evidence.privateMarker = await canvasMarker(
				await page.screenshot(),
				colors,
			);
			evidence.appearanceScope =
				'Independent two-color screenshot geometry and exact Worker planes';
		} else
			evidence.appearanceScope =
				'Worker-only exact planes: uniform colors provide no two-color screenshot causality claim';
		await at('cancel');
		await settle();
		assert.deepEqual(await save('cancelled'), before);
		assert.equal(await page.title(), title);
		evidence.checks.push(
			'Cancel preserves byte-identical project/source/attachment/masks/title',
		);
		stamp = await sequence();
		await add();
		await value();
		binding = await bind(stamp, before, beforeDoc.state);
		await frame(expected, stamp, binding, intended(config.value));
		stamp = await sequence();
		await at('apply');
		await settle();
		const applied = await save('applied'),
			doc = project(applied),
			added = doc.layers[0].filters.at(-1);
		assert.notEqual(
			doc.state,
			beforeDoc.state,
			'Apply publishes exactly a new state',
		);
		assert.match(added.id, /^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i);
		assert.notEqual(added.id, '00000000-0000-0000-0000-000000000000');
		assert.ok(
			!authored.layers[0].filters.some(
				(filter) => filter.id === added.id,
			),
		);
		const expectedAdded = {
			id: added.id,
			enabled: 1,
			kind: config.kind,
			...(config.field ? { [config.field]: config.value } : {}),
			...(doc.version >= 4 ? { mask: null } : {}),
		};
		// Check the entire ordered stack before consulting any saved evaluation.
		assert.deepEqual(doc.layers[0].filters, [
			...authored.layers[0].filters.map((f) =>
				doc.version >= 4 ? { ...f, mask: f.mask ?? null } : f,
			),
			expectedAdded,
		]);
		assert.deepEqual(rawSemantics(doc), rawSemantics(authored));
		assert.deepEqual([...evaluated(doc)], expected);
		// Apply intentionally promotes the exact prepared private texture. It
		// need not open a second Worker just to render those same pixels again.
		if (colors[0].join(',') !== colors[1].join(',')) {
			const promoted = await canvasMarker(
				await page.screenshot(),
				colors,
			);
			assert.deepEqual(promoted, evidence.privateMarker);
			evidence.promotedMarker = promoted;
		}
		evidence.checks.push(
			'Apply publishes the exact intended stack and promotes its admitted private preview; fresh reopen separately verifies committed BASE',
		);
		const savedTitle = await page.title();
		assert.equal(savedTitle, `${name} - PaintCraft`);
		await page.keyboard.press('Meta+z');
		await settle();
		const undone = project(await save('undone'));
		assert.equal(undone.state, beforeDoc.state);
		assert.deepEqual(undone.layers[0].filters, authored.layers[0].filters);
		assert.deepEqual(rawSemantics(undone), rawSemantics(authored));
		await page.keyboard.press('Meta+Shift+z');
		await settle();
		const redone = project(await save('redone'));
		assert.equal(redone.state, doc.state);
		assert.deepEqual(redone, doc);
		evidence.firstPageEvents = await page.evaluate(
			() => window.__pointEvents,
		);
		await fresh();
		const reopenedStamp = await sequence();
		await open(path.join(out, 'applied.pcnd'));
		const reopened = await save('reopened');
		assert.deepEqual(reopened, applied);
		assert.equal(project(reopened).state, doc.state);
		assert.equal(await page.title(), 'applied.pcnd - PaintCraft');
		const committedBinding = await bind(reopenedStamp, reopened, doc.state);
		await frame(expected, reopenedStamp, committedBinding, null);
		evidence.checks.push(
			'Actual Add/change/Cancel/Apply/Undo/Redo/fresh reopen only; no existing Edit/duplicate/preview-off qualification',
		);
		evidence.status = 'passed';
	}

	assert.deepEqual(assetInventory(dist), assets);
	assert.equal(sha(fs.readFileSync(path.join(fixtures, name))), sha(source));
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
		if (page) {
			evidence.events = await page
				.evaluate(() => window.__pointEvents)
				.catch((error) => {
					cleanupError('observer', error);
					return [];
				});
			await page
				.screenshot({ path: path.join(out, 'terminal.png') })
				.catch((error) => cleanupError('screenshot', error));
		}
	} finally {
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
						checks: evidence.checks.length,
					}),
				);
			}
		}
	}
}
