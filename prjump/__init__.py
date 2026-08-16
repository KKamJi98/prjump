"""Pull Request Jump - open GitHub/Bitbucket pull request pages."""

from importlib.metadata import PackageNotFoundError, version

# Read from the installed distribution rather than repeating the number here.
# 0.2.0 shipped to PyPI announcing itself as `prj 0.1.0` because this line was a
# second source of truth and only one of the two got bumped.
try:
    __version__ = version("prjump")
except PackageNotFoundError:  # running from a source tree with nothing installed
    __version__ = "0.0.0+unknown"
