// Production UI controls and downloaded files, with an independent PCND oracle.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import {
	assetInventory,
	canvasMarker,
	evaluated,
	project,
	rawSemantics,
	sha,
} from './lib/studio-filter-oracle.mjs';

assert.ok(process.env.CODEC_EVIDENCE_DIR && process.env.PLAYWRIGHT_MODULE);
const app = fileURLToPath(new URL('../', import.meta.url));
const root = path.resolve(app, '../..');
const dist = path.join(app, 'dist');
const points = JSON.parse(process.env.STUDIO_MASK_COORDINATES ?? '{}');
const phase = process.env.STUDIO_MASK_PHASE ?? 'all';
assert.ok(['layout', 'menu', 'all', 'partial', 'partial-menu'].includes(phase));
for (const value of Object.values(points))
	assert.ok(
		Array.isArray(value) &&
			value.length === 2 &&
			value.every(Number.isFinite),
	);
fs.mkdirSync(process.env.CODEC_EVIDENCE_DIR, { recursive: true });
const out = fs.mkdtempSync(
	path.join(process.env.CODEC_EVIDENCE_DIR, 'attempt-'),
);
const fixture = path.join(
	root,
	phase.startsWith('partial')
		? 'crates/services/paintcraft-formats/tests/pcnd-mask-fixtures/opaque-half.pcnd'
		: 'crates/services/paintcraft-formats/tests/browser-filter-export-fixtures/invert.pcnd',
);
const bytes = fs.readFileSync(fixture);
assert.equal(
	sha(bytes),
	phase.startsWith('partial')
		? '91e4d951c2411fd0371e4560a1d56e8f26af230037516a26bae7fb911265420c'
		: '0fa138d7d8899d8c603178029af5a5903c5aa90e732c3b270ada7df9a6d55276',
);
const source = project(bytes);
const evidence = {
	status: 'started',
	scope: 'Full-coverage mask controls and persistence; screenshot marker checks baseline presentation, not causal attenuation. Recorded Worker frames are diagnostics. Live selection clearing needs a separate UI indication.',
	phase,
	points,
	checks: [],
	assets: assetInventory(dist),
	fixtureSha256: sha(bytes),
	pageErrors: [],
	startedAt: new Date().toISOString(),
};
let browser, server, page;
try {
	server = http.createServer((request, response) => {
		const pathname = new URL(request.url, 'http://localhost').pathname;
		const file = path.resolve(
			dist,
			`.${pathname.endsWith('/') ? `${pathname}index.html` : pathname}`,
		);
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
	const settled = async () => {
		await page.waitForTimeout(500);
		await page.evaluate(
			() =>
				new Promise((resolve) =>
					requestAnimationFrame(() => requestAnimationFrame(resolve)),
				),
		);
	};
	const click = async (point) => {
		await page.mouse.click(...point);
		await settled();
	};
	const at = async (name) => {
		assert.ok(points[name], `Set observed ${name} coordinate`);
		await click(points[name]);
	};
	const shot = async (name) =>
		page.screenshot({ path: path.join(out, `${name}.png`) });
	async function fresh() {
		page = await browser.newPage({
			viewport: { width: 1360, height: 1000 },
			acceptDownloads: true,
		});
		page.on('pageerror', (e) => evidence.pageErrors.push(String(e)));
		await page.addInitScript(() => {
			localStorage.setItem('showWelcomeScreen', 'false');
			window.__maskFrames = [];
			const Original = window.Worker;
			window.Worker = class extends Original {
				constructor(...args) {
					super(...args);
					this.addEventListener('message', ({ data }) => {
						if (
							data?.kind === 'frame' &&
							data.pixels?.length <= 4096
						)
							window.__maskFrames.push({
								owner: data.owner,
								sourceSha256: data.sourceSha256,
								operationSha256: data.operationSha256,
								target: data.target,
								level: data.level,
								region: data.region,
								width: data.width,
								height: data.height,
								pixels: [...data.pixels],
							});
					});
				}
			};
		});
		await page.goto(`http://127.0.0.1:${server.address().port}/`, {
			waitUntil: 'networkidle',
		});
		await page.waitForFunction(
			() =>
				window.paintcraft && document.querySelector('#loading').hidden,
			null,
			{ timeout: 60000 },
		);
		await click(points.interface ?? [1273, 20]);
		await click(points.studio ?? [1160, 98]);
	}
	async function open(file) {
		const chooser = page.waitForEvent('filechooser');
		await click(points.file ?? [20, 46]);
		await click(points.open ?? [80, 94]);
		await (await chooser).setFiles(file);
		await settled();
	}
	async function save(name) {
		const promise = page.waitForEvent('download', { timeout: 15000 });
		await page.keyboard.press('Meta+s');
		await (await promise).saveAs(path.join(out, name));
		await settled();
		const bytes = fs.readFileSync(path.join(out, name));
		const doc = project(bytes);
		doc.snapshotSha256 = sha(bytes);
		assert.deepEqual(rawSemantics(doc), rawSemantics(source));
		return doc;
	}
	async function checkMask(name, enabled) {
		const doc = await save(name);
		assert.equal(doc.version, 4);
		assert.equal(doc.layers[0].filters.length, 1);
		const filter = doc.layers[0].filters[0];
		assert.equal(filter.id, source.layers[0].filters[0].id);
		assert.equal(filter.mask.enabled, Number(enabled));
		assert.deepEqual(filter.mask.domain, [0, 0, 2, 1]);
		assert.deepEqual(filter.mask.bands, [
			{ y0: 0, y1: 1, runs: [{ x0: 0, x1: 2, coverage: 255 }] },
		]);
		assert.deepEqual(evaluated(doc), evaluated(source));
		return doc;
	}
	await fresh();
	await open(fixture);
	const before = await save('before.pcnd');
	assert.equal(before.state, source.state);
	assert.deepEqual(before.layers[0].filters, source.layers[0].filters);
	await shot('layout');
	if (phase === 'partial') {
		evidence.scope =
			'Fractional-mask enable/disable with bound production Worker frames and exact-color screenshots at the same canvas marker; immutable saved source and Undo/Redo.';
		const presented = async (doc, expected, label) => {
			const pixels = [...evaluated(expected)];
			await page.waitForFunction(
				({ state, pixels, sourceSha256, operationSha256 }) =>
					window.__maskFrames.some(
						(f) =>
							f.owner?.state === state &&
							f.sourceSha256 === sourceSha256 &&
							f.operationSha256 === operationSha256 &&
							f.target === null &&
							f.level === 0 &&
							f.width === 2 &&
							f.height === 1 &&
							JSON.stringify(f.region) === '[0,0,2,1]' &&
							JSON.stringify(f.pixels) === JSON.stringify(pixels),
					),
				{
					state: doc.state,
					pixels,
					sourceSha256: doc.snapshotSha256,
					operationSha256: sha(Buffer.from('null')),
				},
				{ timeout: 60000 },
			);
			const marker = await canvasMarker(await shot(label), [
				pixels.slice(0, 3),
				pixels.slice(4, 7),
			]);
			evidence.checks.push(
				`${label}: current-state Worker pixels and literal canvas colors`,
			);
			return marker;
		};
		const marker = await presented(before, source, 'fractional-enabled');
		await at('firstMenu');
		await shot('fractional-mask-menu');
		await at('maskEnabled');
		const disabled = await save('fractional-disabled.pcnd');
		assert.notEqual(disabled.state, before.state);
		const expectedDisabled = structuredClone(source);
		expectedDisabled.layers[0].filters[0].mask.enabled = 0;
		assert.deepEqual(
			disabled.layers[0].filters,
			expectedDisabled.layers[0].filters,
		);
		assert.deepEqual(
			await presented(disabled, expectedDisabled, 'fractional-disabled'),
			marker,
		);
		await page.keyboard.press('Meta+z');
		await settled();
		const undo = await save('fractional-undo.pcnd');
		assert.equal(undo.state, before.state);
		assert.deepEqual(undo.layers[0].filters, before.layers[0].filters);
		assert.deepEqual(
			await presented(undo, source, 'fractional-undo'),
			marker,
		);
		await page.keyboard.press('Meta+Shift+z');
		await settled();
		const redo = await save('fractional-redo.pcnd');
		assert.equal(redo.state, disabled.state);
		assert.deepEqual(redo.layers[0].filters, disabled.layers[0].filters);
		assert.deepEqual(
			await presented(redo, expectedDisabled, 'fractional-redo'),
			marker,
		);
		evidence.status = 'passed';
	} else if (phase === 'layout') {
		evidence.status = 'calibration-only';
	} else {
		await page.keyboard.press('Meta+a');
		await settled();
		await at('firstMenu');
		await shot('mask-menu');
		if (phase === 'menu' || phase === 'partial-menu')
			evidence.status = 'calibration-only';
		else {
			await at('useSelection');
			const selected = await checkMask('selected.pcnd', true);
			assert.notEqual(selected.state, before.state);
			evidence.checks.push(
				'Real Select All and Use Selection publish a full canonical mask with stable filter identity',
			);
			await page.keyboard.press('Meta+d');
			await settled();
			const deselected = await checkMask('deselected.pcnd', true);
			assert.equal(deselected.state, selected.state);
			evidence.checks.push(
				'Deselect shortcut leaves the stored mask and source pixels unchanged',
			);
			await at('firstMenu');
			await shot('stored-mask-menu');
			await at('maskEnabled');
			const disabled = await checkMask('disabled.pcnd', false);
			assert.notEqual(disabled.state, selected.state);
			await page.keyboard.press('Meta+z');
			await settled();
			const undoDisabled = await checkMask('undo-disabled.pcnd', true);
			assert.equal(undoDisabled.state, selected.state);
			await page.keyboard.press('Meta+Shift+z');
			await settled();
			const redoDisabled = await checkMask('redo-disabled.pcnd', false);
			assert.equal(redoDisabled.state, disabled.state);
			evidence.checks.push(
				'Mask disable, Undo and Redo preserve exact coverage',
			);
			await at('firstMenu');
			await at('removeMask');
			const removed = await save('removed.pcnd');
			assert.notEqual(removed.state, disabled.state);
			assert.equal(removed.version, 3);
			assert.deepEqual(
				removed.layers[0].filters,
				source.layers[0].filters,
			);
			await page.keyboard.press('Meta+z');
			await settled();
			const undoRemove = await checkMask('undo-remove.pcnd', false);
			assert.equal(undoRemove.state, disabled.state);
			evidence.checks.push(
				'Removing the last mask returns PCND3; Undo restores disabled mask intent',
			);
			await shot('mask-final');
			await canvasMarker(await page.screenshot());
			await page.close();
			await fresh();
			await open(path.join(out, 'undo-remove.pcnd'));
			const reopened = await checkMask('fresh-reopen.pcnd', false);
			assert.equal(reopened.state, undoRemove.state);
			assert.deepEqual(
				fs.readFileSync(path.join(out, 'fresh-reopen.pcnd')),
				fs.readFileSync(path.join(out, 'undo-remove.pcnd')),
			);
			await canvasMarker(await shot('fresh-reopen'));
			evidence.checks.push(
				'Fresh production application reopens and byte-identically saves PCND4 with visible rendered marker',
			);
			evidence.status = 'passed';
		}
	}
	assert.deepEqual(evidence.pageErrors, []);
} catch (error) {
	evidence.status = 'failed';
	evidence.error = String(error?.stack ?? error);
	if (page)
		await page
			.screenshot({ path: path.join(out, 'failure.png') })
			.catch(() => {});
	throw error;
} finally {
	try {
		if (page)
			evidence.frames = await page
				.evaluate(() => window.__maskFrames)
				.catch(() => []);
		evidence.finalAssets = assetInventory(dist);
		assert.deepEqual(evidence.finalAssets, evidence.assets);
	} catch (error) {
		evidence.status = 'failed';
		evidence.error = String(error?.stack ?? error);
		throw error;
	} finally {
		evidence.finishedAt = new Date().toISOString();
		try {
			fs.writeFileSync(
				path.join(out, 'production.json'),
				`${JSON.stringify(evidence, null, 2)}\n`,
			);
		} finally {
			try {
				await browser?.close();
			} finally {
				if (server?.listening)
					await new Promise((resolve) => server.close(resolve));
				console.log(out);
			}
		}
	}
}
