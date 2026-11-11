// Finite real Main A/B smoke. The full Source8 qualification remains separate.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { fixture, assertSaved } from './lib/main-blend-browser-fixtures.mjs';
import {
	sha,
	inventory,
	serve,
	observeWorkers,
	healthy,
	openEvent,
	completed,
	assertFrame,
	opaqueMarker,
} from './lib/main-blend-browser-driver.mjs';

assert(
	process.env.CODEC_EVIDENCE_DIR,
	'Explicit CODEC_EVIDENCE_DIR is required',
);
const app = fileURLToPath(new URL('../', import.meta.url));
const root = path.resolve(app, '../..'),
	dist = path.join(app, 'dist');
const points = {
	interface: [1270, 20],
	studio: [1158, 96],
	file: [20, 46],
	open: [76, 101],
	canvasClip: [400, 300, 960, 750],
};
assert(Buffer.byteLength(process.env.MAIN_BLEND_COORDINATES ?? '{}') <= 4096);
const supplied = JSON.parse(process.env.MAIN_BLEND_COORDINATES ?? '{}');
assert(supplied && Object.getPrototypeOf(supplied) === Object.prototype);
for (const [key, value] of Object.entries(supplied)) {
	assert(Object.hasOwn(points, key), 'unknown coordinate key');
	points[key] = value;
}
for (const [key, value] of Object.entries(points)) {
	assert(
		Array.isArray(value) && value.length === (key === 'canvasClip' ? 4 : 2),
	);
	assert(
		value.every(
			(n, i) =>
				Number.isInteger(n) && n >= 0 && n < (i % 2 ? 1000 : 1360),
		),
	);
}
assert(
	points.canvasClip[2] > points.canvasClip[0] &&
		points.canvasClip[3] > points.canvasClip[1],
);
fs.mkdirSync(process.env.CODEC_EVIDENCE_DIR, { recursive: true });
const out = fs.mkdtempSync(
	path.join(process.env.CODEC_EVIDENCE_DIR, 'main-blend-'),
);
const save = (name, value) =>
	fs.writeFileSync(
		path.join(out, name),
		JSON.stringify(value, null, 2) + '\n',
		{ flag: 'wx' },
	);
const evidence = {
	status: 'FAILED',
	scope: 'Finite production Main A/B smoke only, not full Source8 qualification or Main numeric credit/work/GC/RSS/GPU/fullD1 proof',
	points,
	viewport: { width: 1360, height: 1000 },
	deviceScaleFactor: 1,
	startedAt: new Date().toISOString(),
	node: process.version,
	cases: [],
	screenshots: [],
	pageErrors: [],
	httpErrors: [],
	served: [],
	assets: null,
	sourceFiles: {},
	components: {},
};
const A = 'general-multiply-alpha',
	B = 'prepared-chain-0';
let server,
	browser,
	page,
	assets,
	error = null,
	postError = null;
const command = process.platform === 'darwin' ? 'Meta' : 'Control';
const tick = (page) =>
	page.evaluate(
		() =>
			new Promise((resolve) =>
				requestAnimationFrame(() => requestAnimationFrame(resolve)),
			),
	);
function sourceInventory() {
	const result = {};
	for (const file of [
		fileURLToPath(import.meta.url),
		fileURLToPath(
			new URL('./lib/main-blend-browser-driver.mjs', import.meta.url),
		),
		fileURLToPath(
			new URL('./lib/main-blend-browser-fixtures.mjs', import.meta.url),
		),
		path.join(
			app,
			'src/paintcraft-render-worker/tests/source8-service-fixtures/oracle.json',
		),
	]) {
		const bytes = fs.readFileSync(file);
		result[path.relative(root, file)] = {
			bytes: bytes.length,
			sha256: sha(bytes),
		};
	}
	return result;
}
async function group(name, fn) {
	const row = { name, status: 'FAILED' };
	try {
		Object.assign(row, await fn());
		row.status = 'PASS';
	} finally {
		evidence.cases.push(row);
		save(`case-${evidence.cases.length}.json`, row);
	}
}
async function screenshot(name) {
	await page.mouse.move(130, 900);
	await tick(page);
	const bytes = await page.screenshot();
	fs.writeFileSync(path.join(out, name + '.png'), bytes, { flag: 'wx' });
	evidence.screenshots.push({ file: name + '.png', sha256: sha(bytes) });
	return bytes;
}
const trace = () => page.evaluate(() => window.__mainBlendSmoke);
const sequence = () => page.evaluate(() => window.__mainBlendSmoke.sequence);
async function click(key) {
	await page.mouse.click(...points[key]);
	await tick(page);
}
async function freshPage() {
	page = await browser.newPage({
		viewport: evidence.viewport,
		deviceScaleFactor: 1,
		acceptDownloads: true,
		locale: 'en-US',
	});
	await page.addInitScript(() =>
		localStorage.setItem('showWelcomeScreen', 'false'),
	);
	await page.addInitScript(observeWorkers);
	page.on('pageerror', (problem) => {
		if (evidence.pageErrors.length < 32)
			evidence.pageErrors.push(String(problem));
		else error ??= 'page error diagnostic limit';
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
}
async function open(id) {
	const specimen = fixture(id),
		since = await sequence();
	const chooser = page.waitForEvent('filechooser', { timeout: 15000 });
	await click('file');
	await click('open');
	await (await chooser).setFiles({
		name: id + '.pcnd',
		mimeType: 'application/octet-stream',
		buffer: specimen.bytes,
	});
	return since;
}
async function frame(since) {
	await page.waitForFunction(
		(since) =>
			window.__mainBlendSmoke.workers.some((w) => {
				const opened = w.events.find(
					(e) =>
						e.direction === 'reply' &&
						e.protocol === 8 &&
						e.kind === 'opened' &&
						e.sequence > since,
				);
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
					opened &&
					open?.encodedSha256 &&
					view &&
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
		since,
		{ timeout: 60000 },
	);
	await tick(page);
	const q = await trace();
	healthy(q);
	const worker = q.workers.find(
		(w) =>
			completed(w) &&
			w.events.some(
				(e) =>
					e.direction === 'reply' &&
					e.protocol === 8 &&
					e.kind === 'opened' &&
					e.sequence > since,
			),
	);
	assert(worker);
	return worker;
}
async function saved(id, label) {
	const pending = page.waitForEvent('download', { timeout: 30000 });
	await page.keyboard.press(command + '+s');
	const download = await pending,
		file = path.join(out, label + '.pcnd');
	await download.saveAs(file);
	assert(fs.statSync(file).size <= 512);
	const bytes = fs.readFileSync(file);
	assertSaved(bytes, id);
	await tick(page);
	assert.equal(
		await page.evaluate(() => window.paintcraft.is_dirty()),
		false,
	);
	return {
		file: path.basename(file),
		bytes: bytes.length,
		sha256: sha(bytes),
		clean: true,
	};
}
try {
	assets = inventory(dist);
	evidence.assets = assets;
	evidence.sourceFiles = sourceInventory();
	for (const id of [A, B]) {
		const row = fixture(id);
		fs.writeFileSync(path.join(out, id + '.pcnd'), row.bytes, {
			flag: 'wx',
		});
	}
	for (const [name, directory, binary] of [
		['main', 'pkg', 'paintcraft_web_bg.wasm'],
		['worker', 'render-worker-pkg', 'paintcraft_web_render_worker_bg.wasm'],
	]) {
		const bytes = fs.readFileSync(path.join(app, directory, binary)),
			digest = sha(bytes);
		const matches = Object.entries(assets).filter(
			([file, row]) => file.endsWith('.wasm') && row.sha256 === digest,
		);
		assert.equal(
			matches.length,
			1,
			'actual generated component must be in dist',
		);
		evidence.components[name] = {
			generated: path.join(directory, binary),
			bytes: bytes.length,
			sha256: digest,
			asset: matches[0][0],
		};
	}
	save('inputs.json', {
		points,
		assets: evidence.assets,
		sources: evidence.sourceFiles,
		components: evidence.components,
	});
	server = serve(dist, assets, evidence.served, evidence.httpErrors);
	await new Promise((resolve, reject) => {
		server.once('error', reject);
		server.listen(0, '127.0.0.1', resolve);
	});
	const playwrightFile = process.env.PLAYWRIGHT_MODULE
		? path.resolve(process.env.PLAYWRIGHT_MODULE)
		: fileURLToPath(import.meta.resolve('playwright-core'));
	evidence.playwright = {
		path: playwrightFile,
		sha256: sha(fs.readFileSync(playwrightFile)),
	};
	const { chromium } = await import(pathToFileURL(playwrightFile).href);
	browser = await chromium.launch({
		channel: 'chrome',
		headless: true,
		args: ['--enable-unsafe-swiftshader'],
	});
	evidence.browser = browser.version();
	evidence.software_backend_permitted = true;
	await freshPage();
	let first, failed;
	await group('real Main A fullRGBA/canonical Save/Undo/clean', async () => {
		first = await frame(await open(A));
		const pixels = assertFrame(first, fixture(A));
		const before = await saved(A, 'A-before-undo');
		await page.keyboard.press(command + '+z');
		await tick(page);
		return { pixels, before, afterUndo: await saved(A, 'A-after-undo') };
	});
	await group(
		'idle real source/view is not recaptured or renewed',
		async () => {
			const before = await trace(),
				requests = (q) =>
					q.workers
						.flatMap((w) => w.events)
						.filter(
							(e) =>
								e.direction === 'request' &&
								e.protocol === 8 &&
								['open', 'view'].includes(e.kind),
						).length;
			for (let i = 0; i < 12; i++) await tick(page);
			const after = await trace();
			healthy(after);
			assert.equal(requests(after), requests(before));
			assert.deepEqual(
				openEvent(after.workers[first.index]),
				openEvent(before.workers[first.index]),
			);
			return { requests: requests(after) };
		},
	);
	await group(
		'same-page old Source8 closes before real B open/publication/Save',
		async () => {
			const second = await frame(await open(B)),
				q = await trace(),
				old = q.workers[first.index];
			const closed = old.events.find(
				(e) =>
					e.direction === 'reply' &&
					e.protocol === 8 &&
					e.kind === 'closed',
			);
			assert(closed && closed.sequence < second.created);
			assert(old.terminated !== null && old.terminated < second.created);
			const pixels = assertFrame(second, fixture(B));
			await page.keyboard.press(command + '+b');
			await tick(page);
			const marker = opaqueMarker(
				await screenshot('B-published'),
				points.canvasClip,
			);
			return {
				pixels,
				marker,
				saved: await saved(B, 'B-saved'),
				closed_before_new_worker: true,
			};
		},
	);
	save('healthy-trace.json', await trace());
	await page.close();
	await freshPage();
	await group(
		'finite actual OPEN-byte corruption joins typed failure and preserves clean A',
		async () => {
			await page.evaluate(() => {
				window.__mainBlendSmoke.faultOnce = true;
			});
			const since = await open(A);
			await page.waitForFunction(
				(since) =>
					window.__mainBlendSmoke.workers.some((w) =>
						w.events.some(
							(e) =>
								e.direction === 'reply' &&
								e.protocol === 8 &&
								e.kind === 'error' &&
								e.closed === true &&
								e.sequence > since,
						),
					),
				since,
				{ timeout: 60000 },
			);
			await tick(page);
			let q = await trace();
			healthy(q);
			assert.equal(q.faults, 1);
			failed = q.workers.find((w) =>
				w.events.some(
					(e) =>
						e.direction === 'reply' &&
						e.protocol === 8 &&
						e.kind === 'error' &&
						e.closed === true,
				),
			);
			assert(failed);
			const refusal = failed.events.find(
				(e) => e.kind === 'error' && e.closed === true,
			);
			assert.equal(refusal.detail.code, 'capability.unsupported');
			assert.equal(openEvent(failed).injectedByte, 0);
			const count = q.workers.length;
			for (let i = 0; i < 12; i++) await tick(page);
			q = await trace();
			healthy(q);
			assert.equal(
				q.workers.length,
				count,
				'same failed source must stay held',
			);
			return {
				fault: 'one real outgoing source byte; genuine Rust error/consuming close, no fake trap',
				code: refusal.detail.code,
				saved: await saved(A, 'fault-A-original'),
			};
		},
	);
	await group(
		'changed source after joined failed OPEN uses real Main B callbacks',
		async () => {
			const next = await frame(await open(B)),
				refusal = failed.events.find(
					(e) => e.kind === 'error' && e.closed === true,
				);
			assert(refusal.sequence < next.created);
			return {
				pixels: assertFrame(next, fixture(B)),
				saved: await saved(B, 'fault-retry-B'),
			};
		},
	);
	assert.equal(evidence.cases.length, 5);
	assert(evidence.cases.every((row) => row.status === 'PASS'));
	healthy(await trace());
	assert.deepEqual(evidence.pageErrors, []);
	assert.deepEqual(evidence.httpErrors, []);
	for (const component of Object.values(evidence.components))
		assert(
			evidence.served.some(
				(row) =>
					row.file === component.asset &&
					row.sha256 === component.sha256,
			),
			'actual Main/Worker artifact was served',
		);
	evidence.status = 'PASS';
} catch (problem) {
	error = String(problem?.stack ?? problem);
} finally {
	if (page)
		try {
			save('final-trace.json', await trace());
			if (error) await screenshot('failure');
		} catch (problem) {
			error ??= String(problem);
		}
	try {
		if (browser) await browser.close();
	} catch (problem) {
		error ??= String(problem);
	}
	try {
		if (server) await new Promise((resolve) => server.close(resolve));
	} catch (problem) {
		error ??= String(problem);
	}
	try {
		if (assets)
			assert.deepEqual(
				inventory(dist),
				assets,
				'dist changed during smoke',
			);
		if (Object.keys(evidence.sourceFiles).length)
			assert.deepEqual(
				sourceInventory(),
				evidence.sourceFiles,
				'script/fixture source changed',
			);
		for (const component of Object.values(evidence.components))
			assert.equal(
				sha(fs.readFileSync(path.join(app, component.generated))),
				component.sha256,
			);
	} catch (problem) {
		postError = String(problem);
	}
	if (error || postError) evidence.status = 'FAILED';
	evidence.error = error;
	evidence.post_source_error = postError;
	evidence.source_unchanged = !postError;
	save('final.json', evidence);
}
if (evidence.status !== 'PASS')
	throw Error(error ?? postError ?? 'Main blend smoke failed');
console.log(
	JSON.stringify({
		status: evidence.status,
		cases: evidence.cases.length,
		evidence: out,
	}),
);
