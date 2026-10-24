# Benchmark decision records

Maintained decisions and scope notes live here. Generated timing samples, logs,
fixtures and binary/source manifests live locally under `benchmarks/results/`,
which is ignored by Git. Existing result directories are preserved locally when
untracked; this does not remove their earlier Git history.

A decision distinguishes observed performance from platform qualification. Local
artifact paths in these records describe generated evidence and are not files
shipped in a fresh checkout. Reproduce measurements with `benchmarks/README.md`
and the tracked drivers before making new performance claims.
