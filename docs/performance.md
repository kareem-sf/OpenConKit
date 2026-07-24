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

### v0.0.1 Windows baseline

Recorded 2026-07-24 for implementation commit
`33b4bb3f78600310692879aeec10b4c9a8facfd9`:

| Field               | Value                                                   |
| ------------------- | ------------------------------------------------------- |
| Host                | Windows 11 Pro 10.0.26200 (build 26200)                 |
| CPU                 | Intel Core i7-1165G7, 4 cores / 8 logical processors    |
| Memory              | 8,362,713,088 bytes                                     |
| Rust                | `rustc 1.97.0 (2d8144b78 2026-07-07)`                   |
| Power mode          | Balanced                                                |
| Background guards   | Defender real-time protection and Windows Search active |
| Target              | `x86_64-pc-windows-msvc`                                |
| Time estimate       | 1.6985 s (`[1.6388 s, 1.7691 s]`)                       |
| Throughput estimate | 2.9438 Kitems/s (`[2.8263, 3.0510]`)                    |
| Sample observations | 10, including 2 mild high outliers                      |

The exact invocation was:

```sh
cargo bench -p openconkit-tool-boq-inspector --bench boq_5000_rows \
  --target x86_64-pc-windows-msvc --locked
```

The Ubuntu native baseline remains to be captured on the release-equivalent
GitHub runner. A benchmark result is diagnostic, not a release promise.
Correctness, resource limits, progress, and cancellation tests remain hard
gates even if a timing regresses.
