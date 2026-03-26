from pathlib import Path

text = Path("ayx-cli/src/main.rs").read_text()
start = text.index("ApiCommand::DcmConnectionLookup")
print(text[start - 200:start + 400])
