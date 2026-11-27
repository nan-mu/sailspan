open Isla_lang.AST

module IntMap = Map.Make (Int)
module Registers = Map.Make (String)

type sort = Bits of int | Bool | Endian
type term = { code : string; sort : sort }
type state_value = { term : term; version : int; event : int }
type entry_read = { ordinal : int; value : term; symbol : int option }

type trace = {
  symbols : term IntMap.t;
  declarations : (int * term) list;
  aliases : term IntMap.t;
  definitions : (term * term) list;
  conditions : string list;
  registers : state_value Registers.t;
  reads : entry_read Registers.t;
}

let require condition message = if not condition then failwith message
let bit width code = { code; sort = Bits width }

let smt_sort = function
  | Bits width -> Printf.sprintf "(_ BitVec %d)" width
  | Bool -> "Bool"
  | Endian -> "Endian"

let equal left right =
  require (left.sort = right.sort)
    (Printf.sprintf "Incompatible state sorts for %s and %s" left.code right.code);
  Printf.sprintf "(= %s %s)" left.code right.code

let register_state name term =
  (name, { term; version = 0; event = 0 })

let initial_registers =
  Registers.of_seq (List.to_seq [
    register_state "r1" (bit 64 "r1_in");
    register_state "r2" (bit 64 "r2_in");
    register_state "PC" (bit 128 "pc_in");
    register_state "host_endian" { code = "endian_in"; sort = Endian };
  ])

let register registers name accessors =
  require (accessors = Nil) ("Unsupported register field on " ^ name);
  match Registers.find_opt name registers with
  | Some state -> state
  | None -> failwith ("Register outside the example interface: " ^ name)

let base_value trace = function
  | Val_Symbolic id ->
      (match IntMap.find_opt id trace.symbols with
       | Some term -> term
       | None -> failwith (Printf.sprintf "Undeclared symbol v%d" id))
  | Val_Bits literal ->
      let digit_width = if literal.[1] = 'x' then 4 else 1 in
      let width = (String.length literal - 2) * digit_width in
      require (width > 0) "Empty bitvector literal";
      bit width literal
  | Val_Bool value -> { code = string_of_bool value; sort = Bool }
  | Val_Enum ("HostLittleEndian" | "HostBigEndian" as value) ->
      { code = value; sort = Endian }
  | Val_Enum value -> failwith ("Enum value outside the example interface: " ^ value)

let register_value trace = function
  | RegVal_Base value -> base_value trace value
  | _ -> failwith "Unsupported register value"

let width term =
  match term.sort with
  | Bits size -> size
  | _ -> failwith ("Expected a bitvector: " ^ term.code)

let rec expression trace = function
  | Val (value, _) -> base_value trace value
  | Unop (ZeroExtend count, argument, _) ->
      let term = expression trace argument in
      require (count >= 0) "Negative zero extension";
      bit (width term + count) (Printf.sprintf "((_ zero_extend %d) %s)" count term.code)
  | Unop (Extract (hi, lo), argument, _) ->
      let term = expression trace argument in
      require (0 <= lo && lo <= hi && hi < width term) "Invalid extraction bounds";
      bit (hi - lo + 1) (Printf.sprintf "((_ extract %d %d) %s)" hi lo term.code)
  | Manyop (Bvmanyarith Bvadd, (first :: second :: rest), _) ->
      let left = expression trace first in
      let size = width left in
      List.fold_left (fun acc argument ->
        let right = expression trace argument in
        require (width right = size) "Addition operands have different widths";
        bit size (Printf.sprintf "(bvadd %s %s)" acc.code right.code))
        left (second :: rest)
  | Binop (Eq, left, right, _) ->
      { code = equal (expression trace left) (expression trace right); sort = Bool }
  | _ -> failwith "Unsupported ITL expression"

let parse path =
  let input = open_in path in
  let parsed = Fun.protect ~finally:(fun () -> close_in input) (fun () ->
    let lexbuf = Lexing.from_channel input in
    Lexing.set_filename lexbuf path;
    Isla_lang.Parser.whole_tree_start Isla_lang.Lexer.token lexbuf) in
  match parsed with
  | BareTree (TreeTrace (events, End))
  | TreeWithSegments (Segments [], TreeTrace (events, End)) -> events
  | _ -> failwith (path ^ ": expected one flat, completed trace without segments")

let pp_event event = PPrint.ToChannel.pretty 0.9 108 stdout (Isla_lang.PP.pp_event event)

(* Assign names to bindings, rather than replacing text in the input ITL. *)
let extract namespace aliases registers events =
  let symbol id sort = { code = Printf.sprintf "%s_v%d" namespace id; sort } in
  let bind trace id term =
    require (not (IntMap.mem id trace.symbols)) "Duplicate local symbol";
    { trace with symbols = IntMap.add id term trace.symbols }
  in
  let constrain trace condition = { trace with conditions = condition :: trace.conditions } in
  let initial = {
    symbols = IntMap.empty; declarations = []; aliases = IntMap.empty;
    definitions = []; conditions = []; registers; reads = Registers.empty;
  } in
  List.fold_left (fun trace (index, event) ->
    let ordinal = index + 1 in
    try match event with
    | Smt (DeclareConst (id, Ty_BitVec size), _) ->
        require (size > 0) "Invalid declared width";
        let term = symbol id (Bits size) in
        { (bind trace id term) with declarations = (id, term) :: trace.declarations }
    | Smt (DefineConst (id, expr), _) ->
        let value = expression trace expr in
        let term = symbol id value.sort in
        { (bind trace id term) with definitions = (term, value) :: trace.definitions }
    | Smt (DefineEnum ("ebpf_host_endian", 2, ["HostLittleEndian"; "HostBigEndian"]), _) ->
        trace
    | Smt (DefineEnum (("alu_code" | "src_kind" | "alu_class"), _, _), _) ->
        (* These decoder metadata sorts do not occur as values in this relation. *)
        trace
    | Smt (Assert expr, _) ->
        let condition = expression trace expr in
        require (condition.sort = Bool) "Assert must be Boolean";
        constrain trace condition.code
    | AssumeReg (name, accessor, value, _) ->
        let current = register trace.registers name accessor in
        constrain trace (equal current.term (register_value trace value))
    | ReadReg (name, accessor, value, _) ->
        let current = register trace.registers name accessor in
        let term = register_value trace value in
        let trace = constrain trace (equal current.term term) in
        if current.version <> 0 || Registers.mem name trace.reads then trace
        else
          let symbol = match value with
            | RegVal_Base (Val_Symbolic id) -> Some id
            | _ -> None
          in
          let bindings = match symbol with
            | Some id when List.mem name aliases && List.mem_assoc id trace.declarations
                           && not (IntMap.mem id trace.aliases) ->
                IntMap.add id current.term trace.aliases
            | _ -> trace.aliases
          in
          { trace with aliases = bindings;
            reads = Registers.add name { ordinal; value = term; symbol } trace.reads }
    | WriteReg (name, accessor, value, _) ->
        let current = register trace.registers name accessor in
        let term = register_value trace value in
        require (current.term.sort = term.sort) ("Incompatible write sort for " ^ name);
        { trace with registers = Registers.add name
            { term; version = current.version + 1; event = ordinal } trace.registers }
    | _ -> failwith "Unsupported ITL event"
    with Failure message ->
      failwith (Printf.sprintf "%s event %d: %s" namespace ordinal message))
    initial (List.mapi (fun index event -> (index, event)) events)

let conjunction terms =
  "(and true " ^ String.concat " " terms ^ ")"

let quantify variables body =
  match variables with
  | [] -> body
  | _ ->
      let declaration term = Printf.sprintf "(%s %s)" term.code (smt_sort term.sort) in
      Printf.sprintf "(exists (%s) %s)" (String.concat " " (List.map declaration variables)) body

let define name value body = Printf.sprintf "(let ((%s %s)) %s)" name.code value.code body

let scope trace body =
  let body = conjunction (body :: List.rev trace.conditions) in
  (* Definitions depend on earlier definitions, so their lets are ordered. *)
  let body = List.fold_left (fun body (name, value) -> define name value body) body trace.definitions in
  let variables, body = List.fold_left (fun (variables, body) (id, name) ->
    match IntMap.find_opt id trace.aliases with
    | Some value -> (variables, define name value body)
    | None -> (name :: variables, body)) ([], body) trace.declarations in
  quantify variables body

let selected_registers = [
  ("r1", bit 64 "r1_out");
  ("r2", bit 64 "r2_out");
  ("PC", bit 128 "pc_out");
  ("host_endian", { code = "endian_out"; sort = Endian });
]

type connection = Sequential | MissingR2 | BExit | AEntry

let compose mode connection trace_a trace_b =
  let a = extract "A" [] initial_registers trace_a in
  let b_r2 = bit 64 "b_r2_in" and b_pc = bit 128 "b_pc_in" in
  let a_exit name = (Registers.find name a.registers).term in
  let normalized = mode = `Normalized in
  let inputs = if normalized then [("r2", a_exit "r2"); ("PC", a_exit "PC")]
               else [("r2", b_r2); ("PC", b_pc)] in
  let b_registers = List.fold_left (fun regs (name, value) ->
    Registers.add name { term = value; version = 0; event = 0 } regs)
    a.registers inputs in
  let b = extract "B" (if normalized then ["r2"; "PC"] else []) b_registers trace_b in
  let b_exit name = (Registers.find name b.registers).term in
  let links = match connection with
    | Sequential -> [equal b_r2 (a_exit "r2")]
    | MissingR2 -> []
    | BExit -> [equal (b_exit "r2") (a_exit "r2")]
    | AEntry -> [equal b_r2 (Registers.find "r2" initial_registers).term]
  in
  let links = if normalized then [] else equal b_pc (a_exit "PC") :: links in
  let outputs = List.map (fun (name, output) -> equal output (b_exit name)) selected_registers in
  let relation = scope a (scope b (conjunction (links @ outputs))) in
  let relation = if normalized then relation else quantify [b_r2; b_pc] relation in
  (a, b, relation)

let show_trace label events state =
  List.iteri (fun index event ->
    Printf.printf "%s.%02d " label (index + 1);
    pp_event event;
    print_newline ()) events;
  List.iter (fun (name, _) ->
    let state = Registers.find name state.registers in
    Printf.printf "%s exit %-11s %s version %d from event %d (%s)\n"
      label name state.term.code state.version state.event (smt_sort state.term.sort))
    selected_registers

let entry_read trace name =
  match Registers.find_opt name trace.reads with
  | Some input -> input
  | None -> failwith ("Missing B entry read for " ^ name)

let show_link a b name =
  let out = Registers.find name a.registers in
  require (out.version > 0) ("Missing A exit write for " ^ name);
  let input = entry_read b name in
  let binding = Option.bind input.symbol (fun id -> IntMap.find_opt id b.aliases) in
  require (binding = Some out.term) ("Missing canonical entry binding for " ^ name);
  Printf.printf "A.%02d %s version %d %s -> B.%02d %s entry %s (%s); same-thread sequential transfer\n"
    out.event name out.version out.term.code input.ordinal name input.value.code (smt_sort input.value.sort);
  Printf.printf "Normalized binding: %s := %s\n" input.value.code out.term.code

let declared_ids events =
  List.filter_map (function
    | Smt ((DeclareConst (id, _) | DefineConst (id, _)), _) -> Some id
    | _ -> None) events

let freshen_after events_a events_b =
  let bound_b = declared_ids events_b in
  let largest = List.fold_left max 0 (declared_ids events_a @ bound_b) in
  require (largest < max_int / 2) "Symbol IDs too large for freshening";
  let fresh id = id + largest + 1 in
  (* Targets are outside both namespaces. The upstream substitution handles
     references. Declaration and definition binders need separate updates. *)
  List.map (fun event ->
    let renamed = List.fold_left (fun current id ->
      subst_val_event (Val_Symbolic (fresh id)) id current) event bound_b in
    match renamed with
    | Smt (DeclareConst (id, ty), annotation) ->
        Smt (DeclareConst (fresh id, ty), annotation)
    | Smt (DefineConst (id, value), annotation) ->
        Smt (DefineConst (fresh id, value), annotation)
    | _ -> renamed) events_b

let write_trace path events =
  let output = open_out path in
  Fun.protect ~finally:(fun () -> close_out output) (fun () ->
    let trace = BareTree (TreeTrace (events, End)) in
    PPrint.ToChannel.pretty 0.9 100 output (Isla_lang.PP.pp_whole_tree trace);
    output_char output '\n')

let change_entry_width trace events =
  match (entry_read trace "r2").symbol with
  | Some id when List.mem_assoc id trace.declarations ->
      List.map (function
        | Smt (DeclareConst (name, Ty_BitVec 64), annot) when name = id ->
            Smt (DeclareConst (id, Ty_BitVec 32), annot)
        | event -> event) events
  | _ -> failwith "Expected a declared symbol for the B r2 entry"

let queries file baseline normalized renamed missing wrong_b wrong_a =
  let output = open_out file in
  Fun.protect ~finally:(fun () -> close_out output) (fun () ->
    let write fmt = Printf.fprintf output fmt in
    write "(set-option :timeout 15000)\n";
    write "(declare-datatype Endian ((HostLittleEndian) (HostBigEndian)))\n";
    List.iter (fun value -> write "(declare-const %s %s)\n" value.code (smt_sort value.sort))
      (List.map (fun (_, value) -> value.term) (Registers.bindings initial_registers)
       @ List.map snd selected_registers);
    let definition name body = write "(define-fun %s () Bool\n  %s)\n" name body in
    definition "isa" (conjunction [
      "(= endian_in HostLittleEndian)";
      "(= endian_out endian_in)";
      "(= r1_out r1_in)";
      "(= r2_out (bvadd r1_in (_ bv4 64)))";
      "(= pc_out (bvadd pc_in (_ bv2 128)))";
    ]);
    List.iter (fun (name, relation) -> definition name relation) [
      ("baseline", baseline); ("normalized", normalized); ("renamed", renamed);
      ("missing", missing); ("wrong_b", wrong_b); ("wrong_a", wrong_a);
    ];
    let query ?(model = false) description assertion =
      write "(echo \"%s\")\n(push)\n(assert %s)\n" description assertion;
      write "(check-sat-using (then simplify qe-light smt))\n";
      if model then write "(get-value (r1_in r2_in r2_out pc_in pc_out endian_in))\n";
      write "(pop)\n"
    in
    let implication description left right =
      query description (Printf.sprintf "(and %s (not %s))" left right)
    in
    implication "baseline implies ISA" "baseline" "isa";
    implication "ISA implies baseline (total input coverage)" "isa" "baseline";
    implication "baseline implies normalized" "baseline" "normalized";
    implication "normalized implies baseline" "normalized" "baseline";
    query "renamed input changes the relation" "(xor normalized renamed)";
    List.iter (fun (description, relation) ->
      query ~model:true description
        (Printf.sprintf "(and %s (distinct r2_out (bvadd r1_in (_ bv4 64))))" relation)) [
      ("missing r2 connection allows incorrect output", "missing");
      ("B exit in place of B entry allows incorrect output", "wrong_b");
      ("A entry in place of A exit allows incorrect output", "wrong_a");
    ])

let () =
  match Array.to_list Sys.argv with
  | _ :: trace_a :: trace_b :: output :: fresh_b :: width_output when List.length width_output <= 1 ->
      let events_a = parse trace_a and events_b = parse trace_b in
      let relation mode connection = compose mode connection events_a events_b in
      let a, b, baseline = relation `Baseline Sequential in
      let _, normalized_b, normalized = relation `Normalized Sequential in
      let fresh = freshen_after events_a events_b in
      let _, _, renamed = compose `Normalized Sequential events_a fresh in
      let _, _, missing = relation `Baseline MissingR2 in
      let _, _, wrong_b = relation `Baseline BExit in
      let _, _, wrong_a = relation `Baseline AEntry in
      show_trace "A" events_a a;
      show_trace "B" events_b b;
      List.iter (show_link a normalized_b) ["r2"; "PC"];
      write_trace fresh_b fresh;
      List.iter (fun path -> write_trace path (change_entry_width b events_b)) width_output;
      queries output baseline normalized renamed missing wrong_b wrong_a
  | _ -> failwith "usage: bind A.itl B.itl relations.smt2 B-fresh.itl [B-wrong-width.itl]"
