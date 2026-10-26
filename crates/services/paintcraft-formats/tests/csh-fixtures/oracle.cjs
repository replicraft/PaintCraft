// Optional upstream JavaScript reader, not a runtime dependency.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const [moduleDirectory, inputDirectory] = process.argv.slice(2);
if (!moduleDirectory || !inputDirectory) {
	throw new Error("Usage: node oracle.cjs <ag-psd package directory> <fixture/export directory>");
}
const upstream = path.resolve(moduleDirectory);
assert.equal(require(path.join(upstream, "package.json")).version, "33.0.0");
const { readCsh } = require(path.join(upstream, "dist/csh.js"));
const expected = JSON.parse(fs.readFileSync(path.join(__dirname, "expected.json"), "utf8"));
const manifest = JSON.parse(fs.readFileSync(path.join(__dirname, "sources.json"), "utf8"));

// The upstream representation discards bounds origins and selector 6. Inspect
// those exact wire fields separately; name padding may be canonicalized.
function wire(data) {
	assert.ok(data.length <= 4 * 1024 * 1024);
	assert.equal(data.subarray(0, 4).toString(), "cush");
	const count = data.readUInt32BE(8);
	assert.ok(count <= 256);
	let offset = 12;
	const shapes = [];
	for (let i = 0; i < count; i++) {
		const units = data.readUInt32BE(offset);
		offset += 4 + units * 2;
		offset = Math.ceil(offset / 4) * 4;
		assert.equal(data.readUInt32BE(offset), 1);
		const size = data.readUInt32BE(offset + 4);
		offset += 8;
		const end = offset + size;
		const idLength = data[offset];
		offset += 1 + idLength;
		shapes.push({ bounds: [...data.subarray(offset, offset + 16)],
			records: [...data.subarray(offset + 16, end)] });
		offset = end;
	}
	assert.equal(offset, data.length);
	return shapes;
}

for (const name of ["empty.csh", "paths.csh"]) {
	const owned = fs.readFileSync(path.join(__dirname, name));
	assert.equal(owned.length, manifest.files[name].bytes);
	assert.equal(crypto.createHash("sha256").update(owned).digest("hex"), manifest.files[name].sha256);
	const data = fs.readFileSync(path.join(inputDirectory, name));
	assert.deepEqual(readCsh(data), name === "empty.csh" ? { shapes: [] } : expected);
	assert.deepEqual(wire(data), wire(owned));
	console.log(JSON.stringify({ name, bytes: data.length, shapes: readCsh(data).shapes.length,
		sha256: crypto.createHash("sha256").update(data).digest("hex"), oracle: "ag-psd 33.0.0" }));
}
