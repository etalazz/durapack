# CLI test scripts

This folder contains helper scripts for exercising the `durapack` CLI on Windows.

- `test-cli.ps1` — smoke-tests `pack`, `scan`, `verify`, and `timeline` end-to-end.

## Requirements

- PowerShell 5+ (Windows PowerShell or PowerShell Core)
- A working Rust toolchain (`cargo`) in PATH

## Usage

From the repo root, run one of the following:

```powershell
# PowerShell Core or Windows PowerShell
pwsh -File scripts/test-cli.ps1
# or
powershell -ExecutionPolicy Bypass -File scripts\test-cli.ps1
```

The script will:

1. Build `durapack-cli`
2. Create temporary JSON inputs (array and JSONL)
3. Pack to several `.durp` files using stdin and file-based input
4. Scan into JSON and JSONL (and carve payloads)
5. Verify using file and stdin
6. Produce a Graphviz DOT timeline and a JSON timeline

All artifacts are written under `scripts/out/`.

