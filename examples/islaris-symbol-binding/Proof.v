From iris.proofmode Require Import tactics.
From isla Require Import isla.
Require Import A B BFresh.

Fixpoint flat_bitvector_declarations (trace : isla_trace) : bool :=
  match trace with
  | tnil => true
  | tcons (Smt (DeclareConst _ (Ty_BitVec _)) _) next =>
      flat_bitvector_declarations next
  | tcons (Smt (DeclareConst _ _) _) _ => false
  | tcons _ next => flat_bitvector_declarations next
  | tcases _ => false
  end.

Example recorded_declarations_are_bitvectors :
  flat_bitvector_declarations trace_A &&
  flat_bitvector_declarations trace_B = true.
Proof. reflexivity. Qed.

(* Islaris's dump frontend discards DefineEnum metadata. The full direct
   conversion retains it. The eBPF runs declare no enum-sorted SMT symbols.
   Keep all register events, including the HostLittleEndian precondition. *)
Fixpoint executable_prefix (source tail : isla_trace) : isla_trace :=
  match source with
  | tnil => tail
  | tcons (Smt (DefineEnum _ _ _) _) next => executable_prefix next tail
  | tcons event next => tcons event (executable_prefix next tail)
  | tcases branches => tcases (map (fun branch => executable_prefix branch tail) branches)
  end.

Fixpoint bound_symbols (trace : isla_trace) : list var_name :=
  match trace with
  | tnil => []
  | tcons (Smt (DeclareConst id _) _) next
  | tcons (Smt (DefineConst id _) _) next => id :: bound_symbols next
  | tcons _ next => bound_symbols next
  | tcases branches => concat (map bound_symbols branches)
  end.

Definition rename_event (offset : Z) (symbols : list var_name) (evt : event) : event :=
  let fresh id := (id + offset)%Z in
  let renamed := fold_left
    (fun current id => subst_val_event (Val_Symbolic (fresh id)) id current)
    symbols evt in
  match renamed with
  | Smt (DeclareConst id ty) annot => Smt (DeclareConst (fresh id) ty) annot
  | Smt (DefineConst id value) annot => Smt (DefineConst (fresh id) value) annot
  | other => other
  end.

Fixpoint rename_trace (rename : event -> event) (trace : isla_trace) : isla_trace :=
  match trace with
  | tnil => tnil
  | tcons evt next => tcons (rename evt) (rename_trace rename next)
  | tcases branches => tcases (map (rename_trace rename) branches)
  end.

Definition freshen_after (first second : isla_trace) : isla_trace :=
  let symbols := bound_symbols second in
  let offset := (1 + fold_left Z.max (bound_symbols first ++ symbols) 0)%Z in
  rename_trace (rename_event offset symbols) second.

Example recorded_add64_freshening :
  trace_B_fresh = freshen_after trace_A trace_B.
Proof. reflexivity. Qed.

Lemma add_low_64 (value : bv 64) :
  bv_extract 0 64 (bv_add (bv_zero_extend 128 value) (BV 128 4)) =
  bv_add_Z value 4.
Proof.
  apply bv_eq.
  rewrite bv_extract_0_unsigned bv_add_unsigned bv_add_Z_unsigned.
  rewrite bv_zero_extend_unsigned //.
  by rewrite bv_wrap_bv_wrap.
Qed.

Section Effects.
  Context `{!Arch} `{!islaG Σ} `{!threadG}.

  Ltac retire :=
    repeat (lazymatch goal with
    | |- environments.envs_entails _ (WPasm tnil) => fail
    | _ => liAStep
    end);
    liShow.

  Definition state (r1 r2 : bv 64) (pc : bv 128) : iProp Σ :=
    ("host_endian" ↦ᵣ RegVal_Base (Val_Enum "HostLittleEndian") ∗
     "r1" ↦ᵣ RVal_Bits r1 ∗
     "r2" ↦ᵣ RVal_Bits r2 ∗
     "PC" ↦ᵣ RVal_Bits pc)%I.

  (* A continuation at tnil describes what comes after the trace, without
     identifying the model's 128-bit PC with Islaris's fetch address. *)
  Lemma mov64_wp x y pc :
    state x y pc -∗
    (state x x (bv_add pc (BV 128 1)) -∗ WPasm tnil) -∗
    WPasm (executable_prefix trace_A tnil).
  Proof.
    unfold state.
    iIntros "(Hend & Hr1 & Hr2 & Hpc) Hnext".
    cbn [executable_prefix trace_A].
    retire. iApply "Hnext". iFrame.
  Qed.

  Lemma add64_wp x y pc :
    state x y pc -∗
    (state x (bv_add_Z y 4) (bv_add pc (BV 128 1)) -∗ WPasm tnil) -∗
    WPasm (executable_prefix trace_B tnil).
  Proof.
    unfold state.
    iIntros "(Hend & Hr1 & Hr2 & Hpc) Hnext".
    cbn [executable_prefix trace_B].
    retire. iApply "Hnext". iFrame.
    rewrite -add_low_64. iAssumption.
  Qed.

  Lemma mov64_add64_wp x y pc :
    state x y pc -∗
    (state x (bv_add_Z x 4) (bv_add (bv_add pc (BV 128 1)) (BV 128 1)) -∗ WPasm tnil) -∗
    WPasm (executable_prefix trace_A
      (executable_prefix (freshen_after trace_A trace_B) tnil)).
  Proof.
    rewrite -recorded_add64_freshening.
    unfold state.
    iIntros "(Hend & Hr1 & Hr2 & Hpc) Hnext".
    cbn [executable_prefix trace_A trace_B_fresh].
    retire. iApply "Hnext". iFrame.
    rewrite -add_low_64. iAssumption.
  Qed.
End Effects.

Print Assumptions mov64_wp.
Print Assumptions add64_wp.
Print Assumptions mov64_add64_wp.
