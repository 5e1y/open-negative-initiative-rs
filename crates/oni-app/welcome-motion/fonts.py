#!/usr/bin/env python3
"""The interface's fonts as a browser reads them: `fonts.py <fonts dir> <out dir>`. Chrome's sanitiser refuses a
font whose vertical metrics do not parse, and a film's text is never set vertically, so those tables are left out."""
import pathlib
import sys

from fontTools.ttLib import TTFont

source, out = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
out.mkdir(parents=True, exist_ok=True)
for path in source.glob('*.ttf'):
    font = TTFont(path)
    for table in ('vmtx', 'vhea'):
        if table in font:
            del font[table]
    font.save(out / path.name)
