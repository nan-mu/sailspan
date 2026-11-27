#!/usr/bin/env bash
set -euo pipefail
example=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
if [[ $# != 2 ]]; then
  echo "usage: $0 example-output-directory islaris-compiled-theories-directory" >&2
  exit 2
fi
theories=$(cd -- "$2" && pwd)
cd -- "$1"
cp -- "$example/Proof.v" Proof.v
{
  coqc --version
  for unit in A B BFresh Proof; do
    coqc -q -Q "$theories" isla "$unit.v" || exit
  done
} 2>&1 | tee proof.log
