# Performance benchmarks

OpenConKit makes no universal speed claim. The repeatable benchmark measures
the full BOQ Inspector path—XLSX preflight and parsing, structure detection,
normalization, deterministic rules, finding construction, and normalized
output—for a generated 5,000-item workbook.

Run it on an otherwise idle native host:

```sh
cargo bench -p openconkit-tool-boq-inspector --bench boq_5000_rows
```

Criterion uses 10 samples, a 2-second warm-up, and an 8-second measurement
window. The workload is generated outside the measured loop and deleted
afterward. It uses unique item descriptions, valid arithmetic, and the
default safety/rule settings. Results include host, power mode, Rust version,
commit, and whether antivirus/indexing was active; compare commits only on
the same host configuration.

## Baseline record

The v0.0.1 release baseline must be recorded here after running the command
on the Windows 11 development host and once on an `ubuntu-22.04` GitHub
runner. A benchmark result is diagnostic, not a release promise. Correctness,
resource limits, progress, and cancellation tests remain hard gates even if a
timing regresses.
