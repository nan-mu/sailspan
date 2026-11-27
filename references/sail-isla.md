# PR2 · 上游组件与复用入口

## 依赖清单及职责

| 上游 | 本轮用途 | 接入方式 |
|---|---|---|
| [Sail eBPF](https://github.com/OpenSourceVerif/sail-ebpf) | 权威候选 ISA 模型：解码及所选普通指令语义 | 固定 commit 的源码依赖，优先 submodule |
| [Isla](https://github.com/rems-project/isla) | 独立指令的实际符号推演和 Trace 生成 | 固定 commit 的 submodule |
| [isla-lang](https://github.com/rems-project/isla-lang) | ITL 语法、OCaml AST、parser/pretty-printer 及 Coq 相关生成定义 | 固定上游版本；若作为源码使用则 submodule，或复用 Islaris 的精确 opam pin，不安装第二套版本 |
| [Islaris](https://github.com/rems-project/islaris) | ITL 操作语义、逻辑规则、转换入口、证明自动化 | 固定 commit 的 submodule |
| [Iris](https://gitlab.mpi-sws.org/iris/iris)、Lithium、stdpp-bitvector | Islaris 的逻辑与位向量证明基础 | 沿用 Islaris 的兼容 opam 依赖，不重写、不另外复制源码 |
| Sail 编译器、OCaml/opam、Dune、Ott、Coq/Rocq、Z3 | 生成 IR、解析/生成定义、运行证明与约束检查 | 按上游兼容组合锁定工具版本 |
| [isla-snapshots](https://github.com/rems-project/isla-snapshots) | 官方 Islaris/Isla 基线例子所需已有 ISA 快照（仅在实际需要时） | 固定版本；不作为 eBPF 模型替代 |

全套管理遵循 [D004](https://gitea.nas.nan-mu.cc/mcp/sailspan/src/commit/18ee2a9a02b9b5d5a877f5da21d4560f007fc9bb/decisions.md)。建议将独立源码依赖置于 third_party/，路径由实施者决定。只引入实际调用的项目与依赖；常规工具不强制 submodule。对于同一依赖优先共用一个版本和 checkout，避免 opam 安装另一份未固定源码。若确需不同版本，说明原因及接口兼容性。

## Sail eBPF

论文：Formal Specification of Linux eBPF Instruction Set Architecture in Sail（eBPF 2026）。[作者模型仓库](https://github.com/OpenSourceVerif/sail-ebpf)、[项目发表记录](https://fiction-zju.github.io/publication/)、[Artifact 分支候选](https://github.com/OpenSourceVerif/sail-ebpf/tree/ebpf26-artifact)。

采用模型中的实际解码与 ISA 定义，不用独立解释器冒充。论文区分顺序指令与原子指令的覆盖；其 Rocq/CompCert 内存适配不意味着 Isla backend 已兼容。优先查 `model/` 的真实源码及 artifact 分支能否取得、版本、许可证和所需编译器。只解决本例必须的接口，不实现完整内存后端。适配不得改变原有 ISA 语义。

## Isla

论文：Isla: Integrating Full-Scale ISA Semantics and Axiomatic Concurrency Models（CAV 2021）。[README](https://github.com/rems-project/isla)、[manual](https://github.com/rems-project/isla/blob/master/doc/manual.adoc)、[isla-sail](https://github.com/rems-project/isla/tree/master/isla-sail)。

Isla 生成 Sail IR 的符号执行结果。先核对 eBPF 64-bit 指令编码/字节序、模型入口、初始寄存器和 PC 的表示。若直接 CLI 的架构接口不满足，复用同一库/API 并做最小适配。Islaris README 的 `bin/isla-footprint` wrapper 和 frontend 调用是生成可消费 Trace 的直接参考；不要预先假定 CLI flags 及格式跨版本兼容。保留原始未简化输出或等价的完整事件，简化结果只作为派生产物。这里需要 ITL，不是 Sail .ir 文件，也不是只有 SMT-LIB 的公式。

[Isla 扩展版论文 §3.2 Per-thread traces](https://doi.org/10.1007/s10703-023-00409-y) 的命名隔离与连接约束可作为参考；其线程/并发构造不必引入本例。优先检查实际版本的 Trace AST、重命名、表达式依赖和 SMT 导出，避免自写文本解析及字符串替换。

## isla-lang

[README](https://github.com/rems-project/isla-lang#isla-lang)、[isla_lang.ott](https://github.com/rems-project/isla-lang/blob/master/isla_lang.ott)、[traces.v](https://github.com/rems-project/isla-lang/blob/master/traces.v)、[main.ml](https://github.com/rems-project/isla-lang/blob/master/main.ml)。

上游通过 Ott 的语法定义生成 OCaml 类型、parser 和 pretty-printer。先复用实际 parser 和 AST 读取 G1 输出，按所选版本的 Coq 生成定义与 Islaris 的实际转换入口导入证明环境。可使用 dune 的现有命令行 example 或小型 OCaml wrapper；Rust 侧只实现项目需要的状态关联/输出，不复制整个 grammar 或以正则提取符号作为正式表示。若 Islaris 使用附带或生成的 Coq 类型，复用同一版生成链，避免并行维护两个语法。

## Islaris 与证明系统

Sammler et al., *Islaris: Verification of Machine Code Against Authoritative ISA Semantics*, PLDI 2022, [DOI 10.1145/3519939.3523434](https://doi.org/10.1145/3519939.3523434), [作者公开全文](https://people.mpi-sws.org/~dg/papers/pldi2022.pdf)。

重点阅读：

- §2.3、Figure 4–5：ITL 的语法及逻辑规则。declare/define 引入值和进行表达式替换；read-reg 约束读到的值与当前寄存器一致，write-reg 改变寄存器状态。Assume/AssumeReg 是 Isla 已采用的前提，须由证明建立，不能作为任意丢弃行为的手段。Islaris 在此采用 Hoare double（以 continuation 解释结果），并非直接为任意两个 Trace 给出关系等价算法。
- §4.2、§4.3：ITL 的操作语义及安全性/程序逻辑；需要区别不可行选择、失败状态与真实正常执行。§4 后部的自动化以状态上下文查找寄存器，适合复用作为关联的逻辑依据。
- §5：与 Sail 生成的 RISC-V Coq 模型做 Translation Validation 的方法和适用边界。不能将已有 RISC-V 论证外推为 eBPF 的机器码→ITL 已验证。

实际源码入口（以最终固定版本为准）：

- [theories/](https://github.com/rems-project/islaris/tree/main/theories)：先定位 ITL 操作语义、lifting 与 adequacy 等定义、相关构造子和证明。
- [theories/automation.v](https://github.com/rems-project/islaris/blob/main/theories/automation.v)：已核对 `li_wp_read_reg`、`li_wp_write_reg`、`li_wp_declare_const_bv`、`li_wp_define_const` 及 `li_wp_assume_reg` 的使用；`FindRegMapsTo` 与字段访问版本用于查找状态。优先直接调用相关引理/automation；不得照抄一套脱离原模型的替代证明。
- [frontend/](https://github.com/rems-project/islaris/tree/main/frontend)、[examples/](https://github.com/rems-project/islaris/tree/main/examples)：官方 README 描述从 dump 为每个地址生成 Coq Trace 定义以及地址→Trace 映射；复用实际转换和最小 proof example。
- [frontend/arch.ml](https://github.com/rems-project/islaris/blob/main/frontend/arch.ml)：本轮只读查到 `supported_archs` 只有 aarch64、riscv64。eBPF 不是开箱即用，验证 ISA 配置、寄存器结构/访问器、指令编码、PC 及 Trace 格式；可绕过架构专用生成入口直接消费真实 Trace 并复用通用理论。任何过滤事件或忽略寄存器的行为须有可验证的依据。
- [islaris.opam](https://github.com/rems-project/islaris/blob/main/islaris.opam) 与 [README 的 Setup](https://github.com/rems-project/islaris#setup)：兼容环境依据。此次可见 opam 文件包含 Coq 8.19.0、Dune 3.9.1、指定 Lithium 版本与 isla-lang pin `bda86c9f0bd28bbaa2481f50ddc986ede342805a`；README 给出已测试 Isla `b8e614bda4a20e42c37d8b216853653133d04322`、isla-snapshots `b58da9170470a422c9396983ac8f87f0a63ba6f8`。这些是上游兼容性线索，非已验证的 eBPF 组合；须按实际选定 Islaris commit 再核对，记录差异及原因。不盲目混用最新版本。

## 复现及边界

先按上游方法建立最小可运行的环境，再接入 eBPF 模型。记录每个实际依赖的 URL、commit/锁定版本、许可、构建入口及其在原型/证明中的调用位置；必要修改用独立 wrapper、可复用小补丁或经授权且可取得的 fork commit。用干净 checkout 初始化 submodule 并验证实际使用的版本，生成 IR/构建输出可重建，不提交整个上游树或依赖 dirty submodule。

作者核对了上游公开论文、README、opam 和部分源码，未实际安装/运行 eBPF、isla-lang 或 Islaris；亦未核实所选 eBPF Artifact 与固定依赖组合的运行兼容性。以上是待实施和待验收的工程调查，不是预验证通过的结论。
