From isla Require Import opsem.

Definition trace_B_fresh : isla_trace :=
  AssumeReg "host_endian" [] (RegVal_Base (Val_Enum "HostLittleEndian")) Mk_annot :t:
  Smt (DefineEnum "ebpf_host_endian" 2%Z ["HostLittleEndian"; "HostBigEndian"]) Mk_annot :t:
  ReadReg "host_endian" [] (RegVal_Base (Val_Enum "HostLittleEndian")) Mk_annot :t:
  Smt (DefineEnum "alu_code" 14%Z ["ADD"; "SUB"; "MUL"; "DIV_OR_SDIV"; "OR"; "AND"; "LSH"; "RSH"; "NEG"; "MOD_OR_SMOD"; "XOR"; "MOV_OR_MOVSX"; "ARSH"; "END"]) Mk_annot :t:
  Smt (DefineEnum "src_kind" 2%Z ["SRC_K"; "SRC_X"]) Mk_annot :t:
  Smt (DefineEnum "alu_class" 2%Z ["ALU32"; "ALU64"]) Mk_annot :t:
  Smt (DeclareConst 6%Z (Ty_BitVec 64%N)) Mk_annot :t:
  ReadReg "r2" [] (RegVal_Base (Val_Symbolic 6%Z)) Mk_annot :t:
  Smt (DefineConst 7%Z (Unop (ZeroExtend 64%N) (Val (Val_Symbolic 6%Z) Mk_annot) Mk_annot)) Mk_annot :t:
  Smt (DefineConst 8%Z (Manyop (Bvmanyarith Bvadd) [Val (Val_Symbolic 7%Z) Mk_annot; Val (Val_Bits (BV 128%N 0x4%Z)) Mk_annot] Mk_annot)) Mk_annot :t:
  Smt (DefineConst 9%Z (Unop (Extract 63%N 0%N) (Val (Val_Symbolic 8%Z) Mk_annot) Mk_annot)) Mk_annot :t:
  WriteReg "r2" [] (RegVal_Base (Val_Symbolic 9%Z)) Mk_annot :t:
  Smt (DeclareConst 10%Z (Ty_BitVec 128%N)) Mk_annot :t:
  ReadReg "PC" [] (RegVal_Base (Val_Symbolic 10%Z)) Mk_annot :t:
  Smt (DefineConst 11%Z (Manyop (Bvmanyarith Bvadd) [Val (Val_Symbolic 10%Z) Mk_annot; Val (Val_Bits (BV 128%N 0x1%Z)) Mk_annot] Mk_annot)) Mk_annot :t:
  WriteReg "PC" [] (RegVal_Base (Val_Symbolic 11%Z)) Mk_annot :t:
  tnil
.
