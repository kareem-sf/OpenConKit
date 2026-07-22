# BOQ research notes (skeleton)

Research backing the BOQ Inspector's checks. Public sources and standards
will be recorded here as they are reviewed during the detection-engine
phase.

## Fixture approach

Real Bills of Quantities are usually commercially confidential, so the test
suite does not (and will not) include real project documents. Instead:

1. `fixtures/source-specs/` holds human-readable specs of synthetic BOQ
   workbooks: column layouts inspired by common practice, plus deliberately
   planted defects (duplicate item codes, broken totals, missing units,
   inconsistent rates, ...).
2. A generator (built in the detection-engine phase) produces the actual
   XLSX files into `fixtures/generated/` (not committed).
3. Each spec declares the expected findings, so detection rules are tested
   against documented ground truth.

## Candidate check categories (to be validated against sources)

- Structural: header detection, merged-cell hazards, multi-sheet layouts.
- Referential: duplicate/missing item codes, orphaned subtotals.
- Arithmetic: quantity x rate mismatches, broken rollup totals, rounding.
- Completeness: missing units, missing descriptions, empty priced rows.
- Consistency: unit normalization (m2 vs sq.m), currency mixing.

Public methodology references (e.g. standard methods of measurement such as
CESMM, NRM, POMI) will be cited here with links and notes once reviewed.
