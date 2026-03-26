from pathlib import Path

text = Path("ayx-cli/src/main.rs").read_text()
start = text.index("ApiCommand::DcmConnectionLookup")
end = text.index("ApiCommand::DcmAdminConnections")
snippet = text[start:end]
print(repr(snippet))
