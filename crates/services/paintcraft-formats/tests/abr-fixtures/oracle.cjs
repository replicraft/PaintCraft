// Optional source-fixture oracle. It is not a product/runtime dependency.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const upstream = process.argv[2];
if (!upstream) throw new Error("Usage: node oracle.cjs <ag-psd package directory>");
const root = path.resolve(upstream);
assert.equal(require(path.join(root, "package.json")).version, "33.0.0");
const { readAbr: readAbrBackend } = require(path.join(root, "dist/abr.js"));
const readAbr = (bytes) => readAbrBackend(bytes, { throwForMissingFeatures: true });
const { createReader, readDataRLE } = require(path.join(root, "dist/psdReader.js"));
const normalizedDirectory = process.argv[3];
const manifest = JSON.parse(fs.readFileSync(path.join(__dirname, "sources.json"), "utf8"));

// readAbr's convenience alpha is eight-bit. Its public channel helper exposes
// the actual 16-bit result, so verify low bits and inert header bytes as well.
function resourceFields(bytes) {
	assert.ok(bytes.length <= 48 * 1024 * 1024);
	const minor = bytes.readUInt16BE(2);
	const sections = [];
	let offset = 4;
	while (offset < bytes.length) {
		assert.equal(bytes.subarray(offset, offset + 4).toString(), "8BIM");
		const kind = bytes.subarray(offset + 4, offset + 8).toString();
		const size = bytes.readUInt32BE(offset + 8);
		offset += 12;
		const payload = bytes.subarray(offset, offset + size);
		if (kind !== "samp") {
			sections.push({ kind, payload: payload.toString("hex") });
		} else {
			const samples = [];
			let pos = 0;
			while (pos < payload.length) {
				const length = payload.readUInt32BE(pos);
				pos += 4;
				const body = payload.subarray(pos, pos + length);
				const boundsOffset = 1 + body[0] + (minor === 1 ? 10 : 264);
				const width = body.readInt32BE(boundsOffset + 12) - body.readInt32BE(boundsOffset + 4);
				const height = body.readInt32BE(boundsOffset + 8) - body.readInt32BE(boundsOffset);
				const header = boundsOffset + 19;
				const depth = body.readUInt16BE(header - 3);
				const compression = body[header - 1];
				let mask;
				if (depth === 16 && compression === 1) {
					const data = body.subarray(header);
					const reader = createReader(data.buffer, data.byteOffset, data.byteLength);
					reader.strict = true;
					mask = new Uint16Array(width * height);
					readDataRLE(reader, { width, height, data: mask }, width, height, 16, 1, [0], false);
					assert.equal(reader.offset, data.length);
					mask = [...mask];
				} else if (depth === 16) {
					mask = Array.from({ length: width * height }, (_, i) => body.readUInt16BE(header + i * 2));
				} else {
					// Eight-bit records are copied verbatim by normalization.
					mask = body.subarray(header).toString("hex");
				}
				if (depth === 16) assert.deepEqual(mask, [0, 1, 255, 256, 32769, 65535]);
				samples.push({ header: body.subarray(0, header - 1).toString("hex"), mask });
				pos += Math.ceil(length / 4) * 4;
			}
			assert.equal(pos, payload.length);
			sections.push({ kind, samples });
		}
		offset += Math.ceil(size / 4) * 4;
	}
	assert.equal(offset, bytes.length);
	return { version: bytes.subarray(0, 4).toString("hex"), sections };
}

for (const [name, receipt] of Object.entries(manifest.files)) {
	const bytes = fs.readFileSync(path.join(__dirname, name));
	assert.equal(bytes.length, receipt.bytes);
	assert.equal(crypto.createHash("sha256").update(bytes).digest("hex"), receipt.sha256);
	if (name === "native-presets.abr") {
		const library = readAbr(bytes);
		assert.equal(library.brushes.length, 15);
		assert.equal(library.samples.length, 3);
		assert.deepEqual([...library.samples[0].alpha], [0, 1, 127, 128, 254, 255]);
		assert.deepEqual([...library.samples[2].alpha], [0, 0, 0, 0, 0, 0]);
		assert.equal(library.brushes[0].shape.sampledData, "raw-8");
		assert.equal(library.brushes[0].shape.size, 3);
		assert.equal(library.brushes[0].shape.angle, 0);
		assert.equal(library.brushes[0].shape.roundness, 1);
		assert.equal(library.brushes[0].shape.spacing, 1);
		assert.equal(library.brushes[1].shape.spacing, 0.1);
		assert.equal(library.brushes[13].shapeDynamics.sizeDynamics.control, "pen pressure");
		console.log(JSON.stringify({ name, bytes: bytes.length, sha256: receipt.sha256,
			oracle: "ag-psd 33.0.0", qualification: "owned static-preset metadata and alpha reader check; no renderer parity claim" }));
		continue;
	}
	if (name === "indexed-extra3.abr") {
		// The upstream palette heuristic mistakes the retained word 3 for the
		// VMAL version. Independent psd-tools verifies this specimen instead.
		assert.throws(() => readAbr(bytes));
		console.log(JSON.stringify({ name, oracle: "ag-psd 33.0.0", qualification: "upstream palette-word ambiguity retained as a known limitation" }));
		continue;
	}
	const library = readAbr(bytes);
	const patternFile = ["patterns.abr", "indexed-no-extra.abr", "auxiliary.abr", "missing-color.abr", "empty-color.abr", "user-mask.abr"].includes(name);
	const counts = patternFile ? [1, 2] : name === "empty.abr" ? [0, 0] : name === "samples-v10.abr" ? [1, 0] : [3, 2];
	assert.equal(library.samples.length, counts[0]);
	assert.equal(library.brushes.length, counts[1]);
	assert.equal(library.patterns.length, patternFile ? name === "patterns.abr" ? 5 : 1 : 0);
	if (name === "patterns.abr") {
		const expected = {
			rgb8: [10, 100, 200, 0, 20, 110, 210, 64, 30, 120, 220, 128, 40, 130, 230, 255],
			gray16: [0, 0, 0, 0, 1, 1, 1, 0, 128, 128, 128, 128, 255, 255, 255, 255],
			float32: [0, 64, 255, 255, 128, 191, 0, 128, 255, 255, 32, 64, 0, 0, 223, 0],
			partial: [0, 10, 100, 255, 1, 20, 110, 0, 2, 30, 120, 64, 0, 40, 130, 255, 3, 50, 140, 128, 4, 60, 150, 255],
		};
		for (const pattern of library.patterns) {
			if (pattern.id === "indexed8") {
				// Ignoring the optional palette tail is not qualified by Adobe's
				// pattern specification; retain data, but do not claim a preview.
				console.log(JSON.stringify({ name, pattern: pattern.id, previewQualified: false,
					qualification: "opaque palette metadata; source slots checked with psd-tools" }));
			} else {
				assert.deepEqual([...pattern.data], expected[pattern.id]);
			}
			assert.equal(pattern.x, -3);
			assert.equal(pattern.y, 4);
			assert.deepEqual(pattern.bounds, { x: 7, y: 5, w: pattern.id === "partial" ? 3 : 2, h: 2 });
		}
	} else if (name === "indexed-no-extra.abr") {
		assert.deepEqual([...library.patterns[0].data], [11, 22, 33, 255, 200, 100, 50, 128, 200, 100, 50, 64, 11, 22, 33, 0]);
	} else if (patternFile) {
		// The upstream reader guesses roles by written ordinal here. PaintCraft
		// deliberately exposes no preview; independent psd-tools checks slots.
		console.log(JSON.stringify({ name, previewQualified: false, qualification: "exact slots checked separately; ambiguous roles are not rendered" }));
	}
	for (const sample of library.samples) {
		assert.deepEqual(sample.bounds, { x: -1, y: -2, w: 3, h: 2 });
		const expected = { "raw-8": [0, 1, 127, 128, 254, 255], "rle-8": [0, 1, 255, 127, 127, 127],
			"raw-16": [0, 0, 0, 1, 128, 255], "rle-16": [0, 0, 0, 1, 128, 255] };
		assert.deepEqual([...sample.alpha], expected[sample.id]);
	}
	if (name === "samples-v6.abr") {
		assert.equal(library.samples[1].id, "rle-8");
		assert.deepEqual([...library.samples[1].alpha], [0, 1, 255, 127, 127, 127]);
		assert.equal(library.samples[2].id, "raw-16");
		// Both upstream convenience models narrow this mask. PaintCraft's Rust
		// test separately proves exact [0,1,255,256,32769,65535] retention.
		assert.deepEqual([...library.samples[2].alpha], [0, 0, 0, 1, 128, 255]);
	}
	if (library.brushes.length) {
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
	const fields = resourceFields(bytes);
	if (normalizedDirectory && name.startsWith("rle16-")) {
		const normalized = fs.readFileSync(path.join(normalizedDirectory, name));
		assert.deepEqual(readAbr(normalized), library);
		assert.deepEqual(resourceFields(normalized), fields);
		console.log(JSON.stringify({ name, normalizedBytes: normalized.length,
			sha256: crypto.createHash("sha256").update(normalized).digest("hex"),
			qualification: "equivalent private raw16 decoder view, every low bit and header verified" }));
	}
	console.log(JSON.stringify({ name, bytes: bytes.length, samples: counts[0], brushes: counts[1],
		sha256: receipt.sha256, oracle: "ag-psd 33.0.0",
		qualification: patternFile && !["patterns.abr", "indexed-no-extra.abr"].includes(name)
			? "metadata reader only; ambiguous preview unavailable; exact slots use psd-tools"
			: "owned-fixture reader interoperability" }));
}
