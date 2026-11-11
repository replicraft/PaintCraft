// Real Normal-only own-mask controls; no document mutation through test hooks.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright-core';
import { fixture, assertSaved } from './lib/layer-mask-browser-fixtures.mjs';
import {
	inventory,
	serve,
	observeWorkers,
	healthy,
	opaqueMarker,
	sha,
} from './lib/main-blend-browser-driver.mjs';
import { observeMaskFrames } from './lib/layer-mask-browser-observer.mjs';

assert(process.env.CODEC_EVIDENCE_DIR, 'Explicit CODEC_EVIDENCE_DIR required');
const app = fileURLToPath(new URL('../', import.meta.url));
const dist = path.join(app, 'dist');
fs.mkdirSync(process.env.CODEC_EVIDENCE_DIR, { recursive: true });
const out = fs.mkdtempSync(
	path.join(process.env.CODEC_EVIDENCE_DIR, 'layer-mask-'),
);
const input = fixture();
const points = {
	interface: [1270, 20],
	studio: [1158, 96],
	file: [20, 46],
	open: [76, 101],
	select: [34, 207],
	start: [671, 561],
	end: [681, 561],
	useSelection: [1190, 644],
	enable: [1170, 672],
	invert: [1170, 698],
	remove: [1170, 726],
	canvas: [660, 535],
	selectedCanvas: [660, 560],
	addFilter: [1175, 568],
	invertOption: [1180, 592],
	applyFilter: [404, 143],
	cancelFilter: [455, 143],
};
const supplied = process.env.LAYER_MASK_COORDINATES ?? '{}';
assert(Buffer.byteLength(supplied) <= 4096);
const overrides = JSON.parse(supplied);
assert(overrides && Object.getPrototypeOf(overrides) === Object.prototype);
for (const [key, value] of Object.entries(overrides)) {
	assert(Object.hasOwn(points, key));
	points[key] = value;
}
for (const point of Object.values(points)) {
	assert(Array.isArray(point) && point.length === 2);
	assert(
		point.every(
			(n, i) =>
				Number.isSafeInteger(n) && n >= 0 && n < (i ? 1000 : 1360),
		),
	);
}
const result = {
	status: 'FAILED',
	scope: 'Finite real Normal own-layer-mask actions only; not nonNormal editing or physical resource qualification',
	points,
	input: { sha256: input.sha256 },
	cases: [],
	screenshots: [],
	pageErrors: [],
	requests: [],
	httpErrors: [],
};
const save = (name, value) =>
	fs.writeFileSync(
		path.join(out, name),
		JSON.stringify(value, null, 2) + '\n',
		{ flag: 'wx' },
	);
const command = process.platform === 'darwin' ? 'Meta' : 'Control';
let browser, server, page, beforeAssets, sourceBefore, failure;
function sources() {
	return Object.fromEntries(
		[
			'scripts/studio-layer-mask-browser.mjs',
			'scripts/lib/layer-mask-browser-observer.mjs',
			'scripts/lib/layer-mask-browser-fixtures.mjs',
			'scripts/lib/main-blend-browser-driver.mjs',
			'scripts/fixtures/studio-layer-mask-normal.pcnd',
		].map((name) => [name, sha(fs.readFileSync(path.join(app, name)))]),
	);
}
const trace = () => page.evaluate(() => window.__mainBlendSmoke);
const sequence = () => page.evaluate(() => window.__mainBlendSmoke.sequence);
async function tick() {
	await page.evaluate(
		() =>
			new Promise((resolve) =>
				requestAnimationFrame(() => requestAnimationFrame(resolve)),
			),
	);
}
async function click(key) {
	await page.mouse.click(...points[key]);
	await tick();
}
async function screenshot(label) {
	await page.mouse.move(1000, 900);
	await tick();
	const bytes = await page.screenshot();
	fs.writeFileSync(path.join(out, label + '.png'), bytes, { flag: 'wx' });
	result.screenshots.push({ file: label + '.png', sha256: sha(bytes) });
	return bytes;
}
async function saved(label, mask, dirty, invert = false) {
	const pending = page.waitForEvent('download', { timeout: 30000 });
	await page.keyboard.press(command + '+s');
	const download = await pending;
	const file = path.join(out, label + '.pcnd');
	await download.saveAs(file);
	assert(fs.statSync(file).size <= 4096);
	const bytes = fs.readFileSync(file);
	const inspected = assertSaved(bytes, { mask, invert });
	inspected.savedSha256 = sha(bytes);
	await tick();
	assert.equal(
		await page.evaluate(() => window.paintcraft.is_dirty()),
		dirty,
	);
	save(label + '-semantics.json', inspected);
	return inspected;
}
async function frame(since, state) {
	const rgba = state.rgba;
	await page.waitForFunction(
		({ since, hex, sourceSha256 }) => {
			const frames = window.__mainBlendSmoke.workers
				.flatMap((w) => w.events)
				.filter(
					(e) =>
						e.direction === 'reply' &&
						e.protocol === 7 &&
						e.kind === 'frame' &&
						e.sequence > since,
				);
			frames.sort((a, b) => a.sequence - b.sequence);
			return (
				frames.at(-1)?.pixelsHex === hex &&
				frames.at(-1)?.sourceSha256 === sourceSha256
			);
		},
		{ since, hex: rgba.toString('hex'), sourceSha256: state.savedSha256 },
		{ timeout: 60000 },
	);
	await tick();
	const q = await trace();
	healthy(q);
	const latest = q.workers
		.flatMap((w) => w.events)
		.filter(
			(e) => e.protocol === 7 && e.kind === 'frame' && e.sequence > since,
		)
		.sort((a, b) => a.sequence - b.sequence)
		.at(-1);
	assert(latest);
	assert.deepEqual([latest.width, latest.height, latest.level], [4, 2, 0]);
	assert.deepEqual(latest.region, [0, 0, 4, 2]);
	assert.equal(latest.sourceSha256, state.savedSha256);
	assert.deepEqual(Buffer.from(latest.pixelsHex, 'hex'), rgba);
	return {
		sequence: latest.sequence,
		sourceSha256: latest.sourceSha256,
		rgbaSha256: sha(rgba),
	};
}
async function opaqueCanvas(label, selectionToolbar = true) {
	const bytes = await screenshot(label);
	const origin = selectionToolbar ? points.selectedCanvas : points.canvas;
	for (let y = 0; y < 2; y++)
		for (let x = 0; x < 4; x++) {
			const left = origin[0] + x * 10 + 3;
			const top = origin[1] + y * 10 + 3;
			opaqueMarker(bytes, [left, top, left + 4, top + 4], [64, 128, 192]);
		}
}
async function group(name, fn) {
	const row = { name, status: 'FAILED' };
	try {
		Object.assign(row, await fn());
		row.status = 'PASS';
	} finally {
		result.cases.push(row);
		save('case-' + result.cases.length + '.json', row);
	}
}
async function change(key, label, mask) {
	const since = await sequence();
	await click(key);
	const state = await saved(label, mask, true);
	return { state, frame: await frame(since, state) };
}
try {
	beforeAssets = inventory(dist);
	sourceBefore = sources();
	save('inputs.json', {
		assets: beforeAssets,
		sources: sourceBefore,
		points,
		input: result.input,
	});
	server = serve(dist, beforeAssets, result.requests, result.httpErrors);
	await new Promise((resolve, reject) => {
		server.once('error', reject);
		server.listen(0, '127.0.0.1', resolve);
	});
	browser = await chromium.launch({
		channel: 'chrome',
		headless: true,
		args: ['--enable-unsafe-swiftshader'],
	});
	result.browser = browser.version();
	page = await browser.newPage({
		viewport: { width: 1360, height: 1000 },
		deviceScaleFactor: 1,
		locale: 'en-US',
		acceptDownloads: true,
	});
	page.on('pageerror', (error) => {
		if (result.pageErrors.length < 32)
			result.pageErrors.push(String(error));
		else failure ??= 'page diagnostic bound';
	});
	await page.addInitScript({
		content: `(${observeWorkers.toString()})();(${observeMaskFrames.toString()})();localStorage.setItem('showWelcomeScreen','false');`,
	});
	await page.goto(`http://127.0.0.1:${server.address().port}/`, {
		waitUntil: 'networkidle',
	});
	await page.waitForFunction(
		() => window.paintcraft && document.querySelector('#loading')?.hidden,
		{},
		{ timeout: 60000 },
	);
	await click('interface');
	await click('studio');
	const chooser = page.waitForEvent('filechooser', { timeout: 15000 });
	await click('file');
	await click('open');
	await (await chooser).setFiles({
		name: 'normal-mask.pcnd',
		mimeType: 'application/octet-stream',
		buffer: input.bytes,
	});
	await page.waitForFunction(
		() => document.title === 'normal-mask.pcnd - PaintCraft',
		{},
		{ timeout: 30000 },
	);
	await tick();
	await page.keyboard.press(command + '+b');
	await tick();
	let baseline, selected, inverted, disabled;
	await group('unchanged Normal source opens and saves exactly', async () => {
		baseline = await saved('baseline', 'none', false);
		assert.equal(baseline.savedSha256, input.sha256);
		await opaqueCanvas('baseline', false);
		return { state: baseline.state, rawIdentity: baseline.rawIdentity };
	});
	await group(
		'real rectangular selection publishes a saved own mask and Source7 frame',
		async () => {
			await click('select');
			await page.mouse.move(...points.start);
			await page.mouse.down();
			await page.mouse.move(...points.end, { steps: 4 });
			await page.mouse.up();
			await tick();
			await screenshot('selection-before-mask');
			const changed = await change(
				'useSelection',
				'selected',
				'selection',
			);
			selected = changed.state;
			assert.notEqual(selected.state, baseline.state);
			await screenshot('mask-created');
			return { state: selected.state, frame: changed.frame };
		},
	);
	await group(
		'repeated mask is a semantic no-op and a single Undo restores absence',
		async () => {
			const before = await trace();
			await click('useSelection');
			const same = await saved('selected-noop', 'selection', true);
			assert.deepEqual(same, selected);
			for (let i = 0; i < 12; i++) await tick();
			const after = await trace();
			healthy(after);
			assert.equal(after.workers.length, before.workers.length);
			const opens = (q) =>
				q.workers
					.flatMap((w) => w.events)
					.filter(
						(e) => e.direction === 'request' && e.kind === 'open',
					).length;
			assert.equal(opens(after), opens(before));
			await page.keyboard.press(command + '+z');
			await tick();
			assert.deepEqual(
				await saved('undo-create', 'none', false),
				baseline,
			);
			await opaqueCanvas('undo-create');
			const since = await sequence();
			await page.keyboard.press(command + '+y');
			await tick();
			assert.deepEqual(
				await saved('redo-create', 'selection', true),
				selected,
			);
			return {
				frame: await frame(since, selected),
				noNewSourceOnNoop: true,
			};
		},
	);
	await group(
		'Invert and disable preserve exact independent coverage and source',
		async () => {
			const a = await change('invert', 'inverted', 'inverse');
			inverted = a.state;
			const b = await change('enable', 'disabled', 'disabled');
			disabled = b.state;
			assert.notEqual(disabled.state, inverted.state);
			assert.deepEqual(disabled.rawIdentity, baseline.rawIdentity);
			await screenshot('mask-disabled');
			return { inverted: a.frame, disabled: b.frame };
		},
	);
	await group(
		'Remove preserves raster and one-step Undo restores disabled mask',
		async () => {
			await click('remove');
			const removed = await saved('removed', 'none', true);
			assert.deepEqual(removed.rawIdentity, baseline.rawIdentity);
			await opaqueCanvas('mask-removed');
			const since = await sequence();
			await page.keyboard.press(command + '+z');
			await tick();
			assert.deepEqual(
				await saved('undo-remove', 'disabled', true),
				disabled,
			);
			return {
				frame: await frame(since, disabled),
				removedState: removed.state,
			};
		},
	);
	await group(
		'masked Invert preview Cancel/Apply/Undo preserve raw source and mask',
		async () => {
			let since = await sequence();
			await page.keyboard.press(command + '+z');
			await tick();
			assert.deepEqual(
				await saved('restore-inverse', 'inverse', true),
				inverted,
			);
			await frame(since, inverted);
			since = await sequence();
			await page.keyboard.press(command + '+z');
			await tick();
			assert.deepEqual(
				await saved('restore-selection', 'selection', true),
				selected,
			);
			await frame(since, selected);
			const preview = {
				...selected,
				rgba: Buffer.from([
					0, 0, 0, 0, 191, 127, 63, 255, 191, 127, 63, 255, 0, 0, 0,
					0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
				]),
			};
			since = await sequence();
			await click('addFilter');
			await click('invertOption');
			await frame(since, preview);
			await screenshot('invert-preview');
			// Invert would change coverage if the private-input guard admitted it.
			await click('invert');
			await click('cancelFilter');
			assert.deepEqual(
				await saved('cancel-invert', 'selection', true),
				selected,
			);
			since = await sequence();
			await click('addFilter');
			await click('invertOption');
			await frame(since, preview);
			since = await sequence();
			const privateWorker = (await trace()).workers.at(-1).index;
			await click('applyFilter');
			await page.waitForFunction(
				({ since, privateWorker }) =>
					window.__mainBlendSmoke.workers[privateWorker].terminated >
					since,
				{ since, privateWorker },
				{ timeout: 60000 },
			);
			await tick();
			const applied = await saved(
				'applied-invert',
				'selection',
				true,
				true,
			);
			assert.notEqual(applied.state, selected.state);
			assert.deepEqual(applied.mask, selected.mask);
			assert.deepEqual(applied.rawIdentity, baseline.rawIdentity);
			const image = await screenshot('invert-applied');
			for (const x of [1, 2]) {
				const left = points.selectedCanvas[0] + x * 10 + 3;
				const top = points.selectedCanvas[1] + 3;
				opaqueMarker(
					image,
					[left, top, left + 4, top + 4],
					[191, 127, 63],
				);
			}
			const publication = {
				reusedPreviewSourceSha256: selected.savedSha256,
				savedProjectSha256: applied.savedSha256,
				opaqueCoveredPixels: 2,
			};
			since = await sequence();
			await page.keyboard.press(command + '+z');
			await tick();
			assert.deepEqual(
				await saved('undo-invert-filter', 'selection', true),
				selected,
			);
			await frame(since, selected);
			since = await sequence();
			await page.keyboard.press(command + '+y');
			await tick();
			assert.deepEqual(
				await saved('redo-invert-filter', 'selection', true, true),
				applied,
			);
			await frame(since, applied);
			return {
				publication,
				filter: applied.filters[0].id,
				cancellationPreservedState: true,
			};
		},
	);
	assert.equal(result.cases.length, 6);
	assert(result.cases.every((row) => row.status === 'PASS'));
	healthy(await trace());
	assert.deepEqual(result.pageErrors, []);
	assert.deepEqual(result.httpErrors, []);
	result.status = 'PASS';
} catch (error) {
	failure = String(error?.stack ?? error);
} finally {
	try {
		if (page) {
			save('trace.json', await trace());
			if (failure) await screenshot('failure');
		}
	} catch (error) {
		failure ??= String(error);
	}
	try {
		if (browser) await browser.close();
	} catch (error) {
		failure ??= String(error);
	}
	try {
		if (server) await new Promise((resolve) => server.close(resolve));
	} catch (error) {
		failure ??= String(error);
	}
	try {
		if (beforeAssets)
			assert.deepEqual(inventory(dist), beforeAssets, 'dist changed');
		if (sourceBefore)
			assert.deepEqual(sources(), sourceBefore, 'test sources changed');
	} catch (error) {
		failure ??= String(error);
	}
	if (failure) result.status = 'FAILED';
	result.error = failure ?? null;
	save('final.json', result);
}
if (result.status !== 'PASS') throw Error(failure ?? 'mask acceptance failed');
console.log(
	JSON.stringify({
		status: result.status,
		cases: result.cases.length,
		evidence: out,
	}),
);
