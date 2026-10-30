"""Independent point-registry corpus keeps exact outcomes and failure evidence."""
import copy
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import pcnd_point_replay_smoke as runner
from pcnd_filter_replay_smoke import inventory as filters
from pcnd_mask_replay_smoke import inventory as masks
from pcnd_replay_smoke import inventory as originals
from pcnd_semantic_smoke import STATE, mutations
from replay_smoke import verify


class PointInventory(unittest.TestCase):
    def manifest(self):
        return json.loads((runner.ROOT / runner.MANIFEST).read_text())

    def test_exact29_point_seeds_remain_separate_from_historical_corpora(self):
        rows=runner.inventory(self.manifest())
        self.assertEqual((len(rows),sum(r['accepted'] for r in rows),runner.TOTAL),(29,23,7453))
        for name,inventory,count in [('pcnd-seeds.json',originals,14),('pcnd-filter-seeds.json',filters,16),('pcnd-mask-seeds.json',masks,24)]:
            self.assertEqual(len(inventory(json.loads((runner.ROOT/name).read_text()))),count)

    def test_provenance_and_same_count_outcome_swaps_are_refused(self):
        for mode in ['schema','count','duplicate','path','target','hash','accepted-swap','accepted-type','oracle','generator']:
            m=copy.deepcopy(self.manifest())
            if mode=='schema':m['schema']='unknown'
            elif mode=='count':m['seeds'].pop()
            elif mode=='duplicate':m['seeds'][1]=copy.deepcopy(m['seeds'][0])
            elif mode=='path':m['seeds'][0]['source']='../outside.pcnd'
            elif mode=='target':m['seeds'][0]['target']='raster'
            elif mode=='hash':m['seeds'][0]['sha256']='0'*64
            elif mode=='accepted-type':m['seeds'][0]['accepted']=1
            elif mode=='accepted-swap':
                a=next(r for r in m['seeds'] if r['accepted']);b=next(r for r in m['seeds'] if not r['accepted'])
                a['accepted'],b['accepted']=False,True
            else:m[mode]['sha256']='0'*64
            with self.subTest(mode=mode),self.assertRaises(ValueError):runner.inventory(m)

    def test_timeout_and_nonzero_keep_exact_generated_batch_then_propagate(self):
        manifest=self.manifest();accepted={r['name']:r['accepted'] for r in manifest['seeds']}
        for mode in ['timeout','exit']:
            generated={};timeout=subprocess.TimeoutExpired(['replay'],60,output=b'partial',stderr=b'failure')
            def execute(args,**kwargs):
                if len(args)==4:
                    good=int(accepted[Path(args[-1]).name]);text=f'target=Pcnd cases=1 accepted={good} rejected={1-good} '
                elif args[2]=='256':text='target=Pcnd cases=7453 accepted=23 rejected=7430 '
                else:
                    generated.update({Path(p).name:Path(p).read_bytes() for p in args[3:]})
                    if mode=='timeout':raise timeout
                    return subprocess.CompletedProcess(args,17,'partial','failure')
                return subprocess.CompletedProcess(args,0,text+'seed=0x5041494e54435241 mode=deterministic-replay policy=invariants-fail-v1\n','')
            with self.subTest(mode=mode),tempfile.TemporaryDirectory() as folder:
                root=Path(folder)/'fuzz';root.mkdir();(root/runner.MANIFEST).write_text(json.dumps(manifest))
                with patch.object(runner,'ROOT',root),patch.object(runner.subprocess,'run',execute):
                    expected=subprocess.TimeoutExpired if mode=='timeout' else subprocess.CalledProcessError
                    with self.assertRaises(expected) as caught:runner.run(Path('not-executed'))
                if mode=='timeout':self.assertIs(caught.exception,timeout)
                parent=root.parent/'docs/validation/pcnd-point-fuzz';failures=list(parent.glob('filter-failure-*'))
                self.assertEqual(len(failures),1);self.assertEqual(len(generated),257)
                self.assertFalse(list(parent.glob('point-semantic-inputs-*')))
                self.assertEqual({p.name:p.read_bytes() for p in failures[0].glob('*.pcnd')},generated)
                failure=json.loads((failures[0]/'failure.json').read_text())
                self.assertEqual(failure['stdout'],'partial')
                self.assertEqual((failure['timeout'],failure['returncode']),(60,None) if mode=='timeout' else (None,17))

    def test_checksum_mutation_is_deterministic_and_never_repairs_inner_bytes(self):
        row=next(r for r in runner.inventory(self.manifest()) if r['name']=='source-all-six.pcnd')
        source=verify(row).read_bytes();cases=list(mutations(source,STATE))
        self.assertEqual(cases,list(mutations(source,STATE)));self.assertEqual(len(cases),257)
        self.assertEqual(cases[0][0],source);self.assertEqual(cases[2][0][:-33],source[:-32])
        for data,_ in cases:self.assertEqual(hashlib.sha256(data[:-32]).digest(),data[-32:])
        self.assertEqual(verify(row).read_bytes(),source)


if __name__=='__main__':unittest.main()
