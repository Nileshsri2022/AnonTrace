import struct

def parse_axml_strings(path):
    with open(path, "rb") as f:
        data = f.read()

    # AXML starts with 0x00080003 (RES_XML_TYPE)
    magic, size = struct.unpack("<II", data[:8])
    if magic != 0x00080003:
        print("Not an AXML file, magic:", hex(magic))
        return []

    # Second chunk is RES_STRING_POOL_TYPE (0x001C0001)
    chunk_type, chunk_size, string_count, style_count, flags, strings_start, styles_start = struct.unpack("<IIIIIII", data[8:36])
    
    is_utf8 = bool(flags & (1 << 8))
    
    # Read string offsets
    offset_pos = 36
    offsets = [struct.unpack("<I", data[offset_pos + i*4 : offset_pos + (i+1)*4])[0] for i in range(string_count)]
    
    pool_data = data[8 + strings_start : 8 + chunk_size]
    strings = []
    
    for off in offsets:
        if is_utf8:
            # UTF-8 in AXML has 2 length bytes
            pos = off
            # skip length bytes
            u16len = pool_data[pos]
            pos += 1
            if u16len & 0x80:
                pos += 1
            u8len = pool_data[pos]
            pos += 1
            if u8len & 0x80:
                pos += 1
            s = pool_data[pos : pos + u8len].decode('utf-8', errors='ignore')
        else:
            # UTF-16LE: 2 bytes length, then chars
            pos = off
            u16len = struct.unpack("<H", pool_data[pos:pos+2])[0]
            pos += 2
            if u16len & 0x8000:
                pos += 2
            s = pool_data[pos : pos + u16len*2].decode('utf-16le', errors='ignore')
        strings.append(s)
    return strings

all_strings = parse_axml_strings("AndroidManifest.xml")
print(f"Total strings in AndroidManifest.xml: {len(all_strings)}")

perms = [s for s in all_strings if "permission" in s.lower()]
print("\n=== Android Permissions Declared in simplex.apk ===")
for p in sorted(set(perms)):
    print("-", p)

components = [s for s in all_strings if any(k in s.lower() for k in ["service", "receiver", "push", "firebase", "notification", "tor"])]
print("\n=== Services, Receivers & Handlers in simplex.apk ===")
for c in sorted(set(components)):
    if len(c) > 3 and not c.startswith("res/"):
        print("-", c)
