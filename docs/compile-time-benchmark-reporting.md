# Compile-Time Benchmark Reporting

Statum is a proc-macro crate. Compile-time cost is part of the user experience,
so performance claims need reproducible commands and ratios.

The repository already includes two compile fixtures outside the workspace:

- `benchmarks/compile/plain-fixture`
- `benchmarks/compile/statum-fixture`

Use them to compare a plain Rust shape against the equivalent Statum shape.

## Run The Benchmark

```bash
bash scripts/benchmark_compile.sh --iterations 5 --mode both
```

Modes:

- `cold`: removes the fixture target directory before each measured run.
- `warm`: primes the fixture once, then measures repeated checks.
- `both`: runs cold and warm measurements.

The script prints per-run milliseconds and averages for both fixtures. Compute the
ratio from the printed averages: `statum_avg / plain_avg` for the same mode.

## Report Format

When publishing benchmark numbers, include:

```text
Command: bash scripts/benchmark_compile.sh --iterations 5 --mode both
Machine: <CPU / OS / Rust toolchain if known>
Fixture: benchmarks/compile/{plain,statum}-fixture
Date: <date>

cold plain avg:  <ms>
cold statum avg: <ms>
cold ratio:      <statum/plain>x

warm plain avg:  <ms>
warm statum avg: <ms>
warm ratio:      <statum/plain>x
```

Do not quote raw milliseconds without the command, fixture, and ratio. Raw times
are machine-dependent; ratios are still imperfect, but they are easier to track
across runs.

## Interpretation

Use compile-time results as a regression signal, not as a universal promise.
The fixture is useful because it is stable and repeatable, but it is not every
possible Statum workload.

Call out the observed surface precisely:

- fixture shape,
- cold or warm mode,
- iteration count,
- Rust toolchain,
- whether `strict-introspection` was part of the measured command.

If a future benchmark measures strict introspection separately, document the
exact feature flags and fixture before making an authority claim.

## Release Checklist

Before a performance-sensitive release:

1. Run the benchmark with at least five iterations.
2. Paste the report format into the release notes or benchmark log.
3. Compare against the previous report if one exists.
4. Investigate large ratio changes before shipping.
5. Avoid claiming general compile-time behavior from one fixture.
