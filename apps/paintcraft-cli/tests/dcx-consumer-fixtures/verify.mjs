import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
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
const { initializeImageMagick, ImageMagick, PixelChannel, Magick } =
	await import(pathToFileURL(path.resolve(js)));
await initializeImageMagick(fs.readFileSync(wasm));
const palette = [
	[193, 255, 192],
	[17, 51, 93],
	[0, 7, 255],
	[12, 34, 56],
];
const indices = [0, 1, 0, 2, 3, 1];
const expected = indices.flatMap((i) => palette[i]);
const results = [];
for (const owner of ["cli", "mcp"]) {
	for (const mode of ["rgb24", "indexed-exact", "quantized256"]) {
		const filename = `${owner}-${mode}.dcx`;
		const data = fs.readFileSync(path.join(folder, filename));
		ImageMagick.readCollection(data, (images) => {
			assert.equal(images.length, 1);
			const image = images[0];
			assert.equal(image.width, 3);
			assert.equal(image.height, 2);
			assert.equal(image.density.x, 72);
			assert.equal(image.density.y, 72);
			const rgb = image.getPixels((pixels) =>
				Array.from(pixels.toByteArray(0, 0, 3, 2, "RGB")),
			);
			assert.deepEqual(rgb, expected);
			if (mode !== "rgb24") {
				const colors = Array.from({ length: 256 }, (_, i) => {
					const color = image.getColormapColor(i);
					return [color.r, color.g, color.b];
				});
				assert.deepEqual(colors, [
					...palette,
					...Array.from({ length: 252 }, () => [0, 0, 0]),
				]);
				const actual = image.getPixels((pixels) => {
					const slot = pixels.getChannelIndex(PixelChannel.Index);
					const samples = pixels.getReadOnlyArea(0, 0, 3, 2);
					return Array.from(
						{ length: 6 },
						(_, i) => samples[i * image.channelCount + slot],
					);
				});
				assert.deepEqual(actual, indices);
			}
		});
		results.push({
			filename,
			sha256: sha(data),
			exact_whole_container_pixels: true,
			exact_palette_and_indices: mode !== "rgb24",
		});
	}
}
console.log(
	JSON.stringify(
		{
			node: process.version,
			image_magick: Magick.imageMagickVersion,
			wrapper_sha256: sha(fs.readFileSync(js)),
			wasm_sha256: sha(fs.readFileSync(wasm)),
			results,
		},
		null,
		4,
	),
);
