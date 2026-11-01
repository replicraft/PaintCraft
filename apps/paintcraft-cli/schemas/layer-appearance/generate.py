"""Additive immutable layer-appearance schemas; never rewrite legacy bundles."""
import argparse
import copy
import hashlib
import json
from pathlib import Path

HERE=Path(__file__).resolve().parent
ROOT=next(p for p in HERE.parents if (p/'apps/paintcraft-cli/schemas/catalog').is_dir())
BASE=ROOT/'apps/paintcraft-cli/schemas'
CATALOG='d8d4b881b9124340bc3ddb246bccc66909e3387ccb3873e5d21ebc4d3e80d7f4'
TOKEN='__LAYER_CONTRACT_SHA256__'
OWN='layer-masks.v1'
SOURCE='source-metadata.v1'
OPS=['layer.mask.from_selection','layer.mask.set_enabled','layer.mask.invert','layer.mask.remove']
# Populated from immutable promoted inputs by source authoring; no runtime code.
INPUTS={'catalog/d8d4b881b9124340bc3ddb246bccc66909e3387ccb3873e5d21ebc4d3e80d7f4/batch.schema.json': 'f28e9e31a980699224d88cfee4a37df96209a4258be43d0ec1353049d7797abb', 'catalog/d8d4b881b9124340bc3ddb246bccc66909e3387ccb3873e5d21ebc4d3e80d7f4/inspect.schema.json': 'e0c7d2c94e1e8d1aab9f737b1379697e55075d552ec09226c058a67a98e7d6f7', 'catalog/d8d4b881b9124340bc3ddb246bccc66909e3387ccb3873e5d21ebc4d3e80d7f4/mcp-inspect.schema.json': 'c597b012317d184c94f1de65cd9887cf4fa81f40429fd77f6fedbf4e6ed897e0', 'result-v5.schema.json': '3012df04bb941c01d7e96cd5d3cc72020a1b0cc6ed664426ce555b1e556f374c', 'file-command-v5.schema.json': '2f16b95ae4ec7a19f60f1a831ab92c2c576a45bf62beed3ee251efd542047894'}

def canonical(value):
    return json.dumps(value,sort_keys=True,separators=(',',':'),ensure_ascii=True).encode()

def sha(value):
    return hashlib.sha256(value).hexdigest()

def load(name):
    raw=(BASE/name).read_bytes()
    if sha(raw)!=INPUTS[name]:
        raise ValueError('Frozen schema input changed: '+name)
    return json.loads(raw)

def stripped(value):
    if isinstance(value,dict):
        return {k:stripped(v) for k,v in value.items() if k not in ['$id','$schema']}
    if isinstance(value,list):
        return [stripped(v) for v in value]
    return value

def templates():
    prefix='catalog/'+CATALOG+'/'
    batch=stripped(load(prefix+'batch.schema.json'))
    result=stripped(load('result-v5.schema.json'))
    envelope=stripped(load('file-command-v5.schema.json'))
    inspect=stripped(load(prefix+'inspect.schema.json'))
    mcp=stripped(load(prefix+'mcp-inspect.schema.json'))
    union=batch['properties']['operations']['items']['oneOf']
    layer_ref=copy.deepcopy(union[0]['properties']['layer_ref'])
    additions=[]
    for op in OPS:
        properties={'op':{'const':op},'layer_ref':copy.deepcopy(layer_ref)}
        if op in OPS[:2]:
            properties['enabled']={'type':'boolean'}
        additions.append({'type':'object','properties':properties,'required':list(properties),'additionalProperties':False})
    union.extend(additions)
    batch['properties']['schema']={'const':'paintcraft.batch/layer-appearance-v1'}
    batch['properties']['required_layer_features']={'oneOf':[{'const':[]},{'const':[OWN]}]}
    batch['required'].append('required_layer_features')
    batch['allOf']=[{'if':{'properties':{'operations':{'contains':{'required':['op'],'properties':{'op':{'enum':OPS}}}}}},
                    'then':{'properties':{'required_layer_features':{'const':[OWN]}}},
                    'else':{'properties':{'required_layer_features':{'const':[]}}}}]
    docs={'batch':batch,'result':result,'file-command':envelope,'inspect':inspect,'mcp-inspect':mcp}
    ids=result['properties']['required_filters']['items']['enum']
    old_mask=inspect['properties']['document']['properties']['layers']['items']['properties']['filters']['items']['properties']['mask']
    own_mask=copy.deepcopy(old_mask)
    own_mask['properties']['intent_digest']={'type':'string','pattern':'^[0-9a-f]{64}$'}
    own_mask['required'].append('intent_digest')
    for kind,d in [('inspect',inspect),('mcp-inspect',mcp)]:
        d['properties']['schema']={'const':'paintcraft.'+kind+'/v5'}
        parent=d['properties']['document'] if kind=='inspect' else d
        layers=parent['properties']['layers'];layers.pop('contains',None)
        layers['items']['properties']['mask']=copy.deepcopy(own_mask)
        layers['contains']={'required':['mask']}
        d['properties']['document_features']=copy.deepcopy(result['properties']['document_features'])
        d['properties']['required_filters']=copy.deepcopy(result['properties']['required_filters'])
        nested = kind=='inspect'
        def condition(test):
            shape={'required':['layers'],'properties':{'layers':{'contains':{'required':['filters'],'properties':{'filters':test}}}}}
            return {'required':['document'],'properties':{'document':shape}} if nested else shape
        def flag(probe,feature):
            return {'if':probe,'then':{'properties':{'document_features':{'contains':{'const':feature}}}},
                    'else':{'properties':{'document_features':{'not':{'contains':{'const':feature}}}}}}
        def kind_test(names):
            return {'contains':{'required':['descriptor'],'properties':{'descriptor':{'required':['kind'],'properties':{'kind':{'enum':names}}}}}}
        catalog_ids=['add-noise-rgb-v1','curves-rgb-v1','exposure-srgb-v1','hue-saturation-rgb-v1','levels-rgb-v1','vibrance-rgb-v1']
        d['allOf']=[flag(condition({'minItems':1}),'layer-point-filters.v1'),
                    flag(condition({'contains':{'required':['mask']}}),'layer-filter-masks.v1'),
                    flag(condition(kind_test(['box-blur-v1'])),'layer-spatial-filters.v1'),
                    flag(condition(kind_test(catalog_ids)),'layer-filter-catalog.v1')]
        if nested:
            source_test={'properties':{'assessment':{'properties':{'required_features':{'contains':{'const':SOURCE}}}}}}
            d['allOf'].append(flag(source_test,SOURCE))
    assessment=inspect['properties']['assessment']
    assessment['properties']['schema']={'const':'paintcraft.open-assessment/layer-appearance-v1'}
    assessment['properties']['profile']={'const':'paintcraft.native/binary7-document7-layer-masks1'}
    assessment['properties']['container_version']={'const':7}
    assessment['properties']['document_schema']={'const':'paintcraft.document/v7'}
    header=['raster.rgba8','canvas.finite','layers.flat','layer-point-filters.v1','layer-filter-masks.v1','layer-filter-catalog.v1',OWN]
    assessment['properties']['required_features']={'oneOf':[{'const':header},{'const':header+[SOURCE]}]}
    assessment['properties']['required_filters'].update(minItems=0)
    assessment['properties']['required_filters'].pop('contains',None)
    for kind,d in docs.items():
        d['$schema']='https://json-schema.org/draft/2020-12/schema'
        d['$id']='urn:paintcraft:schema:layer-appearance-v1:'+TOKEN+':'+kind
        d['title']='PaintCraft layer appearance '+kind
        d['$comment']='Additive immutable own-layer-mask contract; runtime verifies exact inventories, lexical integers, finite domain and actual selection capture. Old profiles remain unchanged.'
    # Keep the embedded engine grammar identical to this bundle's result shape.
    envelope['$defs']['engine_result']=stripped(result)
    contract={'schema':'paintcraft.layer-appearance-profile-contract/v1','filter_catalog_contract_sha256':CATALOG,
              'filter_ids':ids,'feature':OWN,'operations':additions,'batch_schema':'paintcraft.batch/layer-appearance-v1',
              'result_schema':'paintcraft.result/v5','file_command_schema':'paintcraft.file-command/v5',
              'inspect_schemas':['paintcraft.inspect/v5','paintcraft.mcp-inspect/v5'],
              'mask_metadata':own_mask,'source_version':7,'header7_features':header,
              'selection':'Actual live/staged Editor selection; no numeric layer-mask Set and no persisted live selection claim.',
              'runtime_constraints':['exact sorted all stored filter IDs in v5 outputs','exact prospective numerical union in batch','exact operation feature array','canonical integer tokens','finite domain relationships','atomic selection capture','aggregate coverage admission','old worker capabilities remain closed'],
              'inputs':INPUTS,'template_sha256':{k:sha(canonical(v)) for k,v in docs.items()}}
    return docs,contract

def build():
    docs,contract=templates();digest=sha(canonical(contract));files={}
    for kind,value in docs.items():
        files[kind+'.schema.json']=(json.dumps(value,indent="\t").replace(TOKEN,digest)+'\n').encode()
    manifest={'schema':'paintcraft.layer-appearance-schema-manifest/v1','profile_contract_sha256':digest,
              'contract_hash_encoding':'SHA256 canonical compact sorted-key ensure_ascii JSON; no final newline',
              'profile_contract':contract,'schemas':[{'kind':k,'file':k+'.schema.json','id':'urn:paintcraft:schema:layer-appearance-v1:'+digest+':'+k,'sha256':sha(files[k+'.schema.json'])} for k in docs],
              'availability':'Schema/source definition; qualified runtime consumers require separate execution evidence.'}
    files['manifest.json']=(json.dumps(manifest,indent="\t")+'\n').encode()
    return digest,files

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--check',action='store_true');a=parser.parse_args()
    digest,files=build();folder=HERE/digest
    for name,data in files.items():
        path=folder/name
        if a.check:
            if not path.exists() or path.read_bytes()!=data:raise ValueError('Resolved schema drift: '+str(path))
        else:
            folder.mkdir(exist_ok=True);path.write_bytes(data)
    print(digest)
if __name__=='__main__':main()
