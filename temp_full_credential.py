import json
from pathlib import Path

data = json.loads(Path("docs/swagger-v3.json").read_text())
print(json.dumps(data["paths"]["/v3/credentials/{credentialId}/users"], indent=2))
