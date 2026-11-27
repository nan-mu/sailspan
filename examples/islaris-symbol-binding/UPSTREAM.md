# Upstream reuse

| Component | Pin and where it is called |
| --- | --- |
| Sail eBPF | [`767a79b`](https://github.com/OpenSourceVerif/sail-ebpf/tree/767a79b954aeec91464303220a65f92008cbdb8f), the `ebpf26-artifact` branch commit. The adapter calls `ext_decode` in `model/main/main.sail` and `execute` in `model/alu/alu_insts.sail`. The artifact tree provides no license declaration. |
| Sail | [0.20.1](https://github.com/rems-project/sail/releases/tag/0.20.1), the model's documented compiler version. BSD-2-Clause. |
| Isla exporter | [`f742610`](https://github.com/rems-project/isla/tree/f7426103f710f7c7f9191651c55b7a0320b87ce1/isla-sail), `sail_plugin_isla.ml` and `jib_ir.ml` generate the IR. The Islaris-tested Isla revision's plugin fails against Sail 0.20.1 because the Jib `CL_field` constructor changed. The January 2026 plugin builds unchanged. BSD-2-Clause. |
| Isla executor | [`b8e614b`](https://github.com/rems-project/isla/tree/b8e614bda4a20e42c37d8b216853653133d04322), the Islaris-tested engine. `src/footprint.rs` runs each word in a separate process and `isla-lib/src/simplify.rs` serializes the full event tree. Its parser consumes the January plugin's IR for this example. BSD-2-Clause. |
| isla-lang | [`bda86c9`](https://github.com/rems-project/isla-lang/tree/bda86c9f0bd28bbaa2481f50ddc986ede342805a), Islaris's opam pin. `bind.ml` uses the Ott-generated parser, AST, pretty-printer and `subst_val_event`. BSD-2-Clause in source headers. |
| Islaris | [`c978e10`](https://github.com/rems-project/islaris/tree/c978e10f50db5c40f0fdf113f5f76a779782c6f9). `frontend/main.ml` calls `Isla_lang_if.Parser.parse_file` and `Coq_pp.write_trace` in direct mode. `theories/opsem.v`, `lifting.v` and `automation.v` supply the trace objects, register rules and `liAStep` used by the proof. Its LICENSE says BSD-2-Clause and its opam file says BSD-3-Clause. |
| Coq libraries | [Lithium `7945a29`](https://github.com/UIC-verif-group/refinedC/tree/7945a29d1647970709a9b5ad2ffc53c757e130cc), [Iris `6f24ed4`](https://github.com/rocq-iris/iris/tree/6f24ed4baa4d61967a95bac834eab2c66f3b76fa), [stdpp `a1a12e0`](https://github.com/rocq-iris/stdpp/tree/a1a12e00fba07f2e41ccd74a8b593b88e48fc2c3) and RecordUpdate 0.3.3 follow the Islaris and Lithium opam constraints. The Iris GitHub organization mirrors the upstream GitLab projects. The RefinedC mirror contains the same pinned commit from before its downstream changes. These libraries use BSD-3-Clause. |

The source submodules preserve the model, exporter, executor and Islaris pins. The exporter is compiled against the same native OCaml Sail libraries as the compiler. The proof switch installs one copy of isla-lang, including the generated Coq syntax consumed by Islaris.

## Adapter and scope

- The CLI treats eight-byte hex input as most-significant-byte first. `run.sh` reverses the machine bytes before passing them to Isla. Sail still decodes the two complete words.
- `--no-model-reg-init` keeps GPRs and PC symbolic. Both processes use a concrete little-endian setting recorded as an `AssumeReg` and a read. The trace command leaves `--simplify`, `--hide` and `--executable` off. An exploratory simplified run dropped the effects because this adapter does not emit a cycle marker.
- Islaris dump mode handles AArch64 and RISC-V in its architecture table. Direct mode reads the raw eBPF ITL and emits Coq without the dump frontend's width and architecture checks.
- Islaris `trace_step` has no rule for `DefineEnum`. Its dump frontend normally drops those records. The raw ITL and raw conversion here retain them. The proof projects out only these metadata records, keeping the endianness read and assumption. Both traces have zero enum-sorted symbolic declarations.
- The Sail model uses an integer slot PC, represented by Isla here as a 128-bit bitvector. The composition connects that PC explicitly. The Coq obligations stop at the trace continuation, before Islaris's 64-bit instruction-fetch convention. The bitvector checks express modulo-`2^128` behavior at the ITL layer.

Source inspection and an independent probe also reproduced the first AArch64 `hello.dump` instruction using the [Islaris-tested snapshot](https://github.com/rems-project/isla-snapshots/tree/b58da9170470a422c9396983ac8f87f0a63ba6f8). The eBPF generator uses the submodule's model and has no snapshot dependency.
