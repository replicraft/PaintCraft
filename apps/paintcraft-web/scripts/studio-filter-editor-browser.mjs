// Visible Studio controls against built production assets; no product hooks.
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

const app = fileURLToPath(new URL('../', import.meta.url)),
	root = path.resolve(app, '../..'),
	dist = path.join(app, 'dist');
assert.ok(process.env.CODEC_EVIDENCE_DIR && process.env.PLAYWRIGHT_MODULE);
const phase = process.env.STUDIO_FILTER_PHASE ?? 'all';
assert.ok(
	[
		'layout',
		'refusal',
		'menu',
		'parameters',
		'invert-parameters',
		'private',
		'apply',
		'stack',
		'stack-menu',
		'all',
	].includes(phase),
);
const points = JSON.parse(process.env.STUDIO_FILTER_COORDINATES ?? '{}');
assert.ok(Object.keys(points).length <= 40);
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
const evidence = {
	status: 'started',
	phase,
	points,
	assets: {},
	sources: {},
	checks: [],
	startedAt: new Date().toISOString(),
};
const fixtureRoot = path.join(
	root,
	'crates/services/paintcraft-formats/tests/browser-filter-export-fixtures',
);
const fixture =
	phase === 'refusal'
		? process.env.STUDIO_FILTER_REFUSAL_SOURCE
		: path.join(fixtureRoot, 'invert.pcnd');
assert.ok(fixture);
const oracle = JSON.parse(
	fs.readFileSync(path.join(fixtureRoot, 'oracle.json')),
);
const specimen =
	phase === 'refusal'
		? JSON.parse(
				fs.readFileSync(
					path.join(path.dirname(fixture), 'oracle.json'),
				),
			)
		: oracle.fixtures.find((f) => f.file === 'invert.pcnd');
assert.equal(sha(fs.readFileSync(fixture)), specimen.sha256);
const authored = project(fs.readFileSync(fixture));
if (phase === 'refusal') {
	assert.equal(authored.version, 2);
	assert.equal(authored.layers[0].filters.length, 0);
	assert.equal(authored.attachment.profileSha, specimen.profile_sha256);
	assert.deepEqual(
		evaluated(authored),
		Buffer.from(
			Array(authored.width * authored.height)
				.fill(specimen.raw_rgba)
				.flat(),
		),
	);
} else {
	assert.deepEqual([...evaluated(authored)], specimen.expected_rgba);
}
evidence.assets = assetInventory(dist);
for (const p of [
	fileURLToPath(import.meta.url),
	fileURLToPath(new URL('./lib/studio-filter-oracle.mjs', import.meta.url)),
	fixture,
	path.join(fixtureRoot, 'oracle.json'),
	...fs
		.readdirSync(path.join(app, 'src/paintcraft-web/app/filter_editing'))
		.filter((n) => n.endsWith('.rs'))
		.map((n) => path.join(app, 'src/paintcraft-web/app/filter_editing', n)),
	...[
		'app.rs',
		'app/presentation.rs',
		'app/filter_editing.rs',
		'canvas.rs',
		'layers.rs',
		'editor/filters.rs',
	].map((n) => path.join(app, 'src/paintcraft-web', n)),
	...[
		'filter-preview-bridge.js',
		'filter-preview-worker.js',
		'filter-preview-protocol.js',
		'filter-preview-runtime.js',
	].map((n) => path.join(app, 'src/lib', n)),
])
	evidence.sources[path.relative(root, p)] = {
		bytes: fs.statSync(p).size,
		sha256: sha(fs.readFileSync(p)),
	};
if (phase === 'refusal') {
	evidence.refusalOracle = specimen;
	for (const name of ['oracle.json', 'generate.py']) {
		const file = path.join(path.dirname(fixture), name);
		evidence.sources[path.relative(root, file)] = {
			bytes: fs.statSync(file).size,
			sha256: sha(fs.readFileSync(file)),
		};
	}
}
fs.copyFileSync(
	fileURLToPath(import.meta.url),
	path.join(out, 'driver-source.mjs'),
);
fs.copyFileSync(
	fileURLToPath(new URL('./lib/studio-filter-oracle.mjs', import.meta.url)),
	path.join(out, 'oracle-source.mjs'),
);
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
				'.json': 'application/json',
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
	async function freshPage() {
		page = await browser.newPage({
			viewport: { width: 1360, height: 1000 },
			acceptDownloads: true,
		});
		await page.addInitScript(() => {
			localStorage.setItem('showWelcomeScreen', 'false');
			window.__studioWorkers = [];
			window.__studioSequence = 0;
			const Original = window.Worker;
			window.Worker = class extends Original {
				constructor(...args) {
					super(...args);
					this.record = {
						url: String(args[0]),
						events: [],
						terminated: false,
					};
					window.__studioWorkers.push(this.record);
					this.addEventListener('message', ({ data }) => {
						if (this.record.events.length >= 500) return;
						const record = {
							direction: 'reply',
							sequence: ++window.__studioSequence,
							kind: data?.kind,
							id: data?.id,
							generation: data?.generation,
							owner: data?.owner,
							draft: data?.draft,
							target: data?.target,
							view: data?.view,
							region: data?.region,
							level: data?.level,
							width: data?.width,
							height: data?.height,
							operationSha256: data?.operationSha256,
							code: data?.code,
							completed: data?.completed,
						};
						if (
							data?.pixels instanceof Uint8Array &&
							data.pixels.length <= 4096
						)
							record.pixels = Array.from(data.pixels);
						if (
							data?.pixels instanceof Uint8Array &&
							data.pixels.length <= 1 << 20
						) {
							record.rgbaBytes = data.pixels.length;
							const copy = data.pixels.slice();
							crypto.subtle
								.digest('SHA-256', copy)
								.then((bytes) => {
									record.rgbaSha256 = Array.from(
										new Uint8Array(bytes),
										(b) => b.toString(16).padStart(2, '0'),
									).join('');
								})
								.catch((error) => {
									record.observerError = String(error);
								});
						}
						this.record.events.push(record);
					});
				}
				postMessage(value, transfer) {
					if (this.record.events.length < 500)
						this.record.events.push({
							direction: 'request',
							sequence: ++window.__studioSequence,
							kind: value?.kind,
							id: value?.id,
							request: value?.request,
							plan: value?.plan,
							sourceBytes: value?.source?.byteLength,
						});
					return super.postMessage(value, transfer);
				}
				terminate() {
					this.record.terminated = true;
					return super.terminate();
				}
			};
		});
		page.on('pageerror', (e) => {
			evidence.pageErrors ??= [];
			evidence.pageErrors.push(String(e));
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
		await click(...(points.interface ?? [1273, 20]));
		await click(...(points.studio ?? [1160, 98]));
		await settled();
	}
	const click = async (x, y) => {
		await page.mouse.click(x, y);
		await page.waitForTimeout(350);
	};
	const at = async (name) => {
		assert.ok(points[name], `Set observed ${name} coordinate`);
		await click(...points[name]);
	};
	const screenshot = async (name) =>
		page.screenshot({ path: path.join(out, `${name}.png`) });
	const records = () => page.evaluate(() => window.__studioWorkers);
	const checkpoint = () => page.evaluate(() => window.__studioSequence);
	async function frame(expected, since, label, state = null) {
		await page.waitForFunction(
			({ expected, since, state }) =>
				window.__studioWorkers.some((w) =>
					w.events.some(
						(e) =>
							e.direction === 'reply' &&
							e.kind === 'frame' &&
							e.sequence > since &&
							(!state || e.owner?.state === state) &&
							JSON.stringify(e.pixels) ===
								JSON.stringify(expected),
					),
				),
			{ expected, since, state },
			{ timeout: 60000 },
		);
		evidence.checks.push(label);
	}
	async function settled() {
		await page.waitForTimeout(400);
		await page.evaluate(
			() =>
				new Promise((resolve) =>
					requestAnimationFrame(() => requestAnimationFrame(resolve)),
				),
		);
	}
	async function save(name) {
		const promise = page.waitForEvent('download', { timeout: 15000 });
		await page.keyboard.press('Meta+s');
		const download = await promise;
		await download.saveAs(path.join(out, name));
		await settled();
		return fs.readFileSync(path.join(out, name));
	}
	async function open(file) {
		const chooser = page.waitForEvent('filechooser');
		await click(...(points.file ?? [20, 46]));
		await click(...(points.open ?? [80, 94]));
		await (await chooser).setFiles(file);
		await settled();
	}
	async function add(kind) {
		await at('add');
		await screenshot('add-menu');
		if (phase === 'menu') return false;
		await at(kind === 'invert' ? 'addInvert' : 'addBC');
		await screenshot('parameters');
		return true;
	}
	async function value(name, text) {
		assert.ok(points[name]);
		await page.mouse.dblclick(...points[name]);
		await page.keyboard.press('Meta+a');
		await page.keyboard.type(String(text));
		await page.keyboard.press('Enter');
		await settled();
	}
	await freshPage();
	let finalArtifact = 'before.pcnd';
	if (phase === 'refusal') {
		let downloads = 0;
		page.on('download', () => downloads++);
		await open(fixture);
		const before = await save('refusal-before.pcnd');
		assert.deepEqual(before, fs.readFileSync(fixture));
		const title = await page.title();
		await at('add');
		await at('addInvert');
		await page.waitForFunction(
			() =>
				window.__studioWorkers.some((w) =>
					w.events.some(
						(e) =>
							e.kind === 'frame' &&
							typeof e.rgbaSha256 === 'string',
					),
				),
			null,
			{ timeout: 60000 },
		);
		const replies = await records();
		const worker = replies.find((w) =>
			w.events.some((e) => e.kind === 'frame'),
		);
		const actual = worker.events.find((e) => e.kind === 'frame');
		const request = worker.events.find(
			(e) =>
				e.direction === 'request' &&
				e.kind === 'update' &&
				e.id === actual.id,
		);
		assert.deepEqual(actual.view, request.request.view);
		assert.deepEqual(actual.region, actual.view.region);
		assert.equal(actual.level, 0);
		const [x, y, w, h] = actual.region;
		assert.ok(
			x >= 0 && y >= 0 && w > 0 && h > 0 && x + w <= 128 && y + h <= 128,
		);
		assert.deepEqual([actual.width, actual.height], [w, h]);
		assert.equal(actual.rgbaBytes, w * h * 4);
		const cyan = Buffer.from(
			Array(w * h)
				.fill([0, 255, 255, 255])
				.flat(),
		);
		assert.equal(actual.rgbaSha256, sha(cyan));
		evidence.refusalPreview = {
			region: actual.region,
			width: w,
			height: h,
			rgbaSha256: actual.rgbaSha256,
			bytes: cyan.length,
		};
		await at('applyInvert');
		await settled();
		await screenshot('retained-refusal');
		const beforeBlockedSave = downloads;
		await page.keyboard.press('Meta+s');
		await page.waitForTimeout(750);
		assert.equal(
			downloads,
			beforeBlockedSave,
			'Failed publication retains active parameter fence',
		);
		assert.equal(await page.title(), title);
		await at('applyInvert');
		await settled();
		await page.keyboard.press('Meta+s');
		await page.waitForTimeout(750);
		assert.equal(
			downloads,
			beforeBlockedSave,
			'Retry cannot publish or discard the rejected candidate',
		);
		await at('cancelInvert');
		await settled();
		const cancelled = await save('refusal-cancelled.pcnd');
		assert.deepEqual(cancelled, before);
		const restored = project(cancelled);
		assert.equal(restored.layers[0].filters.length, 0);
		assert.equal(restored.attachment.profileSha, specimen.profile_sha256);
		assert.deepEqual(rawSemantics(restored), rawSemantics(authored));
		evidence.firstPageWorkers = await records();
		await page.close();
		await freshPage();
		await open(path.join(out, 'refusal-cancelled.pcnd'));
		const reopened = await save('refusal-reopened.pcnd');
		assert.deepEqual(reopened, before);
		evidence.checks.push(
			'Actual8MiBcompact-only source opens and private Invert renders independent fullcyan samples',
		);
		evidence.checks.push(
			'Real persistent-admission refusal retains parameter fence across Apply and retry',
		);
		evidence.checks.push(
			'Cancel and freshApp reopen/save preserve exactoriginal profile/source/rawpixels and empty filterstack',
		);
		evidence.status = 'passed';
	} else {
		let stamp = await checkpoint();
		await open(fixture);
		await frame(
			specimen.expected_rgba,
			stamp,
			'actual committed filtered source rendered by background Worker',
		);
		const before = await save('before.pcnd');
		const beforeDoc = project(before);
		assert.deepEqual(rawSemantics(beforeDoc), rawSemantics(authored));
		assert.equal(beforeDoc.layers[0].filters.length, 1);
		evidence.titleBefore = await page.title();
		await screenshot('layout');
		if (phase === 'layout') {
			evidence.status = 'calibration-only';
		} else if (
			!(await add(phase === 'invert-parameters' ? 'invert' : 'bc'))
		) {
			evidence.status = 'calibration-only';
		} else if (phase === 'parameters' || phase === 'invert-parameters') {
			evidence.status = 'calibration-only';
		} else {
			stamp = await checkpoint();
			await value('brightnessValue', 20);
			const bright = [255, 255, 255, 255, 51, 51, 51, 255];
			await frame(
				bright,
				stamp,
				'actual private brightness20 preview matches independent literal samples',
			);
			await settled();
			await page.mouse.move(800, 300);
			evidence.privateAppearance = await canvasMarker(
				await page.screenshot(),
				[255, 51],
			);
			await screenshot('private-preview');
			await at('previewBC');
			await settled();
			await page.mouse.move(800, 300);
			const beforeMarker = await canvasMarker(await page.screenshot());
			await page.mouse.move(
				beforeMarker.x + beforeMarker.width / 2,
				beforeMarker.y + beforeMarker.height / 2,
			);
			await page.keyboard.down('Meta');
			await page.mouse.wheel(0, 120);
			await page.keyboard.up('Meta');
			await settled();
			await page.mouse.move(800, 300);
			const zoomMarker = await canvasMarker(await page.screenshot());
			assert.notEqual(
				zoomMarker.width,
				beforeMarker.width,
				'Actual Ctrl/Cmd wheel changes canvas scale while parameters remain open',
			);
			await page.mouse.move(
				zoomMarker.x + zoomMarker.width / 2,
				zoomMarker.y + zoomMarker.height / 2,
			);
			await page.mouse.down({ button: 'middle' });
			await page.mouse.move(
				zoomMarker.x + zoomMarker.width / 2 + 35,
				zoomMarker.y + zoomMarker.height / 2 + 20,
				{ steps: 5 },
			);
			await page.mouse.up({ button: 'middle' });
			await settled();
			await page.mouse.move(800, 300);
			const panMarker = await canvasMarker(await page.screenshot());
			assert.ok(
				Math.abs(panMarker.x - zoomMarker.x) >= 25 &&
					Math.abs(panMarker.y - zoomMarker.y) >= 12,
				'Actual middle drag moves the literal canvas marker',
			);
			evidence.navigation = { beforeMarker, zoomMarker, panMarker };
			await screenshot('navigation');
			await at('cancel');
			await settled();
			const cancelled = await save('cancelled.pcnd');
			finalArtifact = 'cancelled.pcnd';
			assert.deepEqual(cancelled, before);
			assert.equal(await page.title(), evidence.titleBefore);
			evidence.checks.push(
				'Cancel plus nonmodal viewport navigation preserve byte-identical project and title',
			);
			if (phase === 'private') {
				evidence.status = 'passed';
			} else {
				await add('invert');
				stamp = await checkpoint();
				await at('previewInvert');
				await settled();
				await page.waitForFunction(
					(since) =>
						window.__studioWorkers.some((w) =>
							w.events.some(
								(e) =>
									e.direction === 'reply' &&
									e.kind === 'prepared' &&
									e.sequence > since,
							),
						),
					stamp,
					{ timeout: 60000 },
				);
				await at('applyInvert');
				await settled();
				const applied = await save('applied.pcnd');
				const appliedDoc = project(applied);
				await frame(
					oracle.raw_rgba,
					stamp,
					'Preview-off Apply receives committed background frame for the saved state',
					appliedDoc.state,
				);
				assert.deepEqual(
					rawSemantics(appliedDoc),
					rawSemantics(beforeDoc),
				);
				assert.equal(appliedDoc.layers[0].filters.length, 2);
				assert.deepEqual(
					appliedDoc.layers[0].filters.map((f) => [
						f.kind,
						f.enabled,
					]),
					[
						[1, 1],
						[1, 1],
					],
				);
				assert.deepEqual([...evaluated(appliedDoc)], oracle.raw_rgba);
				evidence.checks.push(
					'Preview-off Apply uses current PREPARED then saves ordered PCND3 stack and untouched raw samples',
				);
				stamp = await checkpoint();
				await page.keyboard.press('Meta+z');
				await frame(
					specimen.expected_rgba,
					stamp,
					'Undo performs real background committed refresh',
				);
				const undone = project(await save('undone.pcnd'));
				assert.equal(undone.layers[0].filters.length, 1);
				assert.deepEqual(rawSemantics(undone), rawSemantics(beforeDoc));
				stamp = await checkpoint();
				await page.keyboard.press('Meta+Shift+z');
				await frame(
					oracle.raw_rgba,
					stamp,
					'Redo performs real background committed refresh',
				);
				const redone = project(await save('redone.pcnd'));
				finalArtifact = 'redone.pcnd';
				assert.equal(redone.layers[0].filters.length, 2);
				assert.deepEqual(rawSemantics(redone), rawSemantics(beforeDoc));
				evidence.appliedAppearance = await canvasMarker(
					await page.screenshot(),
					[0, 255],
				);
				await screenshot('applied');
				if (
					phase === 'stack' ||
					phase === 'all' ||
					phase === 'stack-menu'
				) {
					const toggleStamp = await checkpoint();
					await at('firstEnabled');
					await settled();
					const toggled = project(await save('toggled.pcnd'));
					await frame(
						specimen.expected_rgba,
						toggleStamp,
						'Direct enabled row refreshes actual background pixels for saved state',
						toggled.state,
					);
					assert.deepEqual(
						toggled.layers[0].filters.map((f) => f.enabled),
						[0, 1],
					);
					assert.deepEqual(
						[...evaluated(toggled)],
						specimen.expected_rgba,
					);
					await add('bc');
					stamp = await checkpoint();
					await value('brightnessValue', 20);
					await frame(
						bright,
						stamp,
						'private ordered BC candidate respects disabled and enabled filters',
					);
					await at('apply');
					await settled();
					const ordered = project(await save('ordered.pcnd'));
					assert.deepEqual(
						ordered.layers[0].filters.map((f) => [
							f.kind,
							f.enabled,
						]),
						[
							[1, 0],
							[1, 1],
							[2, 1],
						],
					);
					assert.equal(ordered.layers[0].filters[2].brightness, 20);
					assert.deepEqual(
						rawSemantics(ordered),
						rawSemantics(beforeDoc),
					);
					assert.deepEqual([...evaluated(ordered)], bright);
					const reorderStamp = await checkpoint();
					await at('thirdMenu');
					await screenshot('row-menu');
					if (phase === 'stack-menu') {
						evidence.status = 'calibration-only';
					} else {
						await at('moveUp');
						await settled();
						const reordered = project(await save('reordered.pcnd'));
						finalArtifact = 'reordered.pcnd';
						await frame(
							[204, 204, 204, 255, 0, 0, 0, 255],
							reorderStamp,
							'Direct reorder refreshes actual noncommutative pixels for saved state',
							reordered.state,
						);
						assert.deepEqual(
							reordered.layers[0].filters.map((f) => f.kind),
							[1, 2, 1],
						);
						assert.deepEqual(
							rawSemantics(reordered),
							rawSemantics(beforeDoc),
						);
						assert.deepEqual(
							[...evaluated(reordered)],
							[204, 204, 204, 255, 0, 0, 0, 255],
						);
						evidence.checks.push(
							'Direct enabled/reorder rows and saved order match independent rational oracle',
						);
						const previousBC = reordered.layers[0].filters[1].id;
						await at('editBC');
						const editStamp = await checkpoint();
						await value('brightnessValue', 10);
						await frame(
							[229, 229, 229, 255, 0, 0, 0, 255],
							editStamp,
							'Existing filter Edit previews updated parameters independently',
						);
						await at('apply');
						await settled();
						const edited = project(await save('edited.pcnd'));
						finalArtifact = 'edited.pcnd';
						assert.deepEqual(
							rawSemantics(edited),
							rawSemantics(beforeDoc),
						);
						assert.equal(edited.layers[0].filters.length, 3);
						assert.equal(
							edited.layers[0].filters[1].id,
							previousBC,
						);
						assert.equal(
							edited.layers[0].filters[1].brightness,
							10,
						);
						assert.deepEqual(
							[...evaluated(edited)],
							[229, 229, 229, 255, 0, 0, 0, 255],
						);
						evidence.checks.push(
							'Existing-instance Apply preserves UUID and ordered stack while updating parameters',
						);
					}
				}
				if (phase !== 'stack-menu') evidence.status = 'passed';
			}
		}
	}
	if (evidence.status === 'passed' && phase !== 'refusal') {
		evidence.firstPageWorkers = await records();
		await page.close();
		await freshPage();
		const current = project(fs.readFileSync(path.join(out, finalArtifact)));
		const beforeOpen = await checkpoint();
		await open(path.join(out, finalArtifact));
		await frame(
			[...evaluated(current)],
			beforeOpen,
			'Fresh browser application reopens the saved editable stack with independent pixels',
			current.state,
		);
		const reopened = await save('reopened.pcnd');
		const visible = evaluated(current);
		evidence.reopenedAppearance = await canvasMarker(
			await page.screenshot(),
			[visible[0], visible[4]],
		);
		assert.deepEqual(
			reopened,
			fs.readFileSync(path.join(out, finalArtifact)),
		);
		evidence.checks.push(
			'Fresh app Save roundtrip is byte-identical and preserves filter identities/order/raw source',
		);
	}
	evidence.workers = await records();
	assert.ok(
		[...(evidence.firstPageWorkers ?? []), ...evidence.workers].some((w) =>
			w.events.some((e) => e.kind === 'ready'),
		),
		'Actual render Worker acknowledged source',
	);
	assert.deepEqual(evidence.pageErrors ?? [], []);
	await screenshot('final');
} catch (error) {
	evidence.status = 'failed';
	evidence.error = String(error.stack ?? error);
	if (page) {
		evidence.workers = await page
			.evaluate(() => window.__studioWorkers)
			.catch(() => []);
		await page
			.screenshot({ path: path.join(out, 'failure.png') })
			.catch(() => {});
	}
	throw error;
} finally {
	evidence.finishedAt = new Date().toISOString();
	fs.writeFileSync(
		path.join(out, 'production.json'),
		`${JSON.stringify(evidence, null, 2)}\n`,
	);
	await browser?.close();
	if (server) await new Promise((resolve) => server.close(resolve));
	console.log(out);
}
