// Real built-page rotation: UI events and independent saved straightRGBA only.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import {
	inventory,
	serve,
	observeWorkers,
	healthy,
} from './lib/main-blend-browser-driver.mjs';
import { fixture, inspect, sha } from './lib/main-rotation-browser-oracle.mjs';
const app = fileURLToPath(new URL('../', import.meta.url));
const root = path.resolve(app, '../..');
assert(
	fs.existsSync(path.join(root, 'Cargo.lock')),
	'run the promoted production script',
);
const mode = process.env.PAINT_ROTATION_MODE ?? 'run';
assert(['layout', 'run'].includes(mode));
const stage = process.env.PAINT_ROTATION_LAYOUT_STAGE ?? 'modal';
assert(
	['page', 'interface-menu', 'fixture', 'image-menu', 'modal'].includes(
		stage,
	),
);
const selected = process.env.PAINT_ROTATION_FRONTEND ?? 'both';
assert(['classic11', 'studio', 'both'].includes(selected));
const fronts = selected === 'both' ? ['classic11', 'studio'] : [selected];
for (const k of [
	'CODEC_EVIDENCE_DIR',
	'PLAYWRIGHT_MODULE',
	'PAINT_ROTATION_PRODUCER',
	'PAINT_ROTATION_PRODUCER_SHA256',
	'PAINT_ROTATION_COORDINATES',
	'PAINT_ROTATION_COORDINATES_SHA256',
])
	assert(process.env[k], `${k} required before browser`);
const receiptFile = path.resolve(process.env.PAINT_ROTATION_PRODUCER);
const receiptSHA = process.env.PAINT_ROTATION_PRODUCER_SHA256;
const coordinateFile = path.resolve(process.env.PAINT_ROTATION_COORDINATES);
const coordinateSHA = process.env.PAINT_ROTATION_COORDINATES_SHA256;
assert(
	/^[a-f0-9]{64}$/.test(receiptSHA) && /^[a-f0-9]{64}$/.test(coordinateSHA),
);
const coordinateBytes = fs.readFileSync(coordinateFile);
assert(
	coordinateBytes.length <= 8192 && sha(coordinateBytes) === coordinateSHA,
);
const coordinates = JSON.parse(coordinateBytes);
assert(coordinates && Object.getPrototypeOf(coordinates) === Object.prototype);
assert(
	Object.keys(coordinates).every((k) => ['classic11', 'studio'].includes(k)),
);
const names = [
	'interface',
	'interfaceChoice',
	'file',
	'open',
	'image',
	'rotate',
	'angle',
	'apply',
	'cancel',
];
const pair = (p) =>
	assert(
		Array.isArray(p) &&
			p.length === 2 &&
			p.every(
				(n, i) => Number.isInteger(n) && n > 0 && n < (i ? 1000 : 1360),
			),
		'bounded actual viewport point',
	);
for (const frontend of fronts) {
	const p = coordinates[frontend];
	assert(p && Object.getPrototypeOf(p) === Object.prototype);
	for (const [key, value] of Object.entries(p)) {
		assert([...names, 'setupClicks', 'setupDrags'].includes(key));
		if (key === 'setupClicks') {
			assert(Array.isArray(value) && value.length <= 8);
			value.forEach(pair);
		} else if (key === 'setupDrags') {
			assert(Array.isArray(value) && value.length <= 4);
			for (const drag of value) {
				assert(Array.isArray(drag) && drag.length === 2);
				drag.forEach(pair);
			}
		} else pair(value);
	}
	const needed =
		mode === 'run'
			? names
			: stage === 'page'
				? []
				: stage === 'interface-menu'
					? ['interface']
					: stage === 'fixture'
						? names.slice(0, 4)
						: stage === 'image-menu'
							? names.slice(0, 5)
							: names.slice(0, 6);
	needed.forEach((n) =>
		assert(
			Object.hasOwn(p, n),
			`actual ${frontend}.${n} required; layout is not acceptance`,
		),
	);
}
fs.mkdirSync(process.env.CODEC_EVIDENCE_DIR, { recursive: true });
const out = fs.mkdtempSync(
	path.join(process.env.CODEC_EVIDENCE_DIR, 'main-rotation-'),
);
const save = (name, value) =>
	fs.writeFileSync(
		path.join(out, name),
		JSON.stringify(value, null, '\t') + '\n',
		{ flag: 'wx' },
	);
const record = (p) => {
	const b = fs.readFileSync(p);
	return { path: path.relative(root, p), bytes: b.length, sha256: sha(b) };
};
const checked = (row) => {
	const p = path.resolve(root, row.path);
	assert(p.startsWith(root + path.sep));
	assert.deepEqual(record(p), row);
	return p;
};
const selfFiles = [
	fileURLToPath(import.meta.url),
	...[
		'main-rotation-browser-oracle.mjs',
		'main-rotation-browser-codecs.mjs',
		'main-rotation-browser-cases.json',
		'main-blend-browser-driver.mjs',
	].map((n) => fileURLToPath(new URL(`./lib/${n}`, import.meta.url))),
];
function tree(directory) {
	const rows = {};
	let bytes = 0;
	const visit = (folder) => {
		for (const entry of fs.readdirSync(folder, { withFileTypes: true })) {
			assert(!entry.isSymbolicLink(), 'no unreviewed generated symlink');
			const file = path.join(folder, entry.name);
			if (entry.isDirectory()) visit(file);
			else if (entry.isFile()) {
				const row = record(file);
				bytes += row.bytes;
				assert(
					row.bytes <= 25 * 1024 * 1024 && bytes <= 512 * 1024 * 1024,
				);
				rows[path.relative(directory, file)] = {
					bytes: row.bytes,
					sha256: row.sha256,
				};
				assert(Object.keys(rows).length <= 4096);
			}
		}
	};
	visit(directory);
	return rows;
}
function guard() {
	assert.equal(sha(fs.readFileSync(receiptFile)), receiptSHA);
	const receipt = JSON.parse(fs.readFileSync(receiptFile));
	assert.equal(receipt.status, 'PRODUCER_PASS_SOURCES_UNCHANGED');
	assert.equal(receipt.exit_code, 0);
	assert.equal(receipt.source_unchanged, true);
	assert.deepEqual(receipt.command, [
		'pnpm',
		'--dir',
		'apps/paintcraft-web',
		'build',
	]);
	const source = JSON.parse(fs.readFileSync(checked(receipt.source_pre)));
	assert.deepEqual(
		source,
		JSON.parse(fs.readFileSync(checked(receipt.source_post))),
	);
	assert(
		Array.isArray(source.files) &&
			source.files.length === receipt.source_rows &&
			source.files.length > 0,
	);
	source.files.forEach(checked);
	const outputs = JSON.parse(fs.readFileSync(checked(receipt.outputs_after)));
	for (const name of [
		'pkg',
		'render-worker-pkg',
		'codec-worker-pkg',
		'public/fonts',
		'dist',
	]) {
		assert.equal(outputs[name].exists, true);
		outputs[name].files.forEach(checked);
		assert.deepEqual(
			tree(path.join(app, name)),
			Object.fromEntries(
				outputs[name].files.map((row) => [
					path.relative(
						path.join(app, name),
						path.join(root, row.path),
					),
					{ bytes: row.bytes, sha256: row.sha256 },
				]),
			),
		);
	}
	return {
		producer: record(receiptFile),
		sourceRows: source.files.length,
		source: receipt.source_pre,
		outputs: receipt.outputs_after,
		self: selfFiles.map(record),
		assets: inventory(path.join(app, 'dist')),
	};
}
const result = {
	status: 'FAILED',
	mode,
	stage,
	requestedFrontends: fronts,
	cases: [],
	screenshots: [],
	served: [],
	httpErrors: [],
	pageErrors: [],
	coordinates,
	source_unchanged: false,
	scope: 'Real production page File/Open/Image/Rotate custom/angle/Apply/Cancel/UndoRedo and independent finite saved straightRGBA. Requested frontend is anchored by externally calibrated real Interface choice and captured screenshots, not a private getter. No Worker grant, private raster preview, full screenshotRGBA, numericbudget, nativeGPU/IME/GC/RSS claim.',
};
let browser,
	page,
	server,
	pre,
	error = null,
	postError = null;
const sockets = new Set();
const command = process.platform === 'darwin' ? 'Meta' : 'Control';
const tick = async (n = 3) => {
	for (let i = 0; i < n; i++)
		await page.evaluate(
			() =>
				new Promise((resolve) =>
					requestAnimationFrame(() => requestAnimationFrame(resolve)),
				),
		);
};
async function shot(name) {
	await page.mouse.move(130, 900);
	await tick();
	const bytes = await page.screenshot();
	fs.writeFileSync(path.join(out, `${name}.png`), bytes, { flag: 'wx' });
	result.screenshots.push({ name, bytes: bytes.length, sha256: sha(bytes) });
}
async function click(p) {
	await page.mouse.click(...p);
	await tick();
}
async function typedAngle(points, degrees) {
	await page.mouse.dblclick(...points.angle);
	await tick();
	await page.keyboard.press(command + '+a');
	await page.keyboard.type(String(degrees));
	await page.keyboard.press('Enter');
	await tick();
}
async function openRotation(points) {
	await click(points.image);
	await click(points.rotate);
}
async function saved(name, phase, dirty, equal = null) {
	assert.equal(
		await page.evaluate(() => window.paintcraft.is_dirty()),
		dirty,
		`${name} dirty before`,
	);
	const pending = page.waitForEvent('download', { timeout: 30000 });
	await page.keyboard.press(command + '+s');
	const download = await pending,
		file = path.join(out, `${name}.pcnd`);
	await download.saveAs(file);
	assert(fs.statSync(file).size <= 65536);
	const bytes = fs.readFileSync(file),
		facts = inspect(bytes, phase);
	if (equal) assert.deepEqual(bytes, equal, `${name} exact source archive`);
	await tick();
	assert.equal(
		await page.evaluate(() => window.paintcraft.is_dirty()),
		dirty,
		`${name} download is not confirmed disk save`,
	);
	facts.dirty = dirty;
	save(`${name}.json`, facts);
	return { bytes, facts };
}
async function history(key) {
	await page.keyboard.press(command + '+' + key);
	await tick();
}
async function group(name, fn) {
	const row = { name, status: 'FAILED' };
	try {
		Object.assign(row, await fn());
		row.status = 'PASS';
	} finally {
		result.cases.push(row);
		save(`case-${result.cases.length}.json`, row);
	}
}
try {
	pre = guard();
	save('inputs.json', { ...pre, coordinates: record(coordinateFile) });
	const input = fixture();
	inspect(input.bytes, 'baseline');
	fs.writeFileSync(path.join(out, 'input.pcnd'), input.bytes, { flag: 'wx' });
	server = serve(
		path.join(app, 'dist'),
		pre.assets,
		result.served,
		result.httpErrors,
	);
	server.on('connection', (s) => {
		sockets.add(s);
		s.once('close', () => sockets.delete(s));
	});
	await new Promise((resolve, reject) => {
		server.once('error', reject);
		server.listen(0, '127.0.0.1', resolve);
	});
	const module = path.resolve(process.env.PLAYWRIGHT_MODULE);
	result.playwright = record(module);
	const { chromium } = await import(pathToFileURL(module));
	browser = await chromium.launch({
		channel: 'chrome',
		headless: true,
		args: ['--enable-unsafe-swiftshader'],
	});
	result.browser = browser.version();
	result.cdp = await (await browser.newBrowserCDPSession()).send(
		'Browser.getVersion',
	);
	for (const frontend of fronts) {
		const context = await browser.newContext({
			viewport: { width: 1360, height: 1000 },
			deviceScaleFactor: 1,
			acceptDownloads: true,
			locale: 'en-US',
		});
		try {
			page = await context.newPage();
			await page.addInitScript(() =>
				localStorage.setItem('showWelcomeScreen', 'false'),
			);
			await page.addInitScript(observeWorkers);
			page.on('pageerror', (e) => {
				if (result.pageErrors.length < 32)
					result.pageErrors.push(String(e));
				else error ??= 'page error bound';
			});
			await page.goto(`http://127.0.0.1:${server.address().port}/`, {
				waitUntil: 'networkidle',
			});
			await page.waitForFunction(
				() =>
					window.paintcraft &&
					document.querySelector('#loading')?.hidden,
				{},
				{ timeout: 60000 },
			);
			const points = coordinates[frontend];
			if (mode === 'layout' && stage === 'page') {
				await shot(`${frontend}-page`);
				continue;
			}
			await click(points.interface);
			if (mode === 'layout' && stage === 'interface-menu') {
				const b = await page.screenshot();
				fs.writeFileSync(
					path.join(out, `${frontend}-interface-menu.png`),
					b,
					{ flag: 'wx' },
				);
				result.screenshots.push({
					name: `${frontend}-interface-menu`,
					bytes: b.length,
					sha256: sha(b),
				});
				continue;
			}
			await click(points.interfaceChoice);
			const choose = page.waitForEvent('filechooser', { timeout: 15000 });
			await click(points.file);
			await click(points.open);
			await (await choose).setFiles({
				name: 'rotation-subject.pcnd',
				mimeType: 'application/octet-stream',
				buffer: input.bytes,
			});
			await page.waitForFunction(
				() => document.title.startsWith('rotation-subject.pcnd'),
				{},
				{ timeout: 30000 },
			);
			await tick();
			for (const p of points.setupClicks ?? []) await click(p);
			for (const [start, end] of points.setupDrags ?? []) {
				await page.mouse.move(...start);
				await page.mouse.down();
				await tick();
				await page.mouse.move(...end, { steps: 8 });
				await tick();
				await page.mouse.up();
				await tick();
			}
			await shot(`${frontend}-fixture`);
			if (mode === 'layout') {
				if (stage === 'fixture') continue;
				await click(points.image);
				if (stage === 'image-menu') {
					const b = await page.screenshot();
					fs.writeFileSync(
						path.join(out, `${frontend}-image-menu.png`),
						b,
						{ flag: 'wx' },
					);
					result.screenshots.push({
						name: `${frontend}-image-menu`,
						bytes: b.length,
						sha256: sha(b),
					});
					continue;
				}
				await click(points.rotate);
				await shot(`${frontend}-modal`);
				continue;
			}
			let baseline;
			await group(
				`${frontend}: real source open and canonical save`,
				async () => {
					baseline = await saved(
						`${frontend}-baseline`,
						'baseline',
						false,
						input.bytes,
					);
					return baseline.facts;
				},
			);
			await group(
				`${frontend}: staged angle Cancel keeps source and history private`,
				async () => {
					await openRotation(points);
					await typedAngle(points, 90);
					assert.equal(
						await page.evaluate(() => window.paintcraft.is_dirty()),
						false,
					);
					await shot(`${frontend}-staged90`);
					await click(points.cancel);
					return (
						await saved(
							`${frontend}-cancel90`,
							'baseline',
							false,
							baseline.bytes,
						)
					).facts;
				},
			);
			let ninety;
			await group(
				`${frontend}: Apply90 all4RGBA then exact UndoRedo`,
				async () => {
					await openRotation(points);
					await typedAngle(points, 90);
					await click(points.apply);
					ninety = await saved(
						`${frontend}-rotate90`,
						'clockwise90',
						true,
					);
					assert.notEqual(ninety.facts.state, baseline.facts.state);
					await shot(`${frontend}-committed90`);
					await history('z');
					await saved(
						`${frontend}-undo90`,
						'baseline',
						false,
						baseline.bytes,
					);
					await history('Shift+z');
					await saved(
						`${frontend}-redo90`,
						'clockwise90',
						true,
						ninety.bytes,
					);
					return ninety.facts;
				},
			);
			await group(
				`${frontend}: Apply45 rational all9RGBA then exact UndoRedo`,
				async () => {
					await history('z');
					await saved(
						`${frontend}-before45`,
						'baseline',
						false,
						baseline.bytes,
					);
					await openRotation(points);
					await typedAngle(points, 45);
					await click(points.apply);
					const rotated = await saved(
						`${frontend}-rotate45`,
						'clockwise45',
						true,
					);
					assert.notEqual(rotated.facts.state, baseline.facts.state);
					await shot(`${frontend}-committed45`);
					await history('z');
					await saved(
						`${frontend}-undo45`,
						'baseline',
						false,
						baseline.bytes,
					);
					await history('Shift+z');
					await saved(
						`${frontend}-redo45`,
						'clockwise45',
						true,
						rotated.bytes,
					);
					return rotated.facts;
				},
			);
			const trace = await page.evaluate(() => window.__mainBlendSmoke);
			healthy(trace);
			save(`${frontend}-trace.json`, trace);
		} catch (problem) {
			try {
				await shot(`${frontend}-failure`);
			} catch {}
			throw problem;
		} finally {
			await context.close();
			page = null;
		}
	}
	const main = JSON.parse(
		fs.readFileSync(
			checked(JSON.parse(fs.readFileSync(receiptFile)).outputs_after),
		),
	).pkg.files.find((r) => r.path.endsWith('/paintcraft_web_bg.wasm'));
	assert(
		main &&
			result.served.some(
				(r) => r.file.endsWith('.wasm') && r.sha256 === main.sha256,
			),
		'actual bound Main served',
	);
	assert.deepEqual(result.pageErrors, []);
	assert.deepEqual(result.httpErrors, []);
	assert.equal(result.cases.length, mode === 'run' ? 4 * fronts.length : 0);
	result.status = mode === 'run' ? 'PASS' : 'LAYOUT_ONLY_NOT_ACCEPTANCE';
} catch (e) {
	error = String(e?.stack ?? e);
} finally {
	try {
		if (page && error) await shot('failure');
	} catch (e) {
		error ??= String(e);
	}
	try {
		if (browser) await browser.close();
	} catch (e) {
		error ??= String(e);
	}
	for (const socket of sockets) socket.destroy();
	try {
		if (server) await new Promise((resolve) => server.close(resolve));
	} catch (e) {
		error ??= String(e);
	}
	try {
		if (pre) assert.deepEqual(guard(), pre);
		assert.equal(sha(fs.readFileSync(coordinateFile)), coordinateSHA);
		result.source_unchanged = Boolean(pre);
	} catch (e) {
		postError = String(e?.stack ?? e);
	}
	if (error || postError) result.status = 'FAILED';
	result.error = error;
	result.post_source_error = postError;
	save('final.json', result);
}
assert.notEqual(result.status, 'FAILED', error ?? postError);
console.log(
	JSON.stringify({
		status: result.status,
		groups: result.cases.length,
		evidence: out,
	}),
);
