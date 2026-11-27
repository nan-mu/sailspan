# PR1 · eBPF Node Generation

代码仓库：`nan-mu/sailspan`，在实际 PR1 的现有 head 上修订；如果仍是预留编号，按 `task-brief-to-pr` 核对映射及远端基线。作者的 GitHub connector 查询返回 404，未审查到实际 PR 代码，执行者先核实现有实现，不另开重复 PR。上一版任务书见 [Git 历史](https://gitea.nas.nan-mu.cc/mcp/sailspan/src/commit/9da3914ee930585b35ba36c954d2d3f84c007cce/taskbook.md)。

共享规范（固定 main `18ee2a9a02b9b5d5a877f5da21d4560f007fc9bb`）：[项目决策](https://gitea.nas.nan-mu.cc/mcp/sailspan/src/commit/18ee2a9a02b9b5d5a877f5da21d4560f007fc9bb/decisions.md)（D001、D002、D004、D005、D006）、[术语](https://gitea.nas.nan-mu.cc/mcp/sailspan/src/commit/18ee2a9a02b9b5d5a877f5da21d4560f007fc9bb/GLOSSARY.md)。本轮替换旧版在 Node/Boundary 中内嵌原始指令的文件格式；原有输入与节点划分目标继续有效。不存在继续支持旧 v1 文件的要求。

## 目标

### G1 · 指令表与节点结构

用 `implement` + `tdd` 修改现有 Rust 内部类型与 Serde 持久化入口，实现 [产物契约](references/node-artifact.md) 中的 InstructionTable 和 NodeArtifact。先从原始对象构造完整的编号反汇编，再以该表进行节点划分；正常输出保存二者，且能从文件反序列化恢复内部结构。编号引用必须可追溯至对应原始对象，不再产生旧版重复存储指令的结构。

验收：用同一入口生成、读回两份逻辑产物；核查编号稳定、文件关联正确、引用可解析及覆盖完整，参照契约中的人工示例测试双槽指令和跳转目标拆分。以实际内部类型、文件和读回结果为证据，不以打印文本或旧格式转存冒充完成。

### G2 · 发布输入与扩展接口

用 `implement` 接通 [固定发布输入](references/ebpf-object.md)，从已下载的 firewall 压缩包选择指定 eBPF 对象，校验发布摘要后交给对象解析、反汇编和节点生成。来源选择与解析算法独立；未来扩展版本、程序和来源时复用相同的内部类型与节点生成逻辑。无需实现下载客户端、全量批处理或其他架构。不得用运行镜像或重新编译对象替代发布对象。

### G3 · firewall 端到端实例

通过正式 CLI/库入口跑通 G2 → G1，保存实际产物及复现命令。以 `llvm-readelf -h -S -s -r` 与 `llvm-objdump -dr` 或独立等效工具核对原始对象的执行段、指令、位置、重定位和切分点；验证全部合法程序指令的归属，并用 [产物契约](references/node-artifact.md) 的不变量判定正确性。重复运行时逻辑内容和编号稳定。

本轮只验收这一个真实发布程序与必要小样例。其他发布程序、完整 ISA 矩阵、运行时容器、符号推演及跨节点分析留待后续任务；不以未实现这些能力阻塞本轮。无新增 UI、GitHub CI 或无关基础设施。

## 独立模型验证要求

主代理将固定任务书、代码 PR head、原始发布对象与摘要、CLI 证据、两份实际文件和独立反汇编交给未参与实现的 Reviewer/Tester，使用 `code-review`：

- G1：按 [产物契约](references/node-artifact.md) 追踪真实 Rust 类型 → 文件 → 类型；用人工反例和实物检查指令引用。若编号与槽混淆、悬空引用、重复归属、无法定位原始指令，或节点仍内嵌旧指令结构，则拒绝。
- G2：按 [发布证据](references/ebpf-object.md) 核查真实提取链及摘要；检查来源选择没有写入通用节点算法。
- G3：用独立反汇编重建覆盖与切分关系；不能只核对输出数量。若目标槽在旧节点内部、控制流指令被吞入普通节点、重定位被丢弃，均不通过。

分别记录通过/失败/阻塞/不适用、实际模型角色、受审 head、代码位置、输入输出与独立依据。缺证据不得标完整通过。依赖管理按共享 D004 核对；不要求未来消费者已接通。

## 交付

在原 PR1 交付修订（实际编号核对），用 `pr` 在英文 PR 正文记录类型、文件、运行命令、产物摘要、独立验证结论及未解决项。使用独立 worktree，遵守现有审计流程，保持开放待人工审核；未授予合并权限。
