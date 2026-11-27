#!/usr/bin/env bash
# Run from any directory. The configured tool paths are absolute.
set -euo pipefail

example=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd -- "$example/../.." && pwd)

if [[ $# != 1 ]]; then
  echo "usage: $0 new-output-directory" >&2
  exit 2
fi

: "${SAIL:?Set SAIL to the Sail 0.20.1 executable}"
: "${ISLA_PLUGIN:?Set ISLA_PLUGIN to the compiled Isla Sail plugin}"
: "${FOOTPRINT:?Set FOOTPRINT to the pinned isla-footprint executable}"
: "${ISLARIS:?Set ISLARIS to the pinned Islaris converter}"
: "${BINDER:?Set BINDER to the compiled bind.exe}"
model=${EBPF_MODEL_DIR:-"$repo/third_party/sail-ebpf/model"}

mkdir -- "$1"
out=$(cd -- "$1" && pwd)

(
  cd -- "$model"
  file_list=$("$SAIL" --all-modules --list-files-separated ' ' ebpf.sail_project)
  read -r -a sources <<< "$file_list"
  "$SAIL" --plugin "$ISLA_PLUGIN" --isla --isla-preserve sailspan_footprint \
    --strict-var --strict-bitvector --strict-exponentials --no-memo-z3 \
    "${sources[@]}" "$example/footprint.sail" -o "$out/ebpf"
) >"$out/export.log" 2>&1

{
  "$SAIL" --version
  z3 --version
  if [[ -f "$model/../.git" || -d "$model/../.git" ]]; then
    printf 'Model checkout %s\n' "$(git -C "$model" rev-parse HEAD)"
  fi
  printf 'Source pins for this checkout\n'
  git -C "$repo" ls-tree HEAD -- third_party/isla third_party/isla-exporter third_party/islaris third_party/sail-ebpf
  if [[ -f "$repo/third_party/islaris/islaris.opam" ]]; then
    printf 'Islaris parser pin\n'
    rg -F 'isla-lang.dev' "$repo/third_party/islaris/islaris.opam"
  fi
  shasum -a 256 "$out/ebpf.ir" "$SAIL" "$ISLA_PLUGIN" "$FOOTPRINT" "$ISLARIS" "$BINDER" \
    "$example/ebpf.toml" "$example/footprint.sail"
} > "$out/versions.log"

trace() {
  local id=$1 bytes=$2 word
  word=$(python3 -c 'import sys; print(bytes.fromhex(sys.argv[1])[::-1].hex())' "$bytes")
  printf '%s\t%s\t%s\n' "$id" "$bytes" "$word" >> "$out/inputs.tsv"

  local cmd=("$FOOTPRINT" --arch "$out/ebpf.ir" --config "$example/ebpf.toml"
    --threads 1 --no-model-reg-init --pessimistic --tree --hex
    --function sailspan_footprint --instruction "$word")
  {
    printf '%q' "${cmd[0]}"
    printf ' %q' "${cmd[@]:1}"
    printf '\n'
  } >> "$out/commands.log"
  "${cmd[@]}" > "$out/$id.itl" 2> "$out/$id.log"
  test -s "$out/$id.itl"
  "$ISLARIS" --isla -n "trace_$id" -o "$out/$id.v" "$out/$id.itl" \
    > "$out/$id.convert.log" 2>&1
}

printf 'trace\tmemory_bytes\tisla_word\n' > "$out/inputs.tsv"
trace A bf12000000000000
trace B 0702000004000000
"$BINDER" "$out/A.itl" "$out/B.itl" "$out/relations.smt2" "$out/B-fresh.itl" \
  "$out/B-wrong-width.itl" > "$out/binding.log"
"$ISLARIS" --isla -n trace_B_fresh -o "$out/BFresh.v" "$out/B-fresh.itl" \
  > "$out/B-fresh.convert.log" 2>&1
python3 "$example/check.py" "$out/relations.smt2" | tee "$out/check.log"

# The binder derives this mutation from B's parsed r2 entry.
if "$BINDER" "$out/A.itl" "$out/B-wrong-width.itl" "$out/wrong-width.smt2" "$out/wrong-width-fresh.itl" \
    > "$out/wrong-width.log" 2>&1; then
  echo "The binding accepted the derived width mismatch" >&2
  exit 1
fi
rg -Fq "Incompatible state sorts" "$out/wrong-width.log"
