import re

with open("usr/bin/simplex-chat", "rb") as f:
    data = f.read()

# Let's search for case-insensitive 'tor' as a word
matches = re.findall(rb"\b[tT]or\b|\b[oO]nion\b|hidden[_\s\-]?service", data)
print(f"Total Tor/onion references: {len(matches)}")

# Search for proxy configs
proxies = re.findall(rb"[^\x00]{1,30}proxy[^\x00]{0,30}", data, re.IGNORECASE)
print(f"Total proxy references: {len(proxies)}")
for p in list(set(proxies))[:15]:
    print(p.decode(errors='ignore').strip())
