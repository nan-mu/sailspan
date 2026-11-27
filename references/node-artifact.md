# PR1 · 指令表与节点产物契约

旧 v1 内嵌 raw_hex 的结构由本契约替代。建议使用 schema_version=2，以区分旧格式。名称、具体 Rust 布局及 Serde 格式可按实现选择；下面定义的是必须保持的语义和引用关系，而不是要求逐字实现示意字段。

## 内部数据模型与文件映射

```text
ReleaseInput -> original eBPF Object
    -> disassemble -> InstructionTable
                         |
                         v
                  generate_nodes -> NodeArtifact
                         |
    InstructionTable + NodeArtifact -> Serde -> persisted files
```

InstructionTable 和 NodeArtifact 是独立的 Rust 类型；建议分别映射为 firewall.disassembly.json 与 firewall.nodes.json。可以采用其他可读回的 Serde 格式，或一个文件中的两个独立逻辑区，只要保持以下关系。原始 ELF 仍是机器码的唯一依据，不要求把机器字节写进两个产物。

### InstructionTable（唯一指令表）

- 根元数据：schema_version、architecture、endianness、源对象的 Release tag / asset / member 与 SHA-256、执行 section 及入口信息。来源字段记录实际值，人工输入显式标 synthetic。
- 一条逻辑指令一条记录：全对象唯一且稳定的 `id`（建议从 0 连续编号）、ELF `section_index`、section 内 8 字节 `slot`、`slot_count`、可读反汇编 `assembly` 以及实际 `relocations`。稳定顺序按 section index 与 slot 排列；id 不是 slot，LDDW 两槽共用一个 id。不同 section 的相同 slot 不冲突。
- section 与 slot、slot_count 足以从具有相同摘要的原始对象取回精确机器字节；不要反汇编后靠重新汇编来恢复机器码。反汇编文本保留实际操作数、立即数和必要符号引用；不能只存 opcode 名称。
- 重定位只在指令表保存一次：原 section 内字节偏移、类型、符号身份（符号索引、名称）、目标 section（可得时）以及显式/可提取隐式 addend 与来源。不能确定的值显式标未知；不把占位地址写成运行时地址。

### NodeArtifact（只引用编号）

- 根元数据：版本和唯一指令表关联（分文件时保存相应指令表文件摘要或等价强一致身份；单文件可用明确的根结构关联）。不重复保存源对象元数据。
- Node：稳定唯一 id、有序 `instruction_ids`。Boundary：id、kind、有序 `instruction_ids`、静态目标/顺序后继及未解析原因（目标优先引用指令 id，外部或未解析目标可保留符号引用或明确原因）。两种记录不内嵌指令、反汇编字符串、原始字节或重定位副本。
- 一个逻辑指令只能归属于一个 Node 或 Boundary；候选目标引用不是归属。每组内部编号对应同一 section 中实际连续指令。普通 Node 不含程序级控制流；条件/无条件跳转、Helper 调用、其他调用和退出单列 Boundary，当前每个普通 eBPF 边界通常引用一个 id，不制造空节点。记录不支持的调用类别和未解析信息。
- 收集入口、边界和所有可静态确定的跳转目标，目标落入普通序列内部则拆分。eBPF 跳转使用槽号相对下一槽计算（JMP32 JA 使用 imm）；计算后查表映射到逻辑 id，不对 id 做偏移运算。LDDW 第二槽、越界等非法目标须报告，不静默舍入。保留合法但不可达的指令，不进行条件求值、符号执行或全局路径展开。

## 最小示例（人工构造，不是 firewall 实测输出）

以下省略人工源元数据和空重定位列表，用于解释编号关系，不是独立固定测试的逐字段快照。

```json
{
  "instructions": [
    {"id":0,"section_index":1,"slot":0,"slot_count":1,"assembly":"r2 = r1"},
    {"id":1,"section_index":1,"slot":1,"slot_count":1,"assembly":"r2 += 4"},
    {"id":2,"section_index":1,"slot":2,"slot_count":1,"assembly":"if r2 > r3 goto +2"},
    {"id":3,"section_index":1,"slot":3,"slot_count":2,"assembly":"r1 = 4294967296 ll"},
    {"id":4,"section_index":1,"slot":5,"slot_count":1,"assembly":"r0 = 2"},
    {"id":5,"section_index":1,"slot":6,"slot_count":1,"assembly":"call 1"},
    {"id":6,"section_index":1,"slot":7,"slot_count":1,"assembly":"exit"}
  ]
}
```

上面 id=3 是 LDDW，占槽 3、4，但只有一条编号记录。

```json
{
  "nodes": [
    {"id":"n0","instruction_ids":[0,1]},
    {"id":"n1","instruction_ids":[3]},
    {"id":"n2","instruction_ids":[4]}
  ],
  "boundaries": [
    {"id":"b0","kind":"conditional_jump","instruction_ids":[2],"targets":[4],"fallthrough":3},
    {"id":"b1","kind":"helper","instruction_ids":[5],"fallthrough":6},
    {"id":"b2","kind":"exit","instruction_ids":[6]}
  ]
}
```

条件跳转 slot=2、offset=+2：目标 slot=5，对应 id=4，顺序后继 id=3。若误把偏移用于 id，将得到 id=5，验收必须失败。将偏移改成 +1，目标落到 LDDW 的第二槽，必须诊断。这里 id=4 即使可由 id=3 顺序到达仍需成为新节点起点。

## 读回不变量

真实文件均能由 Rust 类型反序列化；版本、对象来源与指令表关联相符；指令编号唯一、槽范围有效；各组有序引用完整、非空、不悬空且无重复归属；普通节点、边界及静态目标符合上述定义。篡改指令表导致关联不一致、插入不存在的 id 或制造重复归属时，应拒绝或明确报告不合法，不能默认为成功。以真实对象和独立反汇编核查表，而非用节点文件自己证明指令表正确。
