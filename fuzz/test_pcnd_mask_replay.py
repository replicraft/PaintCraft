"""PCND4 authored inventory/timeout retention stays separate from older streams."""
import copy
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import pcnd_mask_replay_smoke as runner
from pcnd_filter_replay_smoke import inventory as filters
from pcnd_replay_smoke import inventory as originals


class MaskInventory(unittest.TestCase):
    def manifest(self):
        return json.loads((runner.ROOT / runner.MANIFEST).read_text())

    def test_independent_24_mask_seeds_have_separate_identity(self):
        rows=runner.inventory(self.manifest())
        self.assertEqual((len(rows),sum(r['accepted'] for r in rows),runner.TOTAL),(24,7,6168))
        self.assertEqual(len(filters(json.loads((runner.ROOT/'pcnd-filter-seeds.json').read_text()))),16)
        self.assertEqual(len(originals(json.loads((runner.ROOT/'pcnd-seeds.json').read_text()))),14)

    def test_same_positive_count_cannot_hide_swapped_oracle_outcomes(self):
        m=self.manifest()
        a=next(r for r in m['seeds'] if r['accepted'])
        b=next(r for r in m['seeds'] if not r['accepted'])
        a['accepted'],b['accepted']=False,True
        with self.assertRaises(ValueError):runner.inventory(m)
        for key in ['source','sha256']:
            m=self.manifest();m['seeds'][0][key]='../wrong'
            with self.subTest(key=key),self.assertRaises(ValueError):runner.inventory(m)
        m=self.manifest();m['seeds'][1]=copy.deepcopy(m['seeds'][0])
        with self.assertRaises(ValueError):runner.inventory(m)

    def test_timeout_retains_generated_inputs_and_reraises_original_failure(self):
        manifest=self.manifest();accepted={r['name']:r['accepted'] for r in manifest['seeds']}
        timeout=subprocess.TimeoutExpired(['replay'],60,output=b'partial',stderr=b'failure')
        generated={}
        def execute(args,**kwargs):
            if len(args)==4:
                good=int(accepted[Path(args[-1]).name]);text=f'target=Pcnd cases=1 accepted={good} rejected={1-good} '
            elif args[2]=='256':text='target=Pcnd cases=6168 accepted=7 rejected=6161 '
            else:
                generated.update({Path(p).name:Path(p).read_bytes() for p in args[3:]});raise timeout
            return subprocess.CompletedProcess(args,0,text+'seed=0x5041494e54435241 mode=deterministic-replay policy=invariants-fail-v1\n','')
        with tempfile.TemporaryDirectory() as folder:
            root=Path(folder)/'fuzz';root.mkdir();(root/runner.MANIFEST).write_text(json.dumps(manifest))
            with patch.object(runner,'ROOT',root),patch.object(runner.subprocess,'run',execute):
                with self.assertRaises(subprocess.TimeoutExpired) as caught:runner.run(Path('not-executed'))
            self.assertIs(caught.exception,timeout)
            parent=root.parent/'docs/validation/pcnd4-fuzz';failures=list(parent.glob('filter-failure-*'))
            self.assertEqual(len(failures),1);self.assertEqual(len(generated),257)
            self.assertFalse(list(parent.glob('mask-semantic-inputs-*')))
            self.assertEqual({p.name:p.read_bytes() for p in failures[0].glob('*.pcnd')},generated)
            failure=json.loads((failures[0]/'failure.json').read_text())
            self.assertEqual((failure['timeout'],failure['returncode'],failure['stdout']),(60,None,'partial'))


if __name__=='__main__':unittest.main()
