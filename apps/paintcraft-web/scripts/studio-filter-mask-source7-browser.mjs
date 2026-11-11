// Real Normal filter-mask controls; immutable own-mask R and raw source are independently checked.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright-core';
import { fixture, assertSaved } from './lib/filter-mask-browser-fixtures.mjs';
import {
	inventory,
	serve,
	observeWorkers,
	healthy,
	sha,
} from './lib/main-blend-browser-driver.mjs';
import { observeMaskFrames } from './lib/layer-mask-browser-observer.mjs';

assert(process.env.CODEC_EVIDENCE_DIR, 'Explicit CODEC_EVIDENCE_DIR required');
const app = fileURLToPath(new URL('../', import.meta.url));
const dist = path.join(app, 'dist');
fs.mkdirSync(process.env.CODEC_EVIDENCE_DIR, { recursive: true });
const out = fs.mkdtempSync(
	path.join(process.env.CODEC_EVIDENCE_DIR, 'filter-mask-'),
);
const input = fixture();
const points = {
	interface: [1270, 20],
	studio: [1158, 96],
	file: [20, 46],
	open: [76, 101],
	select: [34, 207],
	start: [671, 561],
	end: [671, 561],
	filterActions: [1210, 613],
	useSelection: [1244, 724],
	enable: [1244, 748],
	remove: [1244, 772],
};
const supplied = process.env.FILTER_MASK_COORDINATES ?? '{}';
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
for (const key of ['filterActions', 'useSelection', 'enable', 'remove'])
	assert(
		points[key].some((n) => n !== 0),
		'Uncalibrated placeholder: ' + key,
	);
const result = {
	status: 'FAILED',
	scope: 'Finite real Normal filter-mask actions only; not NonNormal editing, own-mask mutation or physical resource qualification',
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
			'scripts/studio-filter-mask-source7-browser.mjs',
			'scripts/lib/layer-mask-browser-observer.mjs',
			'scripts/lib/filter-mask-browser-fixtures.mjs',
			'scripts/lib/main-blend-browser-driver.mjs',
			'scripts/fixtures/studio-layer-mask-saved-vectors.json',
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
async function saved(label, mask, dirty) {
	const pending = page.waitForEvent('download', { timeout: 30000 });
	await page.keyboard.press(command + '+s');
	const download = await pending;
	const file = path.join(out, label + '.pcnd');
	await download.saveAs(file);
	assert(fs.statSync(file).size <= 4096);
	const bytes = fs.readFileSync(file);
	const inspected = assertSaved(bytes, { mask });
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
async function maskAction(key) {
	await click('filterActions');
	await screenshot('menu-' + key + '-' + ++menuSerial);
	await click(key);
}
let menuSerial = 0;
function preserved(state, baseline) {
	assert.deepEqual(state.rawIdentity, baseline.rawIdentity);
	assert.deepEqual(state.ownMask, baseline.ownMask);
	assert.equal(state.filters[0].id, baseline.filters[0].id);
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
		name: 'filter-mask.pcnd',
		mimeType: 'application/octet-stream',
		buffer: input.bytes,
	});
	await page.waitForFunction(
		() => document.title === 'filter-mask.pcnd - PaintCraft',
		{},
		{ timeout: 30000 },
	);
	await tick();
	await page.keyboard.press(command + '+b');
	await tick();
	let baseline, selected, disabled, enabled;
	await group(
		'canonical Invert plus own-mask R opens/saves without mutation',
		async () => {
			baseline = await saved('baseline', 'none', false);
			assert.equal(baseline.savedSha256, input.sha256);
			await frame(0, baseline);
			await screenshot('baseline');
			return { state: baseline.state, filter: baseline.filters[0].id };
		},
	);
	await group(
		'filter-row Use Selection masks only x1, preserving own mask R',
		async () => {
			await click('select');
			await page.mouse.move(...points.start);
			await page.mouse.down();
			await tick();
			await page.mouse.move(...points.end, { steps: 4 });
			await tick();
			await page.mouse.up();
			await tick();
			await screenshot('selection');
			const since = await sequence();
			await maskAction('useSelection');
			selected = await saved('created', 'selection', true);
			assert.notEqual(selected.state, baseline.state);
			preserved(selected, baseline);
			return {
				frame: await frame(since, selected),
				state: selected.state,
			};
		},
	);
	await group(
		'same selection is no-op; single Undo/Redo exactly restores source',
		async () => {
			const before = await trace();
			await maskAction('useSelection');
			assert.deepEqual(await saved('noop', 'selection', true), selected);
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
			let since = await sequence();
			await page.keyboard.press(command + '+z');
			await tick();
			assert.deepEqual(
				await saved('undo-create', 'none', false),
				baseline,
			);
			await frame(since, baseline);
			since = await sequence();
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
		'disable/re-enable changes filter coverage only with exact Undo/Redo',
		async () => {
			let since = await sequence();
			await maskAction('enable');
			disabled = await saved('disabled', 'disabled', true);
			preserved(disabled, baseline);
			assert.notEqual(disabled.state, selected.state);
			await frame(since, disabled);
			since = await sequence();
			await maskAction('enable');
			enabled = await saved('enabled', 'selection', true);
			preserved(enabled, baseline);
			assert.notEqual(enabled.state, disabled.state);
			await frame(since, enabled);
			since = await sequence();
			await page.keyboard.press(command + '+z');
			await tick();
			assert.deepEqual(
				await saved('undo-enable', 'disabled', true),
				disabled,
			);
			await frame(since, disabled);
			since = await sequence();
			await page.keyboard.press(command + '+y');
			await tick();
			assert.deepEqual(
				await saved('redo-enable', 'selection', true),
				enabled,
			);
			return { frame: await frame(since, enabled), state: enabled.state };
		},
	);
	await group(
		'Remove mask preserves Invert ID and own R; Undo/Redo restore exact sources',
		async () => {
			let since = await sequence();
			await maskAction('remove');
			const removed = await saved('removed', 'none', true);
			preserved(removed, baseline);
			assert.notEqual(removed.state, enabled.state);
			await frame(since, removed);
			since = await sequence();
			await page.keyboard.press(command + '+z');
			await tick();
			assert.deepEqual(
				await saved('undo-remove', 'selection', true),
				enabled,
			);
			await frame(since, enabled);
			since = await sequence();
			await page.keyboard.press(command + '+y');
			await tick();
			assert.deepEqual(await saved('redo-remove', 'none', true), removed);
			return { frame: await frame(since, removed), state: removed.state };
		},
	);
	assert.equal(result.cases.length, 5);
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
if (result.status !== 'PASS')
	throw Error(failure ?? 'filter-mask acceptance failed');
console.log(
	JSON.stringify({
		status: result.status,
		cases: result.cases.length,
		evidence: out,
	}),
);
