# Recorded execution

| Goal | Artifact |
| --- | --- |
| G1 | [Inputs](inputs.tsv), [commands](commands.log), [binary and IR hashes](versions.log), raw [MOV64](A.itl) and [ADD64](B.itl) with [A stderr](A.log) and [B stderr](B.log). Both runs and both direct conversions exited 0. The unavailable tag-memory primop warnings appear in stderr. Neither arithmetic trace uses memory. |
| G2 | Islaris's raw Coq conversions [A](A.v) and [B](B.v). The stock Islaris ITE proof and frontend build status are in [islaris-stock.log](islaris-stock.log). |
| G3 | [State and binding log](binding.log), [baseline and normalized relations](relations.smt2), [consistent B freshening](B-fresh.itl), and [width mutation rejection](wrong-width.log) on the [derived trace](B-wrong-width.itl). |
| G4 | [Instance proof](../Proof.v), [Coq 8.19.0 output](proof.log), [freshened B conversion](BFresh.v) and [Z3 checking log](check.log) with models for all three incorrect links. |

All eBPF files come from one run of the [reproduction entry points](../REPRODUCE.md). Paths in command and hash logs identify the original build environment. The raw ITL stores source file indices. The recorded IR hash is specific to the generated build.
