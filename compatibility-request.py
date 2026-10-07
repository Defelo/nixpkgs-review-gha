"""The build compatibility caller cannot authorize publication."""
import json
from pathlib import Path
import sys


def validate(request):
    if request.get("publication") != "none":
        raise ValueError("Publication requests must use review-repository.yml directly")


if __name__ == "__main__":
    validate(json.loads(Path(sys.argv[1]).read_text()))
