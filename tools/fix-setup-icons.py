#!/usr/bin/env python3
"""Delete Inno Setup's stock icon groups left in a Wine-built Setup.exe.

usage: fix-setup-icons.py IN.exe OUT.exe

Wine's UpdateResource replaces MAINICON but leaves MAINICON_DARK and the stock RT_ICON images
(ids 1-26). This removes every RT_GROUP_ICON except MAINICON and every RT_ICON that no remaining
group references, by rewriting the resource directory tables in place (same file size, so the
Setup loader's offsets are untouched), then refreshes the PE checksum.
"""
import struct, sys, pefile

src, dst = sys.argv[1:3]
pe = pefile.PE(src)
data = bytearray(open(src, "rb").read())
rdir = pe.OPTIONAL_HEADER.DATA_DIRECTORY[pefile.DIRECTORY_ENTRY["IMAGE_DIRECTORY_ENTRY_RESOURCE"]]
base = pe.get_offset_from_rva(rdir.VirtualAddress)

def read_dir(off):
    _, _, _, _, nn, ni = struct.unpack_from("<IIHHHH", data, off)
    ents = [struct.unpack_from("<II", data, off + 16 + 8 * i) for i in range(nn + ni)]
    return nn, ni, ents

def write_dir(off, nn, ents, old_total):
    ni = len(ents) - nn
    struct.pack_into("<HH", data, off + 12, nn, ni)
    for i in range(old_total):
        if i < len(ents):
            struct.pack_into("<II", data, off + 16 + 8 * i, *ents[i])
        else:
            struct.pack_into("<II", data, off + 16 + 8 * i, 0, 0)

def ent_name(e):
    if e[0] & 0x80000000:
        o = base + (e[0] & 0x7FFFFFFF)
        n = struct.unpack_from("<H", data, o)[0]
        return bytes(data[o + 2:o + 2 + 2 * n]).decode("utf-16le")
    return e[0]

def sub(e):
    assert e[1] & 0x80000000
    return base + (e[1] & 0x7FFFFFFF)

nn, ni, root = read_dir(base)
types = {ent_name(e): e for e in root}
# 1. group icons: keep only MAINICON; collect the RT_ICON ids it references.
goff = sub(types[14])
gnn, gni, gents = read_dir(goff)
keep = [e for e in gents if ent_name(e) == "MAINICON"]
assert len(keep) == 1, "no MAINICON group"
referenced = set()
for e in keep:
    lang_off = sub(e)
    _, _, lents = read_dir(lang_off)
    for le in lents:  # leaf: IMAGE_RESOURCE_DATA_ENTRY
        rva, size, _, _ = struct.unpack_from("<IIII", data, base + le[1])
        o = pe.get_offset_from_rva(rva)
        count = struct.unpack_from("<H", data, o + 4)[0]
        for i in range(count):
            referenced.add(struct.unpack_from("<H", data, o + 6 + 14 * i + 12)[0])
removed_groups = [ent_name(e) for e in gents if e not in keep]
write_dir(goff, 1 if isinstance(ent_name(keep[0]), str) else 0, keep, gnn + gni)
# 2. icon images: keep only referenced ids.
ioff = sub(types[3])
inn, ini, ients = read_dir(ioff)
kept = [e for e in ients if ent_name(e) in referenced]
removed_icons = sorted(ent_name(e) for e in ients if e not in kept)
write_dir(ioff, 0, kept, inn + ini)
open(dst, "wb").write(data)
pe2 = pefile.PE(dst)
pe2.OPTIONAL_HEADER.CheckSum = pe2.generate_checksum()
pe2.write(dst)
print("removed groups:", removed_groups, "removed RT_ICON ids:", removed_icons, "kept icon ids:", sorted(referenced))
