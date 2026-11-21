"""Retained independent observer acceptance; company source is tested unchanged."""

import importlib.util
import sys
import unittest
from pathlib import Path

FIXTURES = Path(__file__).parent / "fixtures" / "showcase-observer"


def load_tests(loader, tests, pattern):
    for filename, name in [
        ("acceptance.py", "showcase_observer_acceptance"),
        ("company_tests.py", "showcase_observer_company_tests"),
        ("company_quality.py", "showcase_observer_company_quality"),
    ]:
        spec = importlib.util.spec_from_file_location(name, FIXTURES / filename)
        module = importlib.util.module_from_spec(spec)
        sys.modules[name] = module
        spec.loader.exec_module(module)
        tests.addTests(loader.loadTestsFromModule(module))
    return tests


if __name__ == "__main__":
    unittest.main()
