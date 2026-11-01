"""Synthetic schema controls only; genuine CLI/MCP tests remain separate."""
import copy
import hashlib
import json
from pathlib import Path
import runpy
import unittest
from jsonschema import Draft202012Validator, FormatChecker

HERE=Path(__file__).resolve().parent
G=runpy.run_path(str(HERE/'generate.py'),run_name='layer_schema_generator')
SHA,FILES=G['build']()
SCHEMAS={k:json.loads(FILES[k+'.schema.json']) for k in ['batch','result','file-command','inspect','mcp-inspect']}
V={k:Draft202012Validator(v,format_checker=FormatChecker()) for k,v in SCHEMAS.items()}
UUID='00000000-0000-0000-0000-000000000001'
OWN='layer-masks.v1'

def batch(op='layer.mask.remove'):
    value={'op':op,'layer_ref':{'id':UUID}}
    if op in G['OPS'][:2]:value['enabled']=False
    return {'schema':'paintcraft.batch/layer-appearance-v1','atomic':True,
            'policy':'paintcraft.core/default-v1','base':{'state_id':UUID,'publication_epoch':'0'},
            'required_filters':[],'required_layer_features':[OWN],'operations':[value]}

def result():
    return {'schema':'paintcraft.result/v5','committed':False,'state_id':UUID,
            'publication_epoch':'0','history_id':None,'aliases':{},'warnings':[],
            'document_features':[OWN],'required_filters':[]}

def inspection(mcp=False):
    mask={'schema':'a8-bands/v1','enabled':False,'domain':[0,0,2,1],'bands':0,'runs':0,
          'digest':'0'*64,'intent_digest':'1'*64}
    layer={'id':UUID,'name':'x','visible':True,'mask':mask}
    if mcp:
        layer['opacity']=255
        return {'schema':'paintcraft.mcp-inspect/v5','document_id':UUID,'state_id':UUID,
                'width':2,'height':1,'dirty':False,'undo_depth':0,'redo_depth':0,
                'required_filters':[],'document_features':[OWN],'layers':[layer]}
    layer.update(opacity_u8=255,origin={'x':0,'y':0},width=2,height=1,default_pixel='#00000000')
    header=['raster.rgba8','canvas.finite','layers.flat','layer-point-filters.v1',
            'layer-filter-masks.v1','layer-filter-catalog.v1',OWN]
    return {'schema':'paintcraft.inspect/v5','input_sha256':'0'*64,
            'required_filters':[],'document_features':[OWN],'ticket':{'state_id':UUID,'publication_epoch':'0'},
            'assessment':{'schema':'paintcraft.open-assessment/layer-appearance-v1','assessment_only':True,
                'source_sha256':'0'*64,'profile':'paintcraft.native/binary7-document7-layer-masks1',
                'container_version':7,'document_schema':'paintcraft.document/v7','required_features':header,
                'required_filters':[],'capability_generation':'0','mode':'editable','migration_id':None,'reasons':[]},
            'document':{'document_id':UUID,'state_id':UUID,'canvas':{'width':2,'height':1},
                'policy':'paintcraft.core/default-v1','layers':[layer]}}

class Schemas(unittest.TestCase):
    def invalid(self,kind,value):
        self.assertTrue(list(V[kind].iter_errors(value)),(kind,value))
    def test_local_resolution_and_immutable_input_hashes(self):
        manifest=json.loads(FILES['manifest.json'])
        self.assertEqual(SHA,hashlib.sha256(G['canonical'](manifest['profile_contract'])).hexdigest())
        self.assertEqual(len(manifest['profile_contract']['filter_ids']),13)
        for name,data in FILES.items():self.assertEqual((HERE/SHA/name).read_bytes(),data)
        for schema in SCHEMAS.values():Draft202012Validator.check_schema(schema)
        for name,digest in G['INPUTS'].items():self.assertEqual(hashlib.sha256((G['BASE']/name).read_bytes()).hexdigest(),digest)
    def test_four_closed_selection_operations_and_required_arrays(self):
        for op in G['OPS']:
            v=batch(op);V['batch'].validate(v)
            bad=copy.deepcopy(v);bad['operations'][0]['mask']={};self.invalid('batch',bad)
            bad=copy.deepcopy(v);bad['required_layer_features']=[];self.invalid('batch',bad)
        bad=batch(G['OPS'][0]);del bad['operations'][0]['enabled'];self.invalid('batch',bad)
        bad=batch();bad['operations'][0]['op']='layer.mask.set';self.invalid('batch',bad)
        for old in ['paintcraft.batch/v1','paintcraft.batch/v5','paintcraft.batch/catalog-v1']:
            bad=batch();bad['schema']=old;self.invalid('batch',bad)
    def test_new_envelope_can_explicitly_bind_no_new_operation_features(self):
        v=batch();v['operations']=[{'op':'selection.clear'}];v['required_layer_features']=[];V['batch'].validate(v)
        v['required_layer_features']=[OWN];self.invalid('batch',v)
    def test_filterless_v5_always_has_empty_requirements(self):
        v=result();V['result'].validate(v)
        del v['required_filters'];self.invalid('result',v)
        v=result();v['required_filters']=['invert-rgb-v1'];V['result'].validate(v)
        v['required_filters']=['unknown-v1'];self.invalid('result',v)
    def test_finite_inspection_accepts_presence_and_refuses_payload_or_absence(self):
        for mcp in [False,True]:
            kind='mcp-inspect' if mcp else 'inspect';v=inspection(mcp);V[kind].validate(v)
            layer=v['layers'][0] if mcp else v['document']['layers'][0]
            layer['mask']['pixels']=[255,0];self.invalid(kind,v);del layer['mask']['pixels']
            del layer['mask']['intent_digest'];self.invalid(kind,v)
            v=inspection(mcp);layer=v['layers'][0] if mcp else v['document']['layers'][0];del layer['mask'];self.invalid(kind,v)
    def test_legacy_only_filter_features_do_not_require_catalog_class(self):
        for mcp in [False,True]:
            kind='mcp-inspect' if mcp else 'inspect';v=inspection(mcp)
            layer=v['layers'][0] if mcp else v['document']['layers'][0]
            layer['filters']=[{'id':UUID,'enabled':False,'descriptor':{'kind':'invert-rgb-v1'}}]
            v['document_features']=['layer-point-filters.v1',OWN];v['required_filters']=['invert-rgb-v1']
            V[kind].validate(v)
            v['document_features']=[OWN];self.invalid(kind,v)
if __name__=='__main__':unittest.main()
