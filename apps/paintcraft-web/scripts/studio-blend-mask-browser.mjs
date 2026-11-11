// Finite genuine Main Source8 mask mutations; no injected application authority.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright-core';
import { fixture, assertSaved } from './lib/blend-mask-browser-fixtures.mjs';
import {
	inventory,
	serve,
	observeWorkers,
	healthy,
	assertFrame,
	openEvent,
	completed,
	sha,
} from './lib/main-blend-browser-driver.mjs';

assert(process.env.CODEC_EVIDENCE_DIR, 'Explicit CODEC_EVIDENCE_DIR required');
const app = fileURLToPath(new URL('../', import.meta.url));
const dist = path.join(app, 'dist');
fs.mkdirSync(process.env.CODEC_EVIDENCE_DIR, { recursive: true });
const out = fs.mkdtempSync(
	path.join(process.env.CODEC_EVIDENCE_DIR, 'blend-mask-'),
);
const specimen = fixture();
const points = {
	interface: [1270, 20],
	studio: [1158, 96],
	file: [20, 46],
	open: [76, 101],
	topLayer: [1220, 510],
	select: [34, 207],
	start: [684, 568],
	end: [687, 571],
	useSelection: [1190, 644],
	invert: [1180, 698],
	enable: [1180, 672],
	remove: [1180, 726],
};
const supplied = process.env.BLEND_MASK_COORDINATES ?? '{}';
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
	scope: 'Finite two-pixel Multiply own-mask edits using genuine original Main/Worker callbacks; no private filter/derived/physical resource qualification',
	points,
	cases: [],
	pageErrors: [],
	requests: [],
	httpErrors: [],
	components: {},
};
const save = (name, value) =>
	fs.writeFileSync(
		path.join(out, name),
		JSON.stringify(value, null, 2) + '\n',
		{ flag: 'wx' },
	);
const command = process.platform === 'darwin' ? 'Meta' : 'Control';
let browser, server, page, assets, sourceBefore, failure;
function sources() {
	return Object.fromEntries(
		[
			'scripts/studio-blend-mask-browser.mjs',
			'scripts/lib/blend-mask-browser-fixtures.mjs',
			'scripts/lib/main-blend-browser-fixtures.mjs',
			'scripts/lib/main-blend-browser-driver.mjs',
			'src/paintcraft-render-worker/tests/source8-service-fixtures/oracle.json',
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
	await page.screenshot({ path: path.join(out, label + '.png') });
}
async function saved(label, mask, dirty) {
	const pending = page.waitForEvent('download', { timeout: 30000 });
	await page.keyboard.press(command + '+s');
	const download = await pending,
		file = path.join(out, label + '.pcnd');
	await download.saveAs(file);
	assert(fs.statSync(file).size <= 1024);
	const bytes = fs.readFileSync(file),
		state = assertSaved(bytes, { mask });
	state.bytes = bytes;
	state.sha256 = sha(bytes);
	state.canvas = [2, 1];
	await tick();
	assert.equal(
		await page.evaluate(() => window.paintcraft.is_dirty()),
		dirty,
	);
	save(label + '-semantics.json', state);
	return state;
}
async function frame(since, state, previous) {
	await page.waitForFunction(
		({ since, hash }) =>
			window.__mainBlendSmoke.workers.some((w) => {
				const open = w.events.find(
					(e) =>
						e.direction === 'request' &&
						e.protocol === 8 &&
						e.kind === 'open',
				);
				const view = w.events
					.filter(
						(e) =>
							e.direction === 'request' &&
							e.protocol === 8 &&
							e.kind === 'view',
					)
					.at(-1);
				return (
					open?.encodedSha256 === hash &&
					view &&
					w.events.some(
						(e) =>
							e.direction === 'reply' &&
							e.protocol === 8 &&
							e.kind === 'opened' &&
							e.sequence > since,
					) &&
					w.events.some(
						(e) =>
							e.direction === 'reply' &&
							e.protocol === 8 &&
							e.kind === 'complete' &&
							e.id === view.id &&
							e.token === view.token,
					)
				);
			}),
		{ since, hash: state.sha256 },
		{ timeout: 60000 },
	);
	await tick();
	const q = await trace();
	healthy(q);
	const worker = q.workers.find(
		(w) =>
			completed(w) &&
			openEvent(w)?.encodedSha256 === state.sha256 &&
			w.events.some(
				(e) =>
					e.direction === 'reply' &&
					e.protocol === 8 &&
					e.kind === 'opened' &&
					e.sequence > since,
			),
	);
	assert(worker);
	const pixels = assertFrame(worker, state);
	if (previous !== undefined) {
		const old = q.workers[previous];
		const closed = old.events.find(
			(e) =>
				e.direction === 'reply' &&
				e.protocol === 8 &&
				e.kind === 'closed',
		);
		assert(closed && closed.sequence < worker.created);
		assert(old.terminated !== null && old.terminated < worker.created);
	}
	return {
		worker: worker.index,
		pixels,
		previousClosedBeforeNewWorker: previous !== undefined,
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
let current;
async function change(key, label, mask) {
	const since = await sequence(),
		old = current;
	await click(key);
	const state = await saved(label, mask, true);
	const rendered = await frame(since, state, old);
	current = rendered.worker;
	assert.deepEqual(state.rawIdentity, specimen.rawIdentity);
	return { state, rendered };
}
async function history(key, label, expected, mask, dirty) {
	const since = await sequence(),
		old = current;
	await page.keyboard.press(command + '+' + key);
	await tick();
	const state = await saved(label, mask, dirty);
	assert.deepEqual(state, expected);
	const rendered = await frame(since, state, old);
	current = rendered.worker;
	return rendered;
}
try {
	assets = inventory(dist);
	sourceBefore = sources();
	for (const [name, generated] of [
		['main', 'pkg/paintcraft_web_bg.wasm'],
		['worker', 'render-worker-pkg/paintcraft_web_render_worker_bg.wasm'],
	]) {
		const digest = sha(fs.readFileSync(path.join(app, generated)));
		const matches = Object.entries(assets).filter(
			([file, row]) => file.endsWith('.wasm') && row.sha256 === digest,
		);
		assert.equal(matches.length, 1);
		result.components[name] = {
			generated,
			sha256: digest,
			asset: matches[0][0],
		};
	}
	save('inputs.json', {
		assets,
		sources: sourceBefore,
		components: result.components,
		points,
	});
	server = serve(dist, assets, result.requests, result.httpErrors);
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
		else failure ??= 'diagnostic bound';
	});
	await page.addInitScript({
		content: `(${observeWorkers.toString()})();localStorage.setItem('showWelcomeScreen','false');`,
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
	const sinceOpen = await sequence(),
		chooser = page.waitForEvent('filechooser', { timeout: 15000 });
	await click('file');
	await click('open');
	await (await chooser).setFiles({
		name: 'multiply-mask.pcnd',
		mimeType: 'application/octet-stream',
		buffer: specimen.bytes,
	});
	await page.waitForFunction(
		() => document.title === 'multiply-mask.pcnd - PaintCraft',
		{},
		{ timeout: 30000 },
	);
	let baseline, right, left, disabled;
	await group('real Multiply source opens and saves exactly', async () => {
		baseline = await saved('baseline', 'none', false);
		assert.equal(baseline.sha256, specimen.sha256);
		const rendered = await frame(sinceOpen, baseline);
		current = rendered.worker;
		await click('topLayer');
		await page.keyboard.press(command + '+b');
		await tick();
		await screenshot('baseline');
		return rendered;
	});
	await group(
		'real right-pixel own mask reopens Source8 after consuming close',
		async () => {
			await click('select');
			await page.mouse.move(...points.start);
			await page.mouse.down();
			await page.mouse.move(...points.end, { steps: 3 });
			await page.mouse.up();
			await tick();
			await screenshot('selection-before-mask');
			const changed = await change('useSelection', 'right', 'right');
			right = changed.state;
			await screenshot('right-mask');
			return changed.rendered;
		},
	);
	await group(
		'same mask retains source and one Undo/Redo restores exact documents',
		async () => {
			const before = await trace();
			await click('useSelection');
			assert.deepEqual(await saved('right-noop', 'right', true), right);
			for (let i = 0; i < 12; i++) await tick();
			const after = await trace();
			healthy(after);
			assert.equal(after.workers.length, before.workers.length);
			const opens = (q) =>
				q.workers
					.flatMap((w) => w.events)
					.filter(
						(e) =>
							e.direction === 'request' &&
							e.protocol === 8 &&
							e.kind === 'open',
					).length;
			assert.equal(opens(after), opens(before));
			await history('z', 'undo-create', baseline, 'none', false);
			return {
				redo: await history('y', 'redo-create', right, 'right', true),
				noNewOpenOnNoop: true,
			};
		},
	);
	await group(
		'Invert and disable preserve both raw layers and exact alpha',
		async () => {
			const a = await change('invert', 'left', 'left');
			left = a.state;
			const b = await change('enable', 'disabled', 'disabled');
			disabled = b.state;
			await history('z', 'undo-disable', left, 'left', true);
			await history('y', 'redo-disable', disabled, 'disabled', true);
			return { inverse: a.rendered, disabled: b.rendered };
		},
	);
	await group(
		'Remove and Undo retain Multiply and disabled A8 without fallback',
		async () => {
			const removed = await change('remove', 'removed', 'none');
			await screenshot('removed');
			return {
				removed: removed.rendered,
				undo: await history(
					'z',
					'undo-remove',
					disabled,
					'disabled',
					true,
				),
			};
		},
	);
	assert.equal(result.cases.length, 5);
	assert(result.cases.every((row) => row.status === 'PASS'));
	const finalTrace = await trace();
	healthy(finalTrace);
	const sourceWorkers = finalTrace.workers.filter((worker) =>
		openEvent(worker),
	);
	for (let i = 1; i < sourceWorkers.length; i++) {
		const previous = sourceWorkers[i - 1],
			next = sourceWorkers[i];
		const closed = previous.events.find(
			(event) =>
				event.direction === 'reply' &&
				event.protocol === 8 &&
				event.kind === 'closed',
		);
		assert(closed && closed.sequence < next.created);
		assert(
			previous.terminated !== null && previous.terminated < next.created,
		);
	}
	result.joinedHandoffs = sourceWorkers.length - 1;
	assert.deepEqual(result.pageErrors, []);
	assert.deepEqual(result.httpErrors, []);
	for (const component of Object.values(result.components))
		assert(
			result.requests.some(
				(r) =>
					r.file === component.asset && r.sha256 === component.sha256,
			),
		);
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
		if (assets) assert.deepEqual(inventory(dist), assets, 'dist changed');
		if (sourceBefore)
			assert.deepEqual(sources(), sourceBefore, 'test source changed');
		for (const c of Object.values(result.components))
			assert.equal(
				sha(fs.readFileSync(path.join(app, c.generated))),
				c.sha256,
			);
	} catch (error) {
		failure ??= String(error);
	}
	if (failure) result.status = 'FAILED';
	result.error = failure ?? null;
	save('final.json', result);
}
if (result.status !== 'PASS')
	throw Error(failure ?? 'blend mask acceptance failed');
console.log(
	JSON.stringify({
		status: result.status,
		cases: result.cases.length,
		evidence: out,
	}),
);
