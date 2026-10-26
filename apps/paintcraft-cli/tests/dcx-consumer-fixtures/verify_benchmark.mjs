import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
const [manifestPath, packagePath] = process.argv.slice(2);
const sha = (value) => crypto.createHash("sha256").update(value).digest("hex");
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
	ResourceLimits,
	PixelChannel,
} = await import(pathToFileURL(path.resolve(js)));
await initializeImageMagick(fs.readFileSync(wasm));
ResourceLimits.memory = 128n << 20n;
ResourceLimits.disk = 0n;
ResourceLimits.listLength = 8n;
const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
assert.equal(manifest.schema, "paintcraft.dcx-delivery/1");
const checked = [];
for (const item of manifest.cases) {
	const png = fs.readFileSync(item.source_png);
	assert.equal(sha(png), item.source_png_sha256);
	let source;
	ImageMagick.read(png, (image) => {
		assert.equal(image.width, item.width);
		assert.equal(image.height, item.height);
		image.getPixels((pixels) => {
			const rgba = pixels.toByteArray(
				0,
				0,
				item.width,
				item.height,
				"RGBA",
			);
			assert.equal(sha(rgba), item.source_rgba_sha256);
			for (let i = 3; i < rgba.length; i += 4) assert.equal(rgba[i], 255);
			source = Buffer.from(
				pixels.toByteArray(0, 0, item.width, item.height, "RGB"),
			);
		});
	});
	const bytes = fs.readFileSync(item.oracle_output);
	assert.equal(sha(bytes), item.oracle_sha256);
	ImageMagick.readCollection(bytes, (images) => {
		assert.equal(images.length, 1);
		const image = images[0];
		assert.equal(image.width, item.width);
		assert.equal(image.height, item.height);
		assert.equal(image.density.x, 72);
		assert.equal(image.density.y, 72);
		image.getPixels((pixels) => {
			assert.equal(
				sha(pixels.toByteArray(0, 0, item.width, item.height, "RGB")),
				sha(source),
			);
			if (item.mode === "indexed-exact") {
				const lookup = new Map();
				const palette = [];
				const expected = Buffer.alloc(item.width * item.height);
				for (let i = 0; i < expected.length; i++) {
					const color = Array.from(source.subarray(i * 3, i * 3 + 3));
					const key = color.join(",");
					if (!lookup.has(key)) {
						lookup.set(key, palette.length);
						palette.push(color);
					}
					expected[i] = lookup.get(key);
				}
				assert(palette.length <= 256);
				while (palette.length < 256) palette.push([0, 0, 0]);
				for (let i = 0; i < 256; i++) {
					const color = image.getColormapColor(i);
					assert.deepEqual([color.r, color.g, color.b], palette[i]);
				}
				const slot = pixels.getChannelIndex(PixelChannel.Index);
				assert(slot >= 0);
				const raw = pixels.getReadOnlyArea(
					0,
					0,
					item.width,
					item.height,
				);
				const actual = Buffer.alloc(expected.length);
				for (let i = 0; i < actual.length; i++)
					actual[i] = raw[i * image.channelCount + slot];
				assert.deepEqual(actual, expected);
			}
		});
	});
	checked.push({
		case: item.id,
		mode: item.mode,
		width: item.width,
		height: item.height,
		source_sha256: item.project_sha256,
		state_id: item.state_id,
		dcx_sha256: sha(bytes),
		rgb_sha256: sha(source),
		exact_whole_container_pixels: true,
		exact_palette_and_indices: item.mode === "indexed-exact",
	});
}
console.log(
	JSON.stringify(
		{
			node: process.version,
			image_magick: Magick.imageMagickVersion,
			wrapper_sha256: sha(fs.readFileSync(js)),
			wasm_sha256: sha(fs.readFileSync(wasm)),
			checked,
		},
		null,
		4,
	),
);
