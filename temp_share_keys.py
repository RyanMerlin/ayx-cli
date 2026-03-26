import json
from pathlib import Path

data = json.loads(Path("docs/swagger-v3.json").read_text())
for name in sorted(data["definitions"].keys()):
    if "share" in name.lower():
        print(name)
