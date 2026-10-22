"""Check the scope of the temporary tag-cleanup validation checkout."""
from pathlib import Path
import json

paths = json.loads(Path('/tmp/pr246-changed.json').read_text())
assert paths and all(path.startswith(('src/', 'tests/')) or path in ('README.md', 'CHANGELOG.md') for path in paths)
print('Preparing only source, tests, and documentation:', ', '.join(paths))
