# Independent PCND3 fixtures

`generate.py` writes these bytes with Python standard-library UUID, LEB128,
ZigZag and SHA-256 primitives. It never calls a PaintCraft serializer. Run it from
any directory to reproduce the fixture bytes and `oracle.json`.

The two source pixels are literal straight RGBA `[17,32,255,0]` and
`[0,127,200,255]`, including hidden RGB. The ordered three-instance stack includes
nil-ID disabled brightness/contrast at the parameter boundary, enabled invert,
and another enabled parameterized adjustment. A disabled-only file still uses
version 3. Version-one/two fixtures use identical raw pixels and empty stacks.

The optional attachment is copied verbatim from the independently authored
`pcnd-metadata-fixtures/rgb-profile.pcnd` source header and original ICC bytes.
The parent SHA and exact source offset/extent recipe are recorded in the
generator/oracle. This reuse proves native metadata preservation; it is not a
new foreign-image, CMS or ICC-colorimetry claim.

Every malformed fixture has a recomputed valid outer SHA. Its failure therefore
exercises the intended count, boolean, duplicate, parameter, algorithm/version,
source-presence or canonical-version boundary rather than a checksum mismatch.

Historical version note: the frozen `future-v5.pcnd` belongs to the earlier
point-only qualification. Version5 is now recognized; this old fixture is still
refused by legacy `read()` because explicit coverage admission is required for
the new grammar. It is no longer evidence that5 is unknown. The PCND5 spatial
corpus contains `future-v6.pcnd` as the current unknown-version control. Original
fixture bytes and oracle hashes are unchanged.
