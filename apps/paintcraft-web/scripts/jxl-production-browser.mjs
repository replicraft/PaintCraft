// Actual Studio picker/adoption proof; every attempt keeps source and failures.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import { createHash } from 'node:crypto';
import { fileURLToPath, pathToFileURL } from 'node:url';

const app = fileURLToPath(new URL('../', import.meta.url));
const root = path.resolve(app, '../..');
const dist = path.join(app, 'dist');
const base = process.env.CODEC_EVIDENCE_DIR;
if (!base || !process.env.PLAYWRIGHT_MODULE)
	throw new Error('Set CODEC_EVIDENCE_DIR and PLAYWRIGHT_MODULE');
fs.mkdirSync(base, { recursive: true });
const out = fs.mkdtempSync(path.join(base, 'attempt-'));
const evidence = {
	startedAt: new Date().toISOString(),
	checks: [],
	sources: {},
	artifacts: {},
};
function snapshot(directory, prefix, target) {
	for (const item of fs.readdirSync(directory, { withFileTypes: true })) {
		const file = path.join(directory, item.name);
		if (item.isDirectory()) snapshot(file, prefix, target);
		else {
			const bytes = fs.readFileSync(file);
			target[path.relative(prefix, file)] = {
				bytes: bytes.length,
				sha256: createHash('sha256').update(bytes).digest('hex'),
			};
		}
	}
}
snapshot(dist, dist, evidence.artifacts);
for (const name of [
	'src/paintcraft-web/importing/jxl.rs',
	'src/paintcraft-web/importing/jxl/bridge.rs',
	'src/paintcraft-web/importing/jxl/response.rs',
	'src/paintcraft-web/files.rs',
	'src/lib/codec-worker.js',
	'src/lib/codec-bridge.js',
	'scripts/jxl-production-browser.mjs',
]) {
	const bytes = fs.readFileSync(path.join(app, name));
	evidence.sources[name] = createHash('sha256').update(bytes).digest('hex');
	const dest = path.join(out, 'sources', name);
	fs.mkdirSync(path.dirname(dest), { recursive: true });
	fs.writeFileSync(dest, bytes);
}
const { chromium } = await import(
	pathToFileURL(process.env.PLAYWRIGHT_MODULE).href
);
let browser, server, active;
try {
	server = http.createServer((request, response) => {
		const pathname = new URL(request.url, 'http://localhost').pathname;
		if (pathname === '/__qa/hang.js') {
			response.setHeader('Content-Type', 'text/javascript');
			response.end('self.onmessage = () => {};');
			return;
		}
		if (pathname === '/__qa/trap.js') {
			response.setHeader('Content-Type', 'text/javascript');
			response.end(
				'self.onmessage = () => { throw Error("qualification trap"); };',
			);
			return;
		}
		if (pathname === '/__qa/malformed.js') {
			response.setHeader('Content-Type', 'text/javascript');
			response.end(
				'self.onmessage = () => self.postMessage(new ArrayBuffer(8));',
			);
			return;
		}
		const file = path.resolve(
			dist,
			`.${pathname.endsWith('/') ? `${pathname}index.html` : pathname}`,
		);
		if (!file.startsWith(`${dist}/`) || !fs.existsSync(file)) {
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
				'.json': 'application/json',
			}[path.extname(file)] ?? 'application/octet-stream',
		);
		fs.createReadStream(file).pipe(response);
	});
	await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
	const origin = `http://127.0.0.1:${server.address().port}`;
	browser = await chromium.launch({
		channel: 'chrome',
		headless: true,
		args: ['--enable-unsafe-swiftshader'],
	});
	const cdp = await browser.newBrowserCDPSession();
	evidence.browser = {
		version: browser.version(),
		cdp: await cdp.send('Browser.getVersion'),
	};
	async function open(mode) {
		const page = await browser.newPage({
			viewport: { width: 1360, height: 1000 },
			acceptDownloads: true,
		});
		active = page;
		page.on('pageerror', (error) =>
			(evidence.pageErrors ??= []).push(String(error)),
		);
		if (mode)
			await page.addInitScript((mode) => {
				const Original = window.Worker;
				window.__codecWorkers = 0;
				window.Worker = class extends Original {
					constructor(url, options) {
						if (String(url).includes('codec-worker-runtime')) {
							window.__codecWorkers++;
							url = `/__qa/${mode === 'stale' ? 'hang' : mode}.js`;
						}
						super(url, options);
					}
				};
			}, mode);
		await page.goto(origin, { waitUntil: 'networkidle' });
		await page.waitForFunction(
			() =>
				window.paintcraft && document.querySelector('#loading').hidden,
			null,
			{ timeout: 60000 },
		);
		await click(page, 1273, 20);
		await click(page, 1160, 98);
		return page;
	}
	async function click(page, x, y) {
		await page.mouse.click(x, y);
		await page.waitForTimeout(500);
	}
	async function pick(page, name) {
		const chooser = page.waitForEvent('filechooser');
		await click(page, 20, 46);
		await click(page, 80, 121);
		await (await chooser).setFiles(
			path.join(
				root,
				'crates/services/paintcraft-formats/tests/jxl-fixtures',
				name,
			),
		);
	}
	async function openProject(page) {
		const chooser = page.waitForEvent('filechooser');
		await click(page, 20, 46);
		await click(page, 80, 94);
		await (await chooser).setFiles(path.join(out, 'imported.pcnd'));
		await page.waitForFunction(
			() => document.title === 'imported.pcnd - PaintCraft',
			null,
			{ timeout: 15000 },
		);
		assert.equal(
			await page.evaluate(() => window.paintcraft.is_dirty()),
			false,
		);
	}
	async function save(page, name) {
		const event = page.waitForEvent('download', { timeout: 15000 });
		await page.keyboard.press('Meta+s');
		const download = await event;
		await download.saveAs(path.join(out, name));
		assert.equal(await download.failure(), null);
		return download.suggestedFilename();
	}
	const page = await open();
	await pick(page, 'rgba8.jxl');
	await page.waitForFunction(
		() => document.title === 'rgba8.jxl - PaintCraft',
		null,
		{ timeout: 30000 },
	);
	assert.equal(await page.evaluate(() => window.paintcraft.is_dirty()), true);
	assert.equal(
		await page.evaluate(
			() => window.paintcraft.codec_service_budget().reservedBytes,
		),
		0,
	);
	evidence.savedFilename = await save(page, 'imported.pcnd');
	assert.equal(evidence.savedFilename, 'rgba8.pcnd');
	await page.keyboard.press('Meta+z');
	await save(page, 'after-empty-undo.pcnd');
	await page.screenshot({ path: path.join(out, 'imported.png') });
	evidence.checks.push(
		'actual Studio JXL picker adopted a dirty independent project; PCND download used project suffix; completed charge released',
	);
	await page.close();
	const reopened = await open();
	await openProject(reopened);
	await save(reopened, 'reopened.pcnd');
	evidence.checks.push(
		'downloaded PCND reopened through actual Open as a clean project and downloaded again',
	);
	await reopened.close();
	const refused = await open();
	const originalTitle = await refused.title();
	await pick(refused, 'rgba16.jxl');
	await refused.waitForTimeout(1000);
	await refused.waitForFunction(
		() => window.paintcraft.codec_service_budget().pendingJobs === 0,
	);
	assert.equal(await refused.title(), originalTitle);
	assert.equal(
		await refused.evaluate(() => window.paintcraft.is_dirty()),
		false,
	);
	assert.equal(
		await refused.evaluate(
			() => window.paintcraft.codec_service_budget().quarantined,
		),
		false,
	);
	await pick(refused, 'rgba8.jxl');
	await refused.waitForFunction(
		() => document.title === 'rgba8.jxl - PaintCraft',
	);
	evidence.checks.push(
		'canonical unsupported depth kept clean source, released charge and allowed real valid retry',
	);
	await refused.close();
	for (const mode of ['hang', 'stale', 'trap', 'malformed']) {
		const failed = await open(mode);
		await openProject(failed);
		const title = await failed.title();
		await pick(failed, 'rgba8.jxl');
		if (mode === 'hang' || mode === 'stale') {
			await failed.waitForFunction(
				() =>
					window.paintcraft.codec_service_budget().pendingJobs === 1,
			);
			await failed.waitForTimeout(500);
			if (mode === 'hang') await failed.keyboard.press('Escape');
			else
				await failed.evaluate(() =>
					window.paintcraft.codec_service().setOwner({
						session: crypto.randomUUID(),
						state: crypto.randomUUID(),
						epoch: 1n,
						frontend: 3,
					}),
				);
		}
		await failed.waitForFunction(
			() => window.paintcraft.codec_service_budget().quarantined,
		);
		await failed.waitForTimeout(600);
		assert.equal(await failed.title(), title);
		assert.equal(
			await failed.evaluate(() => window.paintcraft.is_dirty()),
			false,
		);
		const count = await failed.evaluate(() => window.__codecWorkers);
		await pick(failed, 'rgba8.jxl');
		await failed.waitForTimeout(500);
		assert.equal(await failed.evaluate(() => window.__codecWorkers), count);
		assert.ok(
			(await failed.evaluate(
				() => window.paintcraft.codec_service_budget().reservedBytes,
			)) > 0,
		);
		await save(failed, `${mode}-preserved.pcnd`);
		await failed.screenshot({
			path: path.join(out, `${mode}-quarantine.png`),
		});
		evidence.checks.push(
			`${mode}: logical failure kept nonempty independent 3x2 project, retained quarantine charge, refused new Worker and allowed PCND download`,
		);
		await failed.close();
	}
	fs.writeFileSync(
		path.join(out, 'production.json'),
		`${JSON.stringify(evidence, null, 4)}\n`,
	);
	console.log(
		JSON.stringify({
			out,
			checks: evidence.checks,
			browser: evidence.browser,
			savedFilename: evidence.savedFilename,
		}),
	);
} catch (error) {
	evidence.failure = String(error.stack ?? error);
	if (active)
		await active
			.screenshot({ path: path.join(out, 'failure.png') })
			.catch(() => {});
	fs.writeFileSync(
		path.join(out, 'failure.json'),
		`${JSON.stringify(evidence, null, 4)}\n`,
	);
	console.error(out);
	throw error;
} finally {
	await browser?.close();
	if (server) await new Promise((resolve) => server.close(resolve));
}
