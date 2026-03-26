import json
from pathlib import Path

data = json.loads(Path("docs/swagger-v3.json").read_text())

for path, ops in data["paths"].items():
    if "sharing" in path:
        print(path)
        for method, op in ops.items():
            print(" ", method.upper(), op.get("summary"))
        print()
