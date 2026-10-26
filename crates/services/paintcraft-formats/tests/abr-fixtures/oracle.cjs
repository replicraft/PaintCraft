// Optional source-fixture oracle. It is not a product/runtime dependency.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const upstream = process.argv[2];
if (!upstream) throw new Error("Usage: node oracle.cjs <ag-psd package directory>");
const root = path.resolve(upstream);
assert.equal(require(path.join(root, "package.json")).version, "33.0.0");
const { readAbr } = require(path.join(root, "dist/abr.js"));
const manifest = JSON.parse(fs.readFileSync(path.join(__dirname, "sources.json"), "utf8"));
for (const [name, receipt] of Object.entries(manifest.files)) {
	const bytes = fs.readFileSync(path.join(__dirname, name));
	assert.equal(bytes.length, receipt.bytes);
	assert.equal(crypto.createHash("sha256").update(bytes).digest("hex"), receipt.sha256);
	const library = readAbr(bytes);
	const counts = name === "empty.abr" ? [0, 0] : name === "samples-v6.abr" ? [3, 2] : [1, 0];
	assert.equal(library.samples.length, counts[0]);
	assert.equal(library.brushes.length, counts[1]);
	assert.equal(library.patterns.length, 0);
	if (library.samples.length) {
		assert.equal(library.samples[0].id, "raw-8");
		assert.deepEqual(library.samples[0].bounds, { x: -1, y: -2, w: 3, h: 2 });
		assert.deepEqual([...library.samples[0].alpha], [0, 1, 127, 128, 254, 255]);
	}
	if (name === "samples-v6.abr") {
		assert.equal(library.samples[1].id, "rle-8");
		assert.deepEqual([...library.samples[1].alpha], [0, 1, 255, 127, 127, 127]);
		assert.equal(library.samples[2].id, "raw-16");
		// Both upstream convenience models narrow this mask. PaintCraft's Rust
		// test separately proves exact [0,1,255,256,32769,65535] retention.
		assert.deepEqual([...library.samples[2].alpha], [0, 0, 0, 1, 128, 255]);
		const [computed, sampled] = library.brushes;
		assert.equal(computed.name, "Owned computed 🚀");
		assert.equal(computed.shape.type, "computed");
		assert.equal(computed.shape.hardness, 0.6);
		assert.equal(sampled.name, "Owned sampled");
		assert.equal(sampled.shape.type, "sampled");
		assert.equal(sampled.shape.sampledData, "raw-8");
		assert.equal(sampled.shape.name, "Mask name");
		for (const brush of library.brushes) {
			assert.equal(brush.shape.size, 30);
			assert.equal(brush.shape.angle, 15);
			assert.equal(brush.shape.roundness, 0.8);
			assert.equal(brush.shape.spacingOn, true);
			assert.equal(brush.shape.spacing, 0.25);
			assert.equal(brush.spacing, 1);
			assert.equal(brush.useBrushSize, true);
		}
	}
	console.log(JSON.stringify({ name, bytes: bytes.length, samples: counts[0], brushes: counts[1],
		sha256: receipt.sha256, oracle: "ag-psd 33.0.0", qualification: "owned-fixture reader interoperability" }));
}
