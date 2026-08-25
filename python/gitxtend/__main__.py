"""``python -m gitxtend`` — same command as the ``gitxtend`` console script.

No-cover: this module raises ``SystemExit`` at import time, so it is only
reachable from a subprocess, where coverage does not follow. Its behaviour is
pinned by `test_python_dash_m_runs_the_same_command`.
"""

from ._cli import main  # pragma: no cover

raise SystemExit(main())  # pragma: no cover
