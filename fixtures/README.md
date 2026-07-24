# Fixtures

Test fixtures for the spreadsheet ingestion and BOQ Inspector pipelines.

- `source-specs/` - human-readable specifications describing each synthetic
  fixture workbook (columns, planted defects, expected findings). Specs are
  the source of truth; workbooks are generated from them.
- `generated/` - generated XLSX fixtures (binary, reproducible from specs;
  not committed except for this placeholder).

Run `pnpm fixtures:generate` to rebuild all XLSX files. The Rust fixture
contract test generates the same workbooks in a temporary directory and
compares the complete finding-rule multiset, item-row count, and planted
literal formula-looking cells against each source spec.

Rationale: real Bills of Quantities are usually confidential. OpenConKit uses
synthetic fixtures with planted, documented defects so tests are shareable
and reviewable. See `docs/boq-research.md`.
