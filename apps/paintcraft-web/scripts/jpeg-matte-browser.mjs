// Real canvas UI, color editing and JPEG downloads. No Rust test hooks.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const app = fileURLToPath(new URL('../', import.meta.url));
const root = path.resolve(app, '../..');
const dist = path.join(app, 'dist');
const base = process.env.CODEC_EVIDENCE_DIR;
if (!base || !process.env.PLAYWRIGHT_MODULE)
	throw Error('Set evidence directory and existing Playwright module');
fs.mkdirSync(base, { recursive: true });
const out = fs.mkdtempSync(path.join(base, 'attempt-'));
const phase = process.env.MATTE_BROWSER_PHASE ?? 'finish';
const evidence = {
	status: 'started',
	phase,
	sources: {},
	assets: {},
	checks: [],
	startedAt: new Date().toISOString(),
};
const sha = (bytes) => createHash('sha256').update(bytes).digest('hex');
function inventory(folder) {
	for (const entry of fs.readdirSync(folder, { withFileTypes: true })) {
		const file = path.join(folder, entry.name);
		if (entry.isDirectory()) inventory(file);
		else
			evidence.assets[path.relative(dist, file)] = {
				bytes: fs.statSync(file).size,
				sha256: sha(fs.readFileSync(file)),
			};
	}
}
inventory(dist);
for (const name of [
	'Cargo.lock',
	'apps/paintcraft-web/src/paintcraft-web/app.rs',
	'apps/paintcraft-web/src/paintcraft-web/app/exporting.rs',
	'apps/paintcraft-web/src/paintcraft-web/files.rs',
	'apps/paintcraft-web/src/paintcraft-web/files/flat_export.rs',
	'apps/paintcraft-web/src/paintcraft-web/text_ui.rs',
	'crates/services/paintcraft-formats/src/image.rs',
	'crates/services/paintcraft-formats/src/image/jpeg_encode.rs',
	'crates/services/paintcraft-formats/src/export.rs',
	'crates/foundation/paintcraft-document/src/lib.rs',
	'crates/foundation/paintcraft-document/src/source_metadata.rs',
	'crates/foundation/paintcraft-document/src/source_metadata/wire.rs',
	'crates/foundation/paintcraft-document/src/source_metadata/icc.rs',
	'crates/services/paintcraft-formats/src/pcnd/binary.rs',
	'crates/services/paintcraft-formats/src/pcnd/binary/source_metadata.rs',
	'crates/presentation/paintcraft-ui/src/jpeg_export.rs',
	'apps/paintcraft-web/scripts/jpeg-matte-browser.mjs',
])
	evidence.sources[name] = sha(fs.readFileSync(path.join(root, name)));
const consumers = process.env.MATTE_BROWSER_CONSUMERS
	? path.resolve(process.env.MATTE_BROWSER_CONSUMERS)
	: path.join(root, 'docs/validation/jpeg-matte/consumer-final');
const source = path.join(consumers, 'source.pcnd');
evidence.input = {
	path: path.relative(root, source),
	sha256: sha(fs.readFileSync(source)),
};
const { chromium } = await import(
	pathToFileURL(process.env.PLAYWRIGHT_MODULE).href
);
let browser, server, page;
try {
	server = http.createServer((request, response) => {
		const pathname = new URL(request.url, 'http://localhost').pathname;
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
	browser = await chromium.launch({
		channel: 'chrome',
		headless: true,
		args: ['--enable-unsafe-swiftshader'],
	});
	evidence.browser = browser.version();
	page = await browser.newPage({
		viewport: { width: 1360, height: 1000 },
		acceptDownloads: true,
	});
	await page.addInitScript(() =>
		localStorage.setItem('showWelcomeScreen', 'false'),
	);
	page.on('pageerror', (error) => {
		evidence.pageErrors ??= [];
		evidence.pageErrors.push(String(error));
	});
	const click = async (x, y) => {
		await page.mouse.click(x, y);
		await page.waitForTimeout(400);
	};
	await page.goto(`http://127.0.0.1:${server.address().port}/`, {
		waitUntil: 'networkidle',
	});
	await page.waitForFunction(
		() => window.paintcraft && document.querySelector('#loading').hidden,
		null,
		{ timeout: 60000 },
	);
	await click(1273, 20);
	await click(1160, 98);
	const chooser = page.waitForEvent('filechooser');
	await click(20, 46);
	await click(80, 94);
	await (await chooser).setFiles(source);
	await page.waitForFunction(
		() => document.title === 'source.pcnd - PaintCraft',
	);
	const title = await page.title();
	assert.equal(
		await page.evaluate(() => window.paintcraft.is_dirty()),
		false,
	);
	async function download(name, trigger) {
		const pending = page.waitForEvent('download');
		await trigger();
		const item = await pending;
		await item.saveAs(path.join(out, name));
		assert.equal(await item.failure(), null);
		evidence.checks.push({
			download: name,
			sha256: sha(fs.readFileSync(path.join(out, name))),
		});
	}
	await download('before.pcnd', () => page.keyboard.press('Meta+s'));
	await click(20, 46);
	if (phase === 'menu') {
		await page.screenshot({ path: path.join(out, 'menu.png') });
		evidence.status = 'calibration-only';
	} else {
		await click(80, 242);
		await page.screenshot({ path: path.join(out, 'options.png') });
		if (phase === 'options') evidence.status = 'calibration-only';
		else {
			const c = JSON.parse(process.env.MATTE_BROWSER_COORDINATES ?? '{}');
			assert.ok(
				c.format && c.jpeg && c.color,
				'Set observed format/JPEG/color coordinates',
			);
			await click(...c.format);
			await click(...c.jpeg);
			await page.screenshot({ path: path.join(out, 'jpeg-default.png') });
			if (phase === 'picker') {
				await click(...c.color);
				await page.screenshot({ path: path.join(out, 'picker.png') });
				evidence.status = 'calibration-only';
			} else {
				assert.ok(
					c.download && c.channels && c.outside,
					'Set observed download/RGB coordinates',
				);
				await download('white.jpg', () => click(...c.download));
				await click(20, 46);
				await click(80, 242);
				await click(...c.format);
				await click(...c.jpeg);
				await click(...c.color);
				for (let index = 0; index < 3; index++) {
					await page.mouse.dblclick(...c.channels[index]);
					await page.waitForTimeout(150);
					await page.keyboard.press('Meta+a');
					await page.keyboard.type(String([32, 160, 224][index]));
					await page.keyboard.press('Enter');
					await page.waitForTimeout(150);
				}
				await click(...c.outside);
				await page.screenshot({
					path: path.join(out, 'custom-matte.png'),
				});
				await download('color.jpg', () => click(...c.download));
				assert.equal(await page.title(), title);
				assert.equal(
					await page.evaluate(() => window.paintcraft.is_dirty()),
					false,
				);
				await download('after.pcnd', () =>
					page.keyboard.press('Meta+s'),
				);
				assert.deepEqual(
					fs.readFileSync(path.join(out, 'before.pcnd')),
					fs.readFileSync(path.join(out, 'after.pcnd')),
				);
				for (const name of ['white', 'color'])
					assert.deepEqual(
						fs.readFileSync(path.join(out, `${name}.jpg`)),
						fs.readFileSync(
							path.join(consumers, `cli-${name}.jpg`),
						),
					);
				evidence.status = 'passed';
				evidence.checks.push(
					'Real UI default/edited matte downloads match independently checked CLI/MCP JPEGs byte-for-byte; title/dirty state and before/after PCND unchanged',
				);
			}
		}
	}
} catch (error) {
	evidence.status = 'failed';
	evidence.failure = String(error.stack ?? error);
	await page
		?.screenshot({ path: path.join(out, 'failure.png') })
		.catch(() => {});
	throw error;
} finally {
	evidence.finishedAt = new Date().toISOString();
	fs.writeFileSync(
		path.join(out, 'production.json'),
		`${JSON.stringify(evidence, null, 4)}\n`,
	);
	console.log(JSON.stringify({ out, status: evidence.status, phase }));
	await browser?.close();
	await new Promise((resolve) => server?.close(resolve) ?? resolve());
}
