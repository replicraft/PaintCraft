# Proposed Vibrance PCND6 fixtures

Independent stdlib framing writer, no product serializer/evaluator imports.
Source-only candidate; support and reader tests remain unqualified until promotion.
Run generate.py --check to reproduce19 specimens:10positive (seven signed
amounts-100,-75,-1,0,1,50,100 and disabled-100/0/100),9negative (four signed
out-of-range, four wrong lengths, unknown codec on disabled default).

The profile pair is vibrance-rgb-v1/params-v1 with exact signed i16BE2bytes.
Literal source RGBA=[10,11,128,0,20,115,200,128], retaining hidden RGB and partial
alpha. Semantic v3 hashes pin signed parameters and stored enablement. This is a
codec oracle, not a rendered-output oracle. Old profile fixtures remain unchanged.
