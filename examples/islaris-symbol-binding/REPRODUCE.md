# Reproduce the example

Use Git, opam, a native compiler toolchain, GMP development headers, Python 3, ripgrep, Rust and Z3 with its development library. The recorded run used arm64 macOS, OCaml 4.14.2, Rust 1.100.0 and Z3 5.1.0. Start from the repository root with Bash.

```sh
git submodule update --init
repo=$PWD
example="$repo/examples/islaris-symbol-binding"
deps=$(mktemp -d)
```

## Sail and Isla

Build the native Sail compiler against its own 0.20.1 OCaml libraries. Dune 3.25.0 was used for this switch. Use a distinct switch for the proof.

```sh
opam switch create sailspan-sail ocaml-base-compiler.4.14.2
eval "$(opam env --switch=sailspan-sail --set-switch)"
opam install dune.3.25.0 libsail.0.20.1 sail_maker.0.20.1 ott.0.34
git clone --branch 0.20.1 --depth 1 https://github.com/rems-project/sail.git "$deps/sail"
(cd "$deps/sail" && dune build --release src/bin/sail.exe)
(cd "$repo/third_party/isla-exporter/isla-sail" && dune build --release sail_plugin_isla.cmxs)
cargo build --locked --bin isla-footprint --manifest-path "$repo/third_party/isla/Cargo.toml"

export SAIL="$deps/sail/_build/default/src/bin/sail.exe"
export SAIL_DIR="$deps/sail"
export ISLA_PLUGIN="$repo/third_party/isla-exporter/isla-sail/_build/default/sail_plugin_isla.cmxs"
export FOOTPRINT="$repo/third_party/isla/target/debug/isla-footprint"
```

## Parser and Islaris

The dependency commits below follow the upstream opam pins. The GitHub mirror URLs resolve the same commits as the upstream repositories. Dune 3.9.1 may need its source pin because the active opam index no longer serves that release.

```sh
opam switch create sailspan-proof ocaml-base-compiler.4.14.2
eval "$(opam env --switch=sailspan-proof --set-switch)"
opam repo add coq-released https://coq.inria.fr/opam/released
opam pin add --no-action dune.3.9.1 git+https://github.com/ocaml/dune.git#3.9.1

stdpp=git+https://github.com/rocq-iris/stdpp.git#a1a12e00fba07f2e41ccd74a8b593b88e48fc2c3
for pkg in coq-stdpp coq-stdpp-bitvector coq-stdpp-unstable; do
  opam pin add --no-action "$pkg.dev.2024-09-10.3.a1a12e00" "$stdpp"
done
opam pin add --no-action coq-iris.dev.2024-09-10.1.6f24ed4b \
  git+https://github.com/rocq-iris/iris.git#6f24ed4baa4d61967a95bac834eab2c66f3b76fa
opam pin add --no-action coq-lithium.dev.2024-09-11.0.7945a29d \
  git+https://github.com/UIC-verif-group/refinedC.git#7945a29d1647970709a9b5ad2ffc53c757e130cc
opam pin add --no-action isla-lang.dev \
  git+https://github.com/rems-project/isla-lang.git#bda86c9f0bd28bbaa2481f50ddc986ede342805a

opam install dune.3.9.1 coq.8.19.0 coq-record-update.0.3.3 coq-lithium \
  coq-stdpp-bitvector isla-lang ott.0.33 menhir.20240715 pprint.20230830 \
  cmdliner.1.3.0 integers.0.7.0
(cd "$repo/third_party/islaris" && dune build frontend/main.exe theories/isla.vo examples/ite_test.vo)
(cd "$example" && dune build bind.exe)

export ISLARIS="$repo/third_party/islaris/_build/default/frontend/main.exe"
export BINDER="$example/_build/default/bind.exe"
"$example/run.sh" "$deps/run"
"$example/prove.sh" "$deps/run" "$repo/third_party/islaris/_build/default/theories"
```

`examples/ite_test.vo` checks an upstream proof with the same Islaris build. The example run retains both original Trace files and stderr, raw Coq conversions, the freshened B trace used only for composition, event positions and state versions, the complete SMT-LIB input, and Z3's output including the negative models. `prove.sh` checks all generated Coq modules and prints the assumptions of the three instance theorems.

The upstream proof files use conventional architecture setup for instruction fetch. This example proves `WPasm` effects up to a supplied `tnil` continuation with the 128-bit model PC kept as a register. The arithmetic lemma `add_low_64` uses upstream stdpp bitvector lemmas to identify the trace's extract-after-add with a 64-bit modular add. The SMT queries independently check the relation's input coverage and both directions of normalization.
