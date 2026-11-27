# PR2 · Islaris-based Symbol Binding Example

代码仓库：`nan-mu/sailspan`。已有 PR2 时继续其 head；尚未创建时按 `task-brief-to-pr` 核对预留编号及远端基线。作者本轮 GitHub PR2 查询仍返回 404，执行前确认实际状态，不重复创建已有 PR。上一版：[固定提交](https://gitea.nas.nan-mu.cc/mcp/sailspan/src/commit/381e25216ee0263fe9614baa0661059a9a1961de/taskbook.md)。共享规范：[项目决策](https://gitea.nas.nan-mu.cc/mcp/sailspan/src/commit/18ee2a9a02b9b5d5a877f5da21d4560f007fc9bb/decisions.md)（固定 main `18ee2a9a02b9b5d5a877f5da21d4560f007fc9bb`，D002、D004）、[术语](https://gitea.nas.nan-mu.cc/mcp/sailspan/src/commit/18ee2a9a02b9b5d5a877f5da21d4560f007fc9bb/GLOSSARY.md)。

## 目标

使用真实 Isla 的两次独立 eBPF 指令推演，复用 ITL 的现有语法和 Islaris 的形式化状态规则，建立一个可执行、可审计、可验证的符号关联原型。实验及正确性判据的唯一规格见 [symbol-binding.md](references/symbol-binding.md)；上游项目、复用入口、版本和限制见 [sail-isla.md](references/sail-isla.md)。本轮仍仅使用原有 MOV64、ADD64 两条指令，不要求完整程序分析。

### G1 · 获取两个独立的真实 Trace

用 `research` + `implement` 按 Reference 固定 Sail eBPF、Isla 与兼容工具链，分别对 A、B 进行独立推演。以实际模型/引擎生成的完整 Trace 为依据，保留原始输出、模型与工具版本、配置、字节输入及命令；不得向 B 预注入 A 的状态。优先复用 Islaris 所需的 Isla trace 输出模式或同一引擎 API。单次连续执行仅可作为额外对照。

### G2 · 复用 ITL 的解析器和形式化数据表示

用 `implement` 接入上游 isla-lang 及 Islaris 的 trace 转换能力，将 G1 两份原始 Trace 变成现成结构化表示，能够读取符号声明、表达式、类型、寄存器访问器、读写事件和假设。尽量沿用 isla-lang 的 OCaml parser/AST 与 Islaris 对应的 Coq trace 类型和转换入口；允许独立 wrapper 通过文件/进程边界和 Rust 代码交换数据，不以重写成纯 Rust 为本轮目标，不自行发明一套替代 ITL 的通用语言。

先验证上游依赖兼容及原有最小示例可运行；识别 eBPF 尚需的适配点。若原 frontend 的架构表或指令反汇编宽度阻塞 eBPF，优先将真实 G1 Trace 直接送入通用 parser/converter 或做最小适配，而不是重写整个 frontend。保留解析、转换的实际输入输出及失败信息；核对转换没有丢失对证明必要的语义事件。

### G3 · 从 Islaris 状态规则导出并实现符号关联

用 `research` + `implement` 阅读 [Islaris 复用入口](references/sail-isla.md#islaris-与证明系统)，依据 [关联方法](references/symbol-binding.md#关联方法) 实现小型可运行原型。输入是 G2 的两个独立 Trace 与明确的顺序前提，输出原始符号到状态位置/版本的对应、关联依据及规范化组合表示。优先复用现有 AST、表达式操作、变量作用域和逻辑规则；复用 read/write/declare/define/assume 的语义，不以临时符号名、相同寄存器字符串或预先写好的答案作为等值依据。

实际给出 canonical identity 或保留的等式/表达式连接，在同一组合表示中体现关联；仅完成 Islaris 证明脚本而未输出可检查的符号关联结果不算完成 G3。具体宿主语言、最小内部类型和输出 codec 留给实施者；不建立通用 NodeSummary。

### G4 · 使用 Islaris / Coq（Rocq）验证实例

用 `tdd` 与 `implement` 复用 Islaris 的现成 ITL 操作语义、Iris 逻辑规则及 proof automation，建立 [形式化验证与正确性判据](references/symbol-binding.md#正确性判据) 的可运行验证。必须让证明消费由 G1/G2 真实生成的 Trace，分别证明/检查两个独立指令的状态效果及连接后的结果；再验证 G3 规范化组合对选定外部接口的语义保持。可用 Islaris 的 Hoare double/weakest precondition 表达顺序状态传递，Z3 或其他可检查的证明负责补充双向关系、输入域和反例检查。不得用仅验证手写等式的 Z3 脚本代替实际 Islaris 证明。

保留 theorem/lemma 源码、可复现构建入口、原始检查结果及适用前提，不允许用新的 Admitted、admit、无证明的 Axiom 或公理化目标掩盖缺口。报告实际信任边界，包括上游逻辑的既有假设。目标是此实例的 ITL 层可检查证明，不是整个 eBPF ISA 或通用符号绑定算法已被证明。无法完整执行时按对应目标标阻塞，保留可复现实验和明确技术原因；其他 ISA 的基线成功不能代替 eBPF 成功。

## 范围

PR2 仍为独立 example，可并行于 PR1，不读取其 Release、节点产物，不修改 PR1 的公共构建或产物契约。除最小适配外，不扩展至全量 ISA、内存/Helper 语义、边界执行、循环、全局路径或跨节点消费者。待 PR1 人工审核合入后，再安排正式节点推演扩展。外部依赖遵守共享 D004；不把上游整树复制进 Sailspan。对确有价值的其他上游组件优先复用，并记录理由、实际调用位置及兼容性；无需为未用到的组件增加依赖。

## 独立模型验证要求

主代理将固定任务书、依赖清单、最终代码 head、原始 Trace、结构化转换结果、关联记录、规范化产物、证明源码与执行日志交给未参与实现的 Reviewer/Tester（`code-review`）：

- G1：核对确为两次真实 eBPF 推演，固定输入与实际模型一致。
- G2：追踪真实 Trace 到上游 parser/AST/Coq 对象的完整链路，核对必要事件、符号作用域和假设无遗漏。仅将文件命名为 ITL 或将依赖放进 submodule 而未实际调用不通过。
- G3：按关联 Reference 从真实输入独立重建状态传递；以名称碰撞、错误版本与缺失连接反例检查原型；核对统一表示与证据一致。
- G4：在固定版本环境复现 proof checking 和关系判据，检查真实 Trace 与 theorem 的绑定、实际前提、未消解假设、覆盖和规范化双向关系。仅证明安全性或仅得到 UNSAT 不足以声称关系等价。无证明的占位符、矛盾前提、手写替代 Trace 或只验证单个常量输入均不能通过。

逐项报告通过/失败/阻塞/不适用，注明模型角色、版本、代码位置及证据。没有独立验证或真实运行证据不得标为完整通过。

## 交付

用 `pr` 在英文 PR 正文链接四项交付证据、固定依赖及复现说明，特别列明复用了哪些上游文件/模块/定理，哪些是自身最小适配，哪些结论已由证明检查支持及尚未覆盖的边界。保持开放 PR 供用户审核；未授权自动合并。不向任务书仓库提交实施代码。
