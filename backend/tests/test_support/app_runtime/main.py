"""Test-support FastAPI runtime shell retained after production Python runtime exit."""

from tests.test_support.app_runtime.app_factory import create_app

app = create_app()

__all__ = ["app", "create_app"]
