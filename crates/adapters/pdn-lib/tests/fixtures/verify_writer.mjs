import fs from "node:fs";
import vm from "node:vm";
import crypto from "node:crypto";
import assert from "node:assert/strict";
import path from "node:path";
import { pathToFileURL } from "node:url";
const [folder, codecPath, packagePath] = process.argv.slice(2);
assert(
	folder && codecPath && packagePath,
	"usage: node verify_writer.mjs <vectors-dir> <PdnDocumentCodec.js> <ms-nrbf-package-dir>",
);
const dist = path.join(packagePath, "dist");
const sourceHash = crypto.createHash("sha256");
for (const name of fs
	.readdirSync(dist)
	.filter((name) => name.endsWith(".js"))
	.sort()) {
	sourceHash.update(name + "\0");
	sourceHash.update(fs.readFileSync(path.join(dist, name)));
	sourceHash.update("\0");
}
assert.equal(
	sourceHash.digest("hex"),
	"bfa2b3b5068e121eede69a5fe70e47504635ec1cd75dd8e4e50bf4c154b1939d",
);
const Nrbf = await import(pathToFileURL(path.resolve(dist, "index.js")));
assert.equal(
	JSON.parse(fs.readFileSync(path.join(packagePath, "package.json"))).version,
	"0.1.0",
);
const source = fs.readFileSync(codecPath, "utf8");
assert.equal(
	crypto.createHash("sha256").update(source).digest("hex"),
	"e8f302d1d1f2b28e7c9c904ee2b1a69b093c331078b9b8f4b3fbf11e28cf4bf7",
);
const context = vm.createContext({
	Nrbf,
	TextEncoder,
	TextDecoder,
	Blob,
	Response,
	CompressionStream,
	DecompressionStream,
	Uint8Array,
	Uint8ClampedArray,
	DataView,
	BigInt,
	LayerProperties: {
		getBlendMode(value) {
			assert.equal(value, 0);
			return { value };
		},
	},
	ImageData: class {
		constructor(data, width, height) {
			this.data = data;
			this.width = width;
			this.height = height;
		}
	},
	document: {
		createElement(kind) {
			assert.equal(kind, "canvas");
			return {
				getContext(kind) {
					assert.equal(kind, "2d");
					return {
						putImageData(image) {
							this.image = image;
						},
					};
				},
			};
		},
	},
});
vm.runInContext(source + "\nglobalThis.Codec=PdnDocumentCodec;", context);
// Replace only the absent DOM backing store, retaining original BGRA conversion.
context.document.createElement = () => {
	const canvas = {};
	canvas.getContext = () => ({
		putImageData(image) {
			canvas.pixels = Array.from(image.data);
		},
	});
	return canvas;
};
const sha = (bytes) => crypto.createHash("sha256").update(bytes).digest("hex");
const results = [];
for (const count of [1, 2, 64]) {
	const name = `layers-${count}.pdn`;
	const bytes = fs.readFileSync(`${folder}/${name}`);
	const before = sha(bytes);
	const xmlLength = bytes[4] + (bytes[5] << 8) + (bytes[6] << 16);
	const start = xmlLength + 9;
	const root = Nrbf.deserialize(bytes.subarray(start));
	assert.equal(root.typeName, "PaintDotNet.Document");
	assert.equal(
		root.libraryName,
		"PaintDotNet.Data, Version=5.112.9563.32325, Culture=neutral, PublicKeyToken=null",
	);
	assert.equal(root.members.width, 3);
	assert.equal(root.members.height, 2);
	assert.equal(root.members.userMetadataItems, null);
	assert.deepEqual(
		Object.values(root.members.savedWith.members),
		[0, 1, 0, 0],
	);
	const list = root.members.layers.members;
	assert.equal(list.parent, root);
	assert.equal(list["ArrayList+_size"], count);
	const objects = list["ArrayList+_items"];
	assert.equal(objects.length, count);
	const decoded = await context.Codec.decode(new Uint8Array(bytes));
	assert.equal(decoded.width, 3);
	assert.equal(decoded.height, 2);
	assert.equal(decoded.layers.length, count);
	const layers = [];
	for (let i = 0; i < count; i++) {
		const object = objects[i];
		assert.equal(object.typeName, "PaintDotNet.BitmapLayer");
		assert.equal(
			object.libraryName,
			"PaintDotNet.Core, Version=5.112.9563.32325, Culture=neutral, PublicKeyToken=null",
		);
		for (const [value, typeName] of [
			[
				object.members.properties,
				"PaintDotNet.BitmapLayer+BitmapLayerProperties",
			],
			[object.members.surface, "PaintDotNet.Surface"],
			[
				object.members["Layer+properties"],
				"PaintDotNet.Layer+LayerProperties",
			],
			[
				object.members.properties.members.blendOp,
				"PaintDotNet.UserBlendOps+NormalBlendOp",
			],
			[object.members.surface.members.scan0, "PaintDotNet.MemoryBlock"],
			[
				object.members["Layer+properties"].members.blendMode,
				"PaintDotNet.LayerBlendMode",
			],
		]) {
			assert.equal(value.typeName, typeName);
			assert.equal(
				value.libraryName,
				"PaintDotNet.Core, Version=5.112.9563.32325, Culture=neutral, PublicKeyToken=null",
			);
		}
		const properties = object.members["Layer+properties"].members;
		assert.equal(properties.name, `Layer ${i} 日本語🙂`);
		assert.equal(properties.userMetadataItems, null);
		assert.equal(properties.visible, i % 2 === 0);
		assert.equal(properties.opacity, (i * 127 + 1) & 255);
		assert.equal(properties.isBackground, i === 0);
		assert.equal(properties.blendMode.members.value__, 0);
		const surface = object.members.surface.members;
		assert.equal(surface.width, 3);
		assert.equal(surface.height, 2);
		assert.equal(surface.stride, 12);
		assert.equal(surface.scan0.members.length64, 24n);
		assert.equal(surface.scan0.members.hasParent, false);
		assert.equal(surface.scan0.members.deferred, true);
		const expected = Array.from(
			{ length: 24 },
			(_, j) => (j * 37 + i * 53) & 255,
		);
		const actual = decoded.layers[i];
		assert.deepEqual(Array.from(actual.canvas.pixels), expected);
		assert.equal(actual.name, properties.name);
		assert.equal(actual.visible, properties.visible);
		assert.equal(actual.opacity, properties.opacity);
		assert.equal(actual.isBackground, properties.isBackground);
		assert.equal(actual.blendMode, 0);
		layers.push({
			index: i,
			name: properties.name,
			visible: properties.visible,
			opacity: properties.opacity,
			is_background: properties.isBackground,
			rgba_sha256: sha(Buffer.from(expected)),
		});
	}
	assert.equal(sha(fs.readFileSync(`${folder}/${name}`)), before);
	results.push({
		filename: name,
		file_sha256: before,
		bytes: bytes.length,
		layers,
	});
}
console.log(
	JSON.stringify(
		{
			node: process.version,
			ms_nrbf_js: "0.1.0",
			ms_nrbf_js_archive_sha256:
				"b6d3d34f8484e0be472452708b8c4bb1f0f436aa6a8a520c5539778ce4d34c1d",
			ms_nrbf_js_archive_integrity:
				"sha512-2Zt3EA2Fafzq517Sh6MFgdCYbGyRz8lmFMKaywJF3PDVARaIhXUIWGnAq+opOG9ox+aVFDUeQ/yeUUAVJJ2J3A==",
			paintdotjs_commit: "6fe29faa80fd3a51e84fd97ab8d7ba03b9e637a7",
			paintdotjs_codec_sha256: sha(Buffer.from(source)),
			scope: "independent inert graph and original decoder pixel conversion, no canvas rendering or original Paint.NET qualification",
			results,
		},
		null,
		4,
	),
);
