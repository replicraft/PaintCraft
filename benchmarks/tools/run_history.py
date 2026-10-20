"""Record equivalent complete history lifecycles with the existing Hyperfine runner."""
import argparse
import json
import os
from pathlib import Path
import platform
import subprocess

from benchmark_support import sha256, validate_config
from run_composite import ROOT, output
from run_hyperfine import execute

CASES={"sparse256":256,"sparse1000":1000,"held1000":1000,"noise128":128,"metadata1000":1000}
VARIANTS=["shared","pack32","pack64","pack128"]
CODEC_CASES={**CASES,"structured256":256}
CODEC_VARIANTS=["shared","raw","lz4","zstd1","zstd3"]
SPILL_CASES={k:CASES[k] for k in ["sparse256","held1000","noise128","metadata1000"]}
SPILL_CASES["structured256"]=256
SPILL_VARIANTS=["heap","disk","heap-lz4","disk-lz4"]


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--suite",choices=["ownership","codecs","spill"],default="ownership")
    parser.add_argument("--output",type=Path,required=True)
    parser.add_argument("--execute",action="store_true")
    args=parser.parse_args();root=args.output.resolve();root.mkdir(parents=True,exist_ok=False)
    spill=args.suite=="spill"
    codecs=args.suite=="codecs"
    cases=SPILL_CASES if spill else CODEC_CASES if codecs else CASES
    variants=SPILL_VARIANTS if spill else CODEC_VARIANTS if codecs else VARIANTS
    driver="history_spill" if spill else "history_codecs" if codecs else "history"
    binary=ROOT/f"target/release/examples/{driver}"
    paths=[(binary,"binary"),(ROOT/"Cargo.lock","lockfile"),
           (ROOT/"crates/editing/paintcraft-editor/examples/support/history.rs","generator"),
           (ROOT/f"crates/editing/paintcraft-editor/examples/{driver}.rs","driver"),
           (ROOT/"crates/editing/paintcraft-editor/src/session.rs","other"),
           (ROOT/"crates/foundation/paintcraft-raster/src/raster.rs","other"),
           (ROOT/"crates/foundation/paintcraft-raster/src/raster/leases.rs","other")]
    paths.extend((p,"other") for p in sorted((ROOT/"crates/editing/paintcraft-history/src").rglob("*.rs")))
    if spill:
        paths.extend((ROOT/p,"other") for p in ["Cargo.toml","crates/editing/paintcraft-editor/Cargo.toml","crates/editing/paintcraft-history/Cargo.toml","crates/services/paintcraft-platform/Cargo.toml","crates/foundation/paintcraft-core/src/types/storage.rs","crates/services/paintcraft-platform/src/lib.rs","crates/services/paintcraft-platform/src/spill.rs"])
    artifacts=[{"path":str(p),"sha256":sha256(p),"role":role}for p,role in paths]
    host={"source_revision":output(["git","rev-parse","HEAD"]),"worktree_status":output(["git","status","--porcelain"]),
          "rustc":output(["rustc","-Vv"]),"platform":platform.platform(),"logical_cpus":os.cpu_count(),
          "limitations":"One developer host, no power/thermal/affinity control; RSS is process-wide. All variants retain identical required history, use a 768 MiB retention allowance, and finish full recorded undo/redo; no interactive latency qualification."}
    (root/"host.json").write_text(json.dumps(host,indent=2)+"\n")
    configs={}
    for case,count in cases.items():
        config={"schema":"paintcraft.hyperfine-run/1","mode":"application","case_id":"BF19" if codecs or spill else "BF18","task_id":"T11" if codecs or spill else "T07",
                "relevance":"Heap versus native append segments, raw and LZ4, equal 64-edit checkpoints; full timeline, fixed byte and worker budgets, actual sync/read/GC and store destruction." if spill else "Raw/shared versus LZ4 and Zstd levels 1/3 at equal 64-edit checkpoints; same retained timeline, fixed block sizes and single-threaded codec contexts." if codecs else "Recorded shared COW history versus bounded default-relative independent patches at explicit cold-age checkpoints; equal complete retained timelines.",
                "cwd":str(ROOT),"workload":{"id":f"history-{case}","mode":"batch","work_units":count*3+(3 if case=="metadata1000" else 0),"seed":42,
                "cache_state":"new process/session; 768 MiB retention, 16 MiB compaction staging, two hot states; spill suite creates a 256 MiB store with 4 MiB segment targets in the OS temporary filesystem for every strategy; page cache is not flushed; held case pins every eighth committed snapshot until release phase",
                "completion":"all commits, source CRC captures, held-reader release, exact recorded undo/redo validation, and document/history/output destruction complete"},
                "build":{"source_revision":host["source_revision"]+" with exact source/binary artifact hashes","profile":"release",
                "target":host["rustc"].split("host: ",1)[1].splitlines()[0],"compiler":output(["rustc","-V"]),
                "flags":"Cargo release defaults, opt-level 3, line tables; CARGO_INCREMENTAL=0","environment_notes":host["limitations"]},
                "runs":20,"warmup":3,"timeout_seconds":900,"env":{},"artifacts":artifacts,
                "correctness_commands":[[str(binary),"verify",case]],
                "variants":[{"id":v,"argv":[str(binary),v,case]}for v in variants]}
        validate_config(config);path=root/f"{case}-config.json";path.write_text(json.dumps(config,indent=2)+"\n");configs[case]=(config,path)
    if not args.execute:print(f"Prepared {root}; no benchmark executed");return
    flags=["-l"]if platform.system()=="Darwin"else["-v"]
    for case in cases:
        with (root/f"calibration-{case}.txt").open("w")as log:
            subprocess.run(["/usr/bin/time",*flags,str(binary),"calibrate",case],cwd=ROOT,stdout=log,stderr=log,check=True,timeout=120)
    for session,reverse in [(1,False),(2,True),(3,False)]:
        for case,(config,path) in configs.items():
            run=root/f"session-{session}-{case}";execute(config,path,run,"hyperfine",reverse)
            for variant in variants:
                with (run/f"resources-{variant}.txt").open("w")as log:
                    subprocess.run(["/usr/bin/time",*flags,str(binary),variant,case],cwd=ROOT,stdout=log,stderr=log,check=True,timeout=900)
            (run/"RESOURCES.sha256").write_text("".join(f"{sha256(p)}  {p.name}\n"for p in sorted(run.glob("resources-*.txt"))))
            print(f"Completed session {session}: {case}",flush=True)


if __name__=="__main__":main()
