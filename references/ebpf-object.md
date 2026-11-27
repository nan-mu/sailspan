# eBPF 对象与节点生成的规范参照

## 本轮固定 Release 输入

2026-11-28 通过 [GitHub Releases API](https://api.github.com/repos/nan-mu/libbpf-mcu/releases) 只读核对：当前最新可见候选为 [v0.1.2-rc.1](https://github.com/nan-mu/libbpf-mcu/releases/tag/v0.1.2-rc.1)，prerelease=true；精确标签 v0.1.2 查询返回 404。Release 正文列出 build commit `cc824e88e455b25aaeb03eafc6a8f68b42947930`。

| 输入 | 路径 | 字节数 | SHA-256 |
|---|---|---:|---|
| 发布压缩包 | [firewall.7z](https://github.com/nan-mu/libbpf-mcu/releases/download/v0.1.2-rc.1/firewall.7z) | 9684 | `88bf2b0345bd14e032fa4a09748bbb0110fc2fd81597a8f76a27db4543d5a37c` |
| 发布清单 | [release.json](https://github.com/nan-mu/libbpf-mcu/releases/download/v0.1.2-rc.1/release.json) | 51572 | `1d765524d3f9ab5331eb903134222a3dc20b6b8ac611ff87d7e9460bed0968fa` |
| 实际分析成员 | `objects/firewall_bpf.o` | 8096 | `d37ae51e9cf13b9d2ae7f893877dc3b7ada17e03d824a241cc90dba892dedce4` |

证据：Release API 的 asset metadata 提供 archive/ledger 摘要及大小；发布者 [候选镜像 index.json](https://github.com/nan-mu/libbpf-mcu/blob/9ba2c62383ac26b1a0aaba796e6066869d6a0e5d/.audit/taskbook/pr7-candidate/index.json) 的对应两项一致。作者读取该固定提交的 `release.json.001.b64` 和 `release.json.002.b64`，按顺序拼接并 base64 解码为 JSON，取 archives 中 program=firewall 的 members，得到上表对象路径、大小、摘要；同一清单的 modes.ebpf 将 images/ebpf.bin 标为 rbpf-map-index-le 解释器镜像，不能替代原始对象。清单还列出 images/jit.bin、images/llvm-aot.bin 及 objects/firewall_rv32.o，均不分析。

作者没有实际下载或解压 Release 二进制，也未独立计算其字节摘要；执行端从正式发布资产取得文件，核对上述摘要并提取唯一指定对象。若直接下载受阻，可按 [发布者固定提交的候选镜像复原说明](https://github.com/nan-mu/libbpf-mcu/blob/9ba2c62383ac26b1a0aaba796e6066869d6a0e5d/.audit/taskbook/pr7-candidate/README.md) 复原同一资产字节，并核对与正式资产的摘要一致；不得重新编译替代。这里只需 firewall.7z 和必要清单，不需要其他程序压缩包或 evidence.7z。本轮没有已运行的分析测试可复用。

## 可直接复用的定义

[Linux eBPF ISA](https://docs.kernel.org/bpf/standardization/instruction-set.html) 的 instruction encoding、jump instructions、load/store 章节，以及 [RFC 9669](https://www.rfc-editor.org/rfc/rfc9669.html) 是机器码字段和指令分类的独立依据。8 字节基本槽；宽立即数占两槽；跳转偏移相对下一槽，JMP32 JA 使用 imm。保留字节序和双槽整体；区分 CALL 的类型与 EXIT。静态目标只用于划分与标记，不推导可达性。节点划分算法是 Sailspan 的设计，不是上述规范提供的证明。

[Linux LLVM BPF relocation 文档](https://docs.kernel.org/bpf/llvm_reloc.html) 给出 ELF relocation 结构和 R_BPF_64_64、R_BPF_64_32 的位置、符号及 addend 处理。保留对象原始字段，不能把 map 或调用占位值解释为运行时地址。符号与 section 的关联必须可复查。

## 工具

ELF 读取可复用 Rust object crate 或等效成熟库；按实际需要选择并锁定版本。LLVM readelf/objdump 用于独立校验，不是节点生成的唯一语义来源；记录实际工具版本。7z 或等效读取库用于提取固定成员，检查摘要。第三方依赖管理见[项目决策 D004](https://gitea.nas.nan-mu.cc/mcp/sailspan/src/commit/18ee2a9a02b9b5d5a877f5da21d4560f007fc9bb/decisions.md)。
