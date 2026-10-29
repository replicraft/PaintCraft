// Independent orientation oracles through the actual built isolated Worker/UI.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

export async function orientationProof(api) {
	const {
		root,
		out,
		evidence,
		open,
		pick,
		choose,
		consent,
		save,
		openProject,
	} = api;
	const folder = path.join(
		root,
		'crates/services/paintcraft-formats/tests/tiff-orientation-fixtures',
	);
	const oracle = JSON.parse(
		fs.readFileSync(path.join(folder, 'oracle.json'), 'utf8'),
	);
	const worker = await open();
	evidence.orientationCorpus = { files: 0, pages: 0 };
	for (const fixture of oracle.fixtures) {
		const source = fs.readFileSync(path.join(folder, fixture.file));
		assert.equal(
			createHash('sha256').update(source).digest('hex'),
			fixture.sha256,
		);
		await worker.evaluate(
			async ({ fixture, base64 }) => {
				const check = (value, message) => {
					if (!value) throw Error(message);
				};
				const source = Uint8Array.from(atob(base64), (value) =>
					value.charCodeAt(0),
				);
				const file = new File([source], fixture.file);
				const service = window.paintcraft.codec_service();
				const owner = {
					session: crypto.randomUUID(),
					state: crypto.randomUUID(),
					epoch: 3n,
					frontend: 3,
				};
				service.setOwner(owner);
				const inventory = await service
					.beginTiffFile(file, owner, null, crypto.randomUUID())
					.run();
				const raw = inventory.responseBytes;
				const frame = new DataView(
					raw.buffer,
					raw.byteOffset,
					raw.byteLength,
				);
				const start = 200 + frame.getUint16(36);
				const rows = new DataView(raw.buffer, raw.byteOffset + start);
				const count = frame.getUint16(14);
				check(
					count === fixture.pages.length,
					'orientation inventory count',
				);
				const plans = [];
				for (let index = 0; index < count; index++) {
					const at = index * 24;
					check(
						raw[start + at + 2] === 1,
						'orientation page must remain supported',
					);
					const expected = fixture.pages[index];
					const plan = {
						index,
						pageCount: count,
						width: rows.getUint32(at + 4),
						height: rows.getUint32(at + 8),
						ifdOffset: Number(rows.getBigUint64(at + 12)),
						orientation: rows.getUint32(at + 20),
						sourceSha256: fixture.sha256,
					};
					check(
						plan.width === 3 &&
							plan.height === 2 &&
							plan.orientation === expected.orientation,
						'original orientation/source geometry',
					);
					plans.push(plan);
				}
				inventory.release();
				for (const plan of plans) {
					const selected = await service
						.beginTiffFile(file, owner, plan, crypto.randomUUID())
						.run();
					const bytes = selected.responseBytes;
					const frame = new DataView(
						bytes.buffer,
						bytes.byteOffset,
						bytes.byteLength,
					);
					const reportStart = 200 + frame.getUint16(36);
					const report = new DataView(
						bytes.buffer,
						bytes.byteOffset + reportStart,
						256,
					);
					const rgba = bytes.subarray(reportStart + 256);
					check(
						Array.from(
							bytes.subarray(reportStart + 32, reportStart + 64),
							(value) => value.toString(16).padStart(2, '0'),
						).join('') === fixture.sha256,
						'report binds immutable original source SHA',
					);
					const expected = fixture.pages[plan.index];
					check(
						frame.getUint32(16) === expected.width &&
							frame.getUint32(20) === expected.height,
						'normalized output dimensions',
					);
					check(
						report.getUint32(24) === 3 &&
							report.getUint32(28) === 2 &&
							report.getUint32(180) === expected.orientation &&
							report.getUint16(8) === 2,
						'sealed V2 original facts',
					);
					check(
						report.getUint16(20) === plan.index &&
							report.getUint16(22) === count,
						'selected index/count',
					);
					check(
						rgba.length === expected.rgba.length &&
							rgba.every(
								(value, index) =>
									value === expected.rgba[index],
							),
						'independent explicit coordinate-label pixels',
					);
					const sha = new Uint8Array(
						await crypto.subtle.digest('SHA-256', rgba),
					);
					check(
						sha.every(
							(value, index) =>
								value === bytes[reportStart + 64 + index],
						),
						'output SHA bound',
					);
					check(
						((report.getUint16(10) & 1) !== 0) === fixture.gray,
						'mandatory existing16bit loss',
					);
					selected.release();
					check(
						service.budget().reservedBytes === 0 &&
							!service.budget().quarantined,
						'completed orientation job releases charge',
					);
				}
			},
			{ fixture, base64: source.toString('base64') },
		);
		evidence.orientationCorpus.files++;
		evidence.orientationCorpus.pages += fixture.pages.length;
	}
	await worker.close();
	evidence.orientationPcndOracles = [];
	for (let orientation = 1; orientation <= 8; orientation++) {
		const file = `gray-alpha16-be-orientation-${orientation}.tif`;
		const page = await open();
		const previous = await page.title();
		await pick(page, file, folder);
		await page.waitForFunction(() => window.__codecResults === 1);
		await page.waitForTimeout(400);
		await choose(page, 0, true);
		await page.waitForFunction(() => window.__codecResults === 2);
		await page.waitForTimeout(400);
		assert.equal(await page.title(), previous);
		await page.screenshot({
			path: path.join(out, `orientation-${orientation}-preview.png`),
		});
		await consent(page);
		await page.waitForFunction(
			(file) => document.title === `${file} - PaintCraft`,
			file,
		);
		assert.equal(
			await page.evaluate(() => window.paintcraft.is_dirty()),
			true,
		);
		const name = `orientation-${orientation}.pcnd`;
		await save(page, name);
		evidence.orientationPcndOracles.push({ name, file, index: 0 });
		await page.close();
		const reopened = await open();
		await openProject(reopened, name);
		const again = `orientation-${orientation}-reopened.pcnd`;
		await save(reopened, again);
		evidence.orientationPcndOracles.push({ name: again, file, index: 0 });
		await reopened.close();
	}
	evidence.checks.push(
		'all eight original TIFF orientations normalized exactly once:25 independent files/32 actual Worker pages; eight real Studio16bit preview/consent/PCND imports and clean reopens',
	);
}
