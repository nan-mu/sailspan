// Run one encoded ALU instruction through the upstream decoder and executor.
function sailspan_footprint(word : bits(64)) -> unit = {
  match execute(ext_decode(word)) {
    Retire_Success() => (),
    _ => assert(false, "eBPF instruction did not retire normally"),
  }
}
