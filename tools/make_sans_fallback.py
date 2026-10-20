"""Build assets/fonts/VSSansFallback-Regular.ttf: Source Sans 3 Regular cut to Latin text and
renamed, because the OFL reserves the name "Source" for unmodified fonts.

    python3 tools/make_sans_fallback.py assets/fonts      (needs fontTools: pip install fonttools)
"""
import subprocess, sys, os
from fontTools.ttLib import TTFont

fonts = sys.argv[1]
src = os.path.join(fonts, 'SourceSans3-Regular.ttf')
out = os.path.join(fonts, 'VSSansFallback-Regular.ttf')
UNICODES = 'U+0020-007E,U+00A0-017F,U+0192,U+02C6,U+02DC,U+2000-206F,U+20AC,U+2122,U+2190-2193,U+2212,U+2264,U+2265'
tmp = out + '.tmp'
subprocess.run(['pyftsubset', src, f'--unicodes={UNICODES}', f'--output-file={tmp}'], check=True)
f = TTFont(tmp)
names = {
    1: 'VS Sans Fallback',
    3: 'VSSansFallback-Regular;verSHion',
    4: 'VS Sans Fallback Regular',
    6: 'VSSansFallback-Regular',
    16: 'VS Sans Fallback',
    17: 'Regular',
}
table = f['name']
for rec in list(table.names):
    if rec.nameID in names:
        table.setName(names[rec.nameID], rec.nameID, rec.platformID, rec.platEncID, rec.langID)
    elif rec.nameID in (18, 20, 21, 25):
        table.removeNames(nameID=rec.nameID)
f.save(out)
os.remove(tmp)
check = TTFont(out)
left = [r.toUnicode() for r in check['name'].names if r.nameID not in (0, 7, 13, 14) and 'Source' in r.toUnicode()]
assert not left, left
print(out, os.path.getsize(out))
