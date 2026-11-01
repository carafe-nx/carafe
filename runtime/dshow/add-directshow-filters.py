import struct
import sys
import uuid
from pathlib import Path

romfs = Path(sys.argv[1])
classes_reg = romfs / 'switch/wine/horizon-dlls/classes.reg'

MERIT_PREFERRED = 0x800000
MERIT_NORMAL = 0x600000
MERIT_UNLIKELY = 0x400000

PIN_OUTPUT = 0x8
PIN_RENDERER = 0x2

CATEGORIES = '{da4e3da0-d07d-11d0-bd50-00a0c911ce86}'
LEGACY = '{083863f1-70de-11d0-bd40-00a0c911ce86}'
AUDIO_RENDERERS = '{e0f158e1-cb04-11d0-bd4e-00a0c911ce86}'

NULL = '{00000000-0000-0000-0000-000000000000}'
STREAM = '{e436eb83-524f-11ce-9f53-0020af0ba770}'
VIDEO = '{73646976-0000-0010-8000-00aa00389b71}'
AUDIO = '{73647561-0000-0010-8000-00aa00389b71}'

FILTERS = [
    ('{f9d8d64e-a144-47dc-8ee0-f53498372c29}', 'GStreamer splitter filter', MERIT_NORMAL - 1,
     [(0, [(STREAM, NULL)]), (PIN_OUTPUT, [(AUDIO, NULL)]), (PIN_OUTPUT, [(VIDEO, NULL)])]),
    ('{70e102b0-5556-11ce-97c0-00aa0055595a}', 'Video Renderer', MERIT_PREFERRED,
     [(PIN_RENDERER, [(VIDEO, NULL)])]),
    ('{6bc1cffa-8fc1-4261-ac22-cfb4cc38db50}', 'Video Renderer', MERIT_PREFERRED + 1,
     [(PIN_RENDERER, [(VIDEO, NULL)])]),
    ('{e436ebb5-524f-11ce-9f53-0020af0ba770}', 'File Source (Async.)', MERIT_UNLIKELY,
     [(PIN_OUTPUT, [(STREAM, NULL)])]),
    ('{6a08cf80-0e18-11cf-a24d-0020afd79767}', 'ACM Wrapper', MERIT_NORMAL - 16,
     [(0, [(AUDIO, NULL)]), (PIN_OUTPUT, [(AUDIO, NULL)])]),
    ('{cf49d4e0-1115-11ce-b03a-0020af0ba770}', 'AVI Decompressor', MERIT_NORMAL - 16,
     [(0, [(VIDEO, NULL)]), (PIN_OUTPUT, [(VIDEO, NULL)])]),
]


def guid(text):
    return uuid.UUID(text).bytes_le


def filter_data(merit, pins):
    size = 16 + sum(24 + 16 * len(types) for _, types in pins)
    main = bytearray(struct.pack('<4I', 2, merit, len(pins), 0))
    store = bytearray()

    def offset(value):
        data = guid(value)
        index = store.find(data)
        if index < 0 or index % 16:
            index = len(store)
            store.extend(data)
        return size + index

    for number, (flags, types) in enumerate(pins):
        main += bytes([ord('0') + number]) + b'pi3'
        main += struct.pack('<5I', flags, 0, len(types), 0, 0)
        for kind, (major, minor) in enumerate(types):
            main += bytes([ord('0') + kind]) + b'ty3'
            main += struct.pack('<3I', 0, offset(major), offset(minor))
    return bytes(main + store)


def key(*parts):
    return '[Software\\\\Classes\\\\CLSID\\\\' + '\\\\'.join(parts) + ']'


lines = [';; DirectShow: categories and filters']
for category, name, merit in ((LEGACY, 'DirectShow Filters', MERIT_NORMAL),
                              (AUDIO_RENDERERS, 'Audio Renderers', MERIT_NORMAL)):
    lines += [key(CATEGORIES, 'Instance', category), f'"CLSID"="{category}"', f'"FriendlyName"="{name}"',
              f'"Merit"=dword:{merit:08x}', '']
for clsid, name, merit, pins in FILTERS:
    data = ','.join(f'{byte:02x}' for byte in filter_data(merit, pins))
    lines += [key(LEGACY, 'Instance', clsid), f'"CLSID"="{clsid}"', f'"FriendlyName"="{name}"',
              f'"FilterData"=hex:{data}', '']

text = classes_reg.read_text(encoding='utf-8')
if not text.endswith('\n'):
    text += '\n'
classes_reg.write_text(text + '\n' + '\n'.join(lines) + '\n', encoding='utf-8')
print(f'DirectShow: {len(FILTERS)} filters in 2 categories registered in {classes_reg.name}')
