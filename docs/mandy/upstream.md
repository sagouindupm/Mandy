# Following upstream (XCL)

Mandy is XCL with its name changed, plus our own commits. The rename is mechanical and repeatable, so
upstream's new work can be taken at any time.

## Branches

| Branch | What it holds | Rule |
|---|---|---|
| `main` | Mandy: the renamed upstream plus our changes | Changed through pull requests only |
| `upstream/xcl` | An exact copy of `ivall/xcl` `main` | Never edited by hand |
| `vendor/xcl-renamed` | `upstream/xcl` after `tools/rename_upstream.py`, nothing else | Never edited by hand |

## Taking upstream's new commits

```sh
git fetch https://github.com/ivall/xcl.git main:upstream/xcl
git checkout vendor/xcl-renamed
git merge -s ours --no-commit upstream/xcl        # records upstream as a parent
git read-tree -u --reset upstream/xcl             # the tree becomes upstream's
python tools/rename_upstream.py .                 # (script taken from main)
git add -A && git commit -m "Vendor: upstream xcl <sha>, renamed"
git checkout main && git merge vendor/xcl-renamed
```

The last merge only conflicts where we changed the same lines as upstream.

## Rules

- **One code base.** Mandy is GPL-2.0-only, like XCL. Code from projects under an incompatible license
  (GPL-3.0, Apache-2.0 without an alternative) is never copied in; an idea can be reimplemented.
- **No game files** in this repository: no executable, archive, extracted shader or dump.
- **No local configuration, paths or raw logs.**
- Links to upstream issues and pull requests (`ivall/xcl`) are kept as they are.
- Any GPU vendor is in scope. A change comes with the result of a real run: GPU, duration, what shows.
