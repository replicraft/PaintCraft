// Actual whole-source TIFF Worker and Studio chooser/consent proof.
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
	throw new Error('Set CODEC_EVIDENCE_DIR and PLAYWRIGHT_MODULE');
fs.mkdirSync(base, { recursive: true });
const out = fs.mkdtempSync(path.join(base, 'attempt-'));
const evidence = {
	startedAt: new Date().toISOString(),
	status: 'started',
	host: { platform: process.platform, controllerArchitecture: process.arch },
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
	'src/paintcraft-web/importing/tiff.rs',
	'src/paintcraft-web/importing/tiff/bridge.rs',
	'src/paintcraft-web/importing/tiff/response.rs',
	'src/paintcraft-web/importing/tiff/review.rs',
	'src/paintcraft-web/files.rs',
	'src/paintcraft-web/importing.rs',
	'src/paintcraft-web/importing/jxl.rs',
	'src/paintcraft-web/app.rs',
	'src/paintcraft-web/app/presentation.rs',
	'src/paintcraft-web/lib.rs',
	'src/paintcraft-codec-worker/src/lib.rs',
	'src/paintcraft-codec-worker/src/tiff.rs',
	'src/paintcraft-codec-worker/src/wasm.rs',
	'src/lib/tiff-page-worker-protocol.js',
	'src/lib/codec-worker.js',
	'src/lib/codec-worker-runtime.js',
	'src/lib/codec-bridge.js',
	'scripts/tiff-production-browser.mjs',
]) {
	const bytes = fs.readFileSync(path.join(app, name));
	evidence.sources[name] = createHash('sha256').update(bytes).digest('hex');
	const dest = path.join(out, 'sources', name);
	fs.mkdirSync(path.dirname(dest), { recursive: true });
	fs.writeFileSync(dest, bytes);
}
function retainSource(file) {
	const name = path.relative(root, file),
		bytes = fs.readFileSync(file);
	evidence.sources[name] = createHash('sha256').update(bytes).digest('hex');
	const target = path.join(out, 'sources/repository', name);
	fs.mkdirSync(path.dirname(target), { recursive: true });
	fs.writeFileSync(target, bytes);
}
function retainTree(directory) {
	for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
		const file = path.join(directory, entry.name);
		if (entry.isDirectory()) retainTree(file);
		else retainSource(file);
	}
}
retainSource(path.join(root, 'Cargo.lock'));
retainSource(path.join(root, 'Cargo.toml'));
retainSource(path.join(root, 'rust-toolchain.toml'));
retainSource(
	path.join(root, 'crates/services/paintcraft-formats/src/registry.rs'),
);
retainSource(
	path.join(root, 'crates/presentation/paintcraft-ui/src/page_import.rs'),
);
retainSource(
	path.join(root, 'crates/services/paintcraft-formats/src/tiff_pages.rs'),
);
retainTree(
	path.join(root, 'crates/services/paintcraft-formats/src/tiff_pages'),
);
retainTree(
	path.join(root, 'crates/services/paintcraft-formats/tests/tiff-fixtures'),
);
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
		page.on('pageerror', (error) => {
			evidence.pageErrors ??= [];
			evidence.pageErrors.push(String(error));
		});
		await page.addInitScript((mode) => {
			localStorage.setItem('showWelcomeScreen', 'false');
			const originalRead = File.prototype.arrayBuffer;
			const reads = new WeakMap();
			window.__tiffReadAttempts = 0;
			File.prototype.arrayBuffer = async function () {
				if (/\.tiff?$/i.test(this.name)) window.__tiffReadAttempts++;
				const bytes = await originalRead.call(this);
				const count = (reads.get(this) ?? 0) + 1;
				reads.set(this, count);
				if (
					mode === 'changed' &&
					/\.tiff?$/i.test(this.name) &&
					count > 1
				) {
					const changed = new Uint8Array(bytes.slice(0));
					changed[0] ^= 1;
					return changed.buffer;
				}
				return bytes;
			};

			const Original = window.Worker;
			window.__codecWorkers = 0;
			window.__codecResults = 0;
			window.Worker = class extends Original {
				constructor(url, options) {
					if (String(url).includes('codec-worker-runtime')) {
						window.__codecWorkers++;
						if (
							mode &&
							mode !== 'changed' &&
							window.__codecWorkers > 1
						)
							url = `/__qa/${mode === 'stale' ? 'hang' : mode}.js`;
					}
					super(url, options);
					this.addEventListener('message', (event) => {
						if (event.data?.kind === 'result')
							window.__codecResults++;
					});
				}
			};
		}, mode ?? null);
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
				'crates/services/paintcraft-formats/tests/tiff-fixtures',
				name,
			),
		);
	}
	async function openProject(page, name = 'imported.pcnd') {
		const chooser = page.waitForEvent('filechooser');
		await click(page, 20, 46);
		await click(page, 80, 94);
		await (await chooser).setFiles(path.join(out, name));
		await page.waitForFunction(
			(name) => document.title === `${name} - PaintCraft`,
			name,
			{ timeout: 15000 },
		);
		assert.equal(
			await page.evaluate(() => window.paintcraft.is_dirty()),
			false,
		);
	}
	async function save(page, name) {
		await frame(page);
		const event = page.waitForEvent('download', { timeout: 15000 });
		await page.keyboard.press('Meta+s');
		const download = await event;
		await download.saveAs(path.join(out, name));
		assert.equal(await download.failure(), null);
		return download.suggestedFilename();
	}
	const fixtures = path.join(
		root,
		'crates/services/paintcraft-formats/tests/tiff-fixtures',
	);
	const oracle = JSON.parse(
		fs.readFileSync(path.join(fixtures, 'oracle.json'), 'utf8'),
	);
	evidence.fixtureOracleSha256 = createHash('sha256')
		.update(fs.readFileSync(path.join(fixtures, 'oracle.json')))
		.digest('hex');
	evidence.pcndOracles = [];
	const eight = 'rgba8-le-strips-none.tif',
		sixteen = 'unsupported-first-rgba16.tif';

	function artifact(name, file, index) {
		evidence.pcndOracles.push({ name, file, index });
	}
	// First exercise every independent source through the actual built isolated
	// Wasm graph. UI publication below separately invokes mandatory Rust checks.
	const workerPage = await open();
	for (const fixture of oracle.fixtures) {
		evidence.currentFixture = fixture.file;
		const source = fs.readFileSync(path.join(fixtures, fixture.file));
		assert.equal(
			createHash('sha256').update(source).digest('hex'),
			fixture.sha256,
		);
		const result = await workerPage.evaluate(
			async ({ base64, fixture }) => {
				const check = (value, message) => {
					if (!value) throw Error(message);
				};
				const file = new File(
					[
						Uint8Array.from(atob(base64), (value) =>
							value.charCodeAt(0),
						),
					],
					fixture.file,
				);
				const owner = {
					session: crypto.randomUUID(),
					state: crypto.randomUUID(),
					epoch: 3n,
					frontend: 3,
				};
				const service = window.paintcraft.codec_service();
				service.setOwner(owner);
				async function run(ticket, stage) {
					try {
						return await ticket.run();
					} catch (error) {
						throw Error(
							`${fixture.file} ${stage}: ${error.code ?? 'untyped'} ${error.message}`,
						);
					}
				}
				if (!fixture.whole_file_qualified) {
					let refusal;
					try {
						await service
							.beginTiffFile(
								file,
								owner,
								null,
								crypto.randomUUID(),
							)
							.run();
					} catch (error) {
						refusal = error;
					}
					check(
						refusal?.code === 'CapabilityUnsupported',
						'unqualified TIFF container must be explicitly refused',
					);
					check(
						service.budget().reservedBytes === 0 &&
							!service.budget().quarantined,
						'completed container refusal must release the transient lease',
					);
					return { pages: 0, decoded: 0, refused: true };
				}
				const inventory = await run(
					service.beginTiffFile(
						file,
						owner,
						null,
						crypto.randomUUID(),
					),
					'inventory',
				);
				check(
					inventory.requiresRustValidation,
					'Rust validation must remain mandatory',
				);
				const bytes = inventory.responseBytes,
					view = new DataView(
						bytes.buffer,
						bytes.byteOffset,
						bytes.byteLength,
					);
				const pages = view.getUint16(14),
					start = 200 + view.getUint16(36),
					rows = new DataView(bytes.buffer, bytes.byteOffset + start);
				check(pages === fixture.pages.length, 'actual TIFF page count');
				const sourceSha256 = Array.from(
					inventory.sourceSha256,
					(value) => value.toString(16).padStart(2, '0'),
				).join('');
				check(sourceSha256 === fixture.sha256, 'whole source SHA');
				const plans = [];
				for (let index = 0; index < pages; index++) {
					const at = index * 24,
						supported = bytes[start + at + 2] === 1,
						sample = fixture.pages[index];
					check(
						rows.getUint16(at) === index &&
							supported === sample.qualified_integer_profile,
						'per-page profile inventory',
					);
					if (supported)
						plans.push({
							index,
							pageCount: pages,
							width: rows.getUint32(at + 4),
							height: rows.getUint32(at + 8),
							ifdOffset: Number(rows.getBigUint64(at + 12)),
							sourceSha256,
						});
				}
				inventory.release();
				check(
					service.budget().reservedBytes === 0,
					'inventory transient lease retirement',
				);
				for (const plan of plans) {
					const selected = await run(
						service.beginTiffFile(
							file,
							owner,
							plan,
							crypto.randomUUID(),
						),
						`selected page ${plan.index}`,
					);
					const output = selected.responseBytes,
						frame = new DataView(
							output.buffer,
							output.byteOffset,
							output.byteLength,
						);
					const version = frame.getUint16(36),
						reportLength = frame.getUint32(32),
						reportStart = 200 + version;
					const report = new DataView(
						output.buffer,
						output.byteOffset + reportStart,
						reportLength,
					);
					const pixels = output.subarray(reportStart + reportLength),
						sample = fixture.pages[plan.index];
					const reportBytes = output.subarray(
						reportStart,
						reportStart + reportLength,
					);
					const reportSource = Array.from(
						reportBytes.subarray(32, 64),
						(value) => value.toString(16).padStart(2, '0'),
					).join('');
					const pixelSha = new Uint8Array(
						await crypto.subtle.digest('SHA-256', pixels),
					);
					check(
						selected.requiresRustValidation &&
							reportSource === fixture.sha256 &&
							Number(report.getBigUint64(12)) === file.size,
						'report binds the original whole File',
					);
					check(
						pixelSha.every(
							(value, index) => value === reportBytes[64 + index],
						),
						'report binds exact decoded output SHA',
					);
					check(
						reportLength === 256 &&
							report.getUint16(20) === plan.index &&
							report.getUint16(22) === pages &&
							Number(report.getBigUint64(96)) === plan.ifdOffset,
						'sealed selected report binds inventory',
					);
					check(
						pixels.length === sample.expected_rgba8.length &&
							pixels.every(
								(value, index) =>
									value === sample.expected_rgba8[index],
							),
						'independent libtiff raw RGBA8 oracle',
					);
					check(
						((report.getUint16(10) & 1) !== 0) ===
							(sample.bits === 16),
						'mandatory depth loss',
					);
					check(
						((report.getUint16(10) & 8) !== 0) === pages > 1,
						'mandatory other-pages loss',
					);
					selected.release();
					check(
						service.budget().reservedBytes === 0,
						'selected transient lease retirement',
					);
				}
				return { pages, decoded: plans.length };
			},
			{ base64: source.toString('base64'), fixture },
		);
		evidence.workerCorpus ??= {
			files: 0,
			inventoriedPages: 0,
			decodedPages: 0,
			refusedContainers: 0,
		};
		evidence.workerCorpus.files++;
		evidence.workerCorpus.inventoriedPages += result.pages;
		evidence.workerCorpus.decodedPages += result.decoded;
		evidence.workerCorpus.refusedContainers += Number(
			result.refused ?? false,
		);
		evidence.checks.push(
			result.refused
				? `${fixture.file}: actual Worker explicitly refused the unqualified container, released its charge and allowed subsequent jobs`
				: `${fixture.file}: actual whole-source Worker inventoried ${result.pages} pages and decoded ${result.decoded} qualified selections against independent raw samples`,
		);
	}
	await workerPage.close();
	async function key(page, value) {
		await page.keyboard.press(value);
		await page.waitForTimeout(150);
	}
	async function choose(page, index, unsupportedFirst = false) {
		// The modal starts with no selected page. Move focus to the intended
		// enabled radio, then explicitly activate its Import button.
		await key(page, 'Tab');
		if (index === 1 && !unsupportedFirst) await key(page, 'Tab');
		await key(page, 'Space');
		const remaining = index === 0 && !unsupportedFirst ? 3 : 2;
		for (let i = 0; i < remaining; i++) await key(page, 'Tab');
		await key(page, 'Enter');
	}
	async function consent(page) {
		await key(page, 'Tab');
		await key(page, 'Space');
		await key(page, 'Tab');
		await key(page, 'Enter');
	}
	async function frame(page) {
		await page.evaluate(
			() =>
				new Promise((resolve) =>
					requestAnimationFrame(() =>
						requestAnimationFrame(() => resolve()),
					),
				),
		);
	}
	const first = await open();
	const untitled = await first.title();
	await pick(first, eight);
	await first.waitForFunction(() => window.__codecResults === 1);
	await first.waitForTimeout(600);
	assert.equal(await first.title(), untitled);
	assert.equal(await first.evaluate(() => window.__codecWorkers), 1);
	assert.equal(await first.evaluate(() => window.__tiffReadAttempts), 1);
	assert.equal(
		await first.evaluate(
			() => window.paintcraft.codec_service_budget().reservedBytes,
		),
		0,
	);
	await first.screenshot({
		path: path.join(out, 'chooser-no-implicit-page.png'),
	});
	await choose(first, 0);
	await first.waitForFunction(() => window.__codecResults === 2);
	await first.waitForTimeout(600);
	assert.equal(await first.title(), untitled);
	await first.screenshot({
		path: path.join(out, 'selected8-preview-before-consent.png'),
	});
	await consent(first);
	await first.waitForFunction(
		(name) => document.title === `${name} - PaintCraft`,
		eight,
	);
	assert.equal(
		await first.evaluate(() => window.paintcraft.is_dirty()),
		true,
	);
	assert.equal(await first.evaluate(() => window.__tiffReadAttempts), 2);
	await save(first, 'baseline8.pcnd');
	artifact('baseline8.pcnd', eight, 0);
	await first.keyboard.press('Meta+z');
	await save(first, 'empty-undo8.pcnd');
	artifact('empty-undo8.pcnd', eight, 0);
	await first.close();
	evidence.checks.push(
		'actual Studio picker required explicit page selection and preview consent before a dirty exact8 copy; empty imported Undo retained its pixels',
	);
	const second = await open();
	await openProject(second, 'baseline8.pcnd');
	await pick(second, eight);
	await second.waitForFunction(() => window.__codecResults === 1);
	await second.waitForTimeout(600);
	await choose(second, 1);
	await second.waitForFunction(() => window.__codecResults === 2);
	await second.waitForTimeout(600);
	await consent(second);
	await second.waitForFunction(
		(name) => document.title === `${name} - PaintCraft`,
		eight,
	);
	await save(second, 'same-source-page1.pcnd');
	artifact('same-source-page1.pcnd', eight, 1);
	await second.close();
	evidence.checks.push(
		'a different explicit page from the same source produced its own editable copy and PCND artifact',
	);
	const integer = await open();
	await openProject(integer, 'baseline8.pcnd');
	const oldTitle = await integer.title();
	await pick(integer, sixteen);
	await integer.waitForFunction(() => window.__codecResults === 1);
	await integer.waitForTimeout(600);
	assert.equal(await integer.title(), oldTitle);
	await integer.screenshot({
		path: path.join(out, 'unsupported-first-valid-later.png'),
	});
	await integer.keyboard.press('Escape');
	await save(integer, 'inventory-cancel.pcnd');
	artifact('inventory-cancel.pcnd', eight, 0);
	await pick(integer, sixteen);
	await integer.waitForFunction(() => window.__codecResults === 2);
	await integer.waitForTimeout(600);
	await choose(integer, 1, true);
	await integer.waitForFunction(() => window.__codecResults === 3);
	await integer.waitForTimeout(600);
	assert.equal(await integer.title(), oldTitle);
	assert.equal(
		await integer.evaluate(() => window.paintcraft.is_dirty()),
		false,
	);
	await integer.screenshot({
		path: path.join(out, 'selected16-preview-before-consent.png'),
	});
	await integer.keyboard.press('Escape');
	await integer.waitForFunction(
		() => window.paintcraft.codec_service_budget().reservedBytes === 0,
	);
	await save(integer, 'preview-cancel.pcnd');
	artifact('preview-cancel.pcnd', eight, 0);
	await pick(integer, sixteen);
	await integer.waitForFunction(() => window.__codecResults === 4);
	await integer.waitForTimeout(600);
	await choose(integer, 1, true);
	await integer.waitForFunction(() => window.__codecResults === 5);
	await integer.waitForTimeout(600);
	await consent(integer);
	await integer.waitForFunction(
		(name) => document.title === `${name} - PaintCraft`,
		sixteen,
	);
	assert.equal(
		await integer.evaluate(() => window.paintcraft.is_dirty()),
		true,
	);
	await save(integer, 'selected16.pcnd');
	artifact('selected16.pcnd', sixteen, 1);
	await integer.close();
	const reopened = await open();
	await openProject(reopened, 'selected16.pcnd');
	await save(reopened, 'reopened16.pcnd');
	artifact('reopened16.pcnd', sixteen, 1);
	await reopened.close();
	evidence.checks.push(
		'unsupported first page did not block qualified16 later page; chooser/preview Cancel preserved the old clean project; explicit consent adopted >>8 pixels and clean PCND reopen',
	);
	for (const mode of ['changed', 'hang', 'stale', 'trap', 'malformed']) {
		const failed = await open(mode);
		await openProject(failed, 'baseline8.pcnd');
		const title = await failed.title();
		await pick(failed, sixteen);
		await failed.waitForFunction(() => window.__codecResults === 1);
		await failed.waitForTimeout(600);
		await choose(failed, 1, true);
		if (mode === 'hang' || mode === 'stale') {
			await failed.waitForFunction(
				() =>
					window.__codecWorkers === 2 &&
					window.paintcraft.codec_service_budget().pendingJobs === 1,
			);
			if (mode === 'hang') await failed.keyboard.press('Escape');
			else
				await failed.evaluate(() =>
					window.paintcraft.codec_service().setOwner({
						session: crypto.randomUUID(),
						state: crypto.randomUUID(),
						epoch: 7n,
						frontend: 3,
					}),
				);
		}
		if (mode === 'changed') {
			await failed.waitForFunction(
				() =>
					window.__tiffReadAttempts === 2 &&
					window.paintcraft.codec_service_budget().pendingJobs === 0,
			);
			assert.equal(await failed.evaluate(() => window.__codecWorkers), 1);
			assert.equal(
				await failed.evaluate(
					() => window.paintcraft.codec_service_budget().quarantined,
				),
				false,
			);
		} else
			await failed.waitForFunction(
				() => window.paintcraft.codec_service_budget().quarantined,
			);
		await failed.waitForTimeout(400);
		assert.equal(await failed.title(), title);
		assert.equal(
			await failed.evaluate(() => window.paintcraft.is_dirty()),
			false,
		);
		await save(failed, `${mode}-preserved.pcnd`);
		artifact(`${mode}-preserved.pcnd`, eight, 0);
		await failed.screenshot({
			path: path.join(out, `${mode}-preserved.png`),
		});
		await failed.close();
		evidence.checks.push(
			`${mode}: full-source/owner/unknown completion guard kept the prior document and PCND pixels without publication`,
		);
	}
	evidence.status = 'passed';
	fs.writeFileSync(
		path.join(out, 'pcnd-oracles.json'),
		`${JSON.stringify(evidence.pcndOracles, null, 4)}\n`,
	);
	fs.writeFileSync(
		path.join(out, 'production.json'),
		`${JSON.stringify(evidence, null, 4)}\n`,
	);
	console.log(
		JSON.stringify({
			out,
			checks: evidence.checks,
			pcndArtifacts: evidence.pcndOracles.length,
			browser: evidence.browser,
		}),
	);
} catch (error) {
	evidence.status = 'failed';
	evidence.failure = String(error.stack ?? error);
	if (active)
		await active
			.screenshot({ path: path.join(out, 'failure.png') })
			.catch(() => {});
	fs.writeFileSync(
		path.join(out, 'production.json'),
		`${JSON.stringify(evidence, null, 4)}\n`,
	);
	throw error;
} finally {
	await browser?.close();
	await new Promise((resolve) => server?.close(resolve) ?? resolve());
}
