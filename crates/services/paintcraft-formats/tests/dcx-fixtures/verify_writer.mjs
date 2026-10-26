import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import assert from "node:assert/strict";
import { pathToFileURL } from "node:url";
const [folder, packagePath] = process.argv.slice(2);
const sha = (bytes) => crypto.createHash("sha256").update(bytes).digest("hex");
const js = path.join(packagePath, "dist/index.js");
const wasm = path.join(packagePath, "dist/x86/magick.wasm");
assert.equal(
	sha(fs.readFileSync(js)),
	"cf1db24f7e63f560118ef434fd581dc9ef66ad099dc417184765cd6076fb4a1b",
);
assert.equal(
	sha(fs.readFileSync(wasm)),
	"b7d6710436b150dc5bd1ca04d80b6b7614c54854d7470f888f7e69a2a41b00e4",
);
const {
	initializeImageMagick,
	ImageMagick,
	Magick,
	Quantum,
	ResourceLimits,
	PixelChannel,
} = await import(pathToFileURL(path.resolve(js)));
await initializeImageMagick(fs.readFileSync(wasm));
assert.equal(Quantum.depth, 8);
assert.equal(Quantum.max, 255);
ResourceLimits.memory = 128n << 20n;
ResourceLimits.disk = 0n;
ResourceLimits.listLength = 1024n;
const shapes = [
	[3, 2],
	[65, 2],
	[1, 5],
];
function palette(page) {
	const p = Array.from({ length: 256 }, (_, i) => [
		(i + page * 31) & 255,
		(i * 43 + page * 11) & 255,
		(255 - i + page * 7) & 255,
	]);
	p[17] = p[3].slice();
	return p;
}
function indices(w, h) {
	return Array.from(
		{ length: w * h },
		(_, i) => [3, 17, 192, 255, 0, 7][i % 6],
	);
}
function rgb(page, w, h) {
	return Array.from({ length: w * h }, (_, i) =>
		i % 5 === 0 ? [193, 255, 192] : [17, 51, (i + page * 43) & 255],
	).flat();
}
const results = [];
for (const kind of ["rgb", "indexed", "mixed"]) {
	const filename = path.join(folder, `${kind}.dcx`);
	const data = fs.readFileSync(filename);
	const before = sha(data);
	const pages = [];
	ImageMagick.readCollection(data, (images) => {
		assert.equal(images.length, 3);
		for (let i = 0; i < 3; i++) {
			const im = images[i];
			const [w, h] = shapes[i];
			const indexed = kind === "indexed" || (kind === "mixed" && i !== 1);
			assert.equal(im.width, w);
			assert.equal(im.height, h);
			assert.equal(im.density.x, 72 + i);
			assert.equal(im.density.y, 144 + i);
			const pal = palette(i);
			const idx = indices(w, h);
			const expected = indexed
				? idx.flatMap((j) => pal[j])
				: rgb(i, w, h);
			const colors = im.getPixels((p) =>
				Array.from(p.toByteArray(0, 0, w, h, "RGB")),
			);
			assert.deepEqual(colors, expected);
			let colorMapSha = null,
				indexSha = null;
			if (indexed) {
				assert.equal(im.colormapSize, 256);
				const actual = Array.from({ length: 256 }, (_, j) => {
					const p = im.getColormapColor(j);
					assert(p);
					return [p.r, p.g, p.b];
				});
				assert.deepEqual(actual, pal);
				const actualIndices = im.getPixels((p) => {
					const slot = p.getChannelIndex(PixelChannel.Index);
					assert(slot >= 0);
					const samples = p.getReadOnlyArea(0, 0, w, h);
					return Array.from(
						{ length: w * h },
						(_, j) => samples[j * im.channelCount + slot],
					);
				});
				assert.deepEqual(actualIndices, idx);
				colorMapSha = sha(Buffer.from(actual.flat()));
				indexSha = sha(Buffer.from(actualIndices));
			}
			pages.push({
				page: i,
				width: w,
				height: h,
				dpi: [im.density.x, im.density.y],
				indexed,
				rgb_sha256: sha(Buffer.from(colors)),
				palette_sha256: colorMapSha,
				index_sha256: indexSha,
			});
		}
	});
	assert.equal(sha(fs.readFileSync(filename)), before);
	results.push({
		filename: path.basename(filename),
		file_sha256: before,
		pages,
	});
}
const corrupt = fs.readFileSync(path.join(folder, "rgb.dcx"));
corrupt.writeUInt32LE(0xffffffff, 4);
let refused = false;
try {
	ImageMagick.readCollection(corrupt, () => {});
} catch (error) {
	refused = true;
}
assert(refused, "out-of-file first page must be refused");
console.log(
	JSON.stringify(
		{
			node: process.version,
			package: "@imagemagick/magick-wasm",
			version: "0.0.44",
			image_magick: Magick.imageMagickVersion,
			quantum_depth: Quantum.depth,
			archive_sha256:
				"808e876ed849de942b47dec268d4a8059e68fc2470aa895266bb36754eb968a7",
			wrapper_sha256: sha(fs.readFileSync(js)),
			wasm_sha256: sha(fs.readFileSync(wasm)),
			corrupt_first_offset_refused: refused,
			results,
		},
		null,
		4,
	),
);
