"""Independent descriptive wire vector only, NOT valid ABI/source admission."""
import hashlib
import json
import pathlib
import struct

def vector():
    limits = (64 << 20, 268435456, 1 << 35)
    fields = struct.pack('>QQQQ', *limits, limits[1] + limits[2])
    name = b'noise-source-work-v1'
    digest = hashlib.sha256(b'paintcraft.noise.workload/v1\0' + struct.pack('>H',len(name)) + name + struct.pack('>I',1) + fields).digest()
    workload = fields + digest
    region = bytearray(128)
    region[:8] = struct.pack('>HHHH',8,1,6,2)
    struct.pack_into('>IIII',region,8,0,0,1,1)
    struct.pack_into('>QQQQQQQ',region,24,1,0,10,1024,1034,1024,128)
    region[80:112] = digest
    program = bytearray(256)
    program[:8] = region[:8]
    struct.pack_into('>IIII',program,8,1,1,1,0)
    struct.pack_into('>QQQQQQQQQ',program,24,0,0,0,128,10,1024,1034,128,2048)
    program[96:128] = bytes([17])*32
    program[128:160] = bytes([34])*32
    program[160:192] = digest
    program[192:224] = bytes([51])*32
    return {'scope':'Independent wire framing. Deliberately foreign ABI controls/placeholder capabilities; no real source acceptance.', 'workload':workload.hex(),'region':region.hex(),'program':program.hex()}

if __name__ == '__main__':
    target=pathlib.Path(__file__).with_name('vector.json')
    expected=json.dumps(vector(),indent='\t')+'\n'
    import sys
    if '--write' in sys.argv:target.write_text(expected)
    else:assert target.read_text()==expected
