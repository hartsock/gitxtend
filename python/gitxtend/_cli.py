"""The ``gitxtend`` console script.

This is a *shim*, deliberately. Argument parsing, output rendering and exit
codes all live in Rust (``src/cli.rs``) and are reached through the compiled
``cli_main``; this module only forwards argv in and the captured streams out.

Keeping it this thin is the point: the standalone ``gitxtend`` binary and this
console script execute the same parser, so they cannot disagree about a flag
name, an output line, or an exit code. There is nothing here to keep in sync.
"""

from __future__ import annotations

import sys
from typing import Sequence

from ._gitxtend import cli_main


def main(argv: Sequence[str] | None = None) -> int:
    """Entry point for the ``gitxtend`` console script and ``python -m gitxtend``.

    ``argv`` excludes the program name; it defaults to ``sys.argv[1:]``.
    Returns the process exit code (0 ok, 1 git failed, 2 bad usage).
    """
    args = list(sys.argv[1:] if argv is None else argv)
    code, out, err = cli_main(args)
    if out:
        sys.stdout.write(out)
        sys.stdout.flush()
    if err:
        sys.stderr.write(err)
        sys.stderr.flush()
    return code


if __name__ == "__main__":  # pragma: no cover - exercised via __main__.py
    raise SystemExit(main())
