# Choptick - 终端背单词工具

一个基于 Rust 和 [ratatui](https://github.com/ratatui-org/ratatui) 构建的终端英语词汇学习工具。

左侧显示大型数字时钟，右侧随机展示单词及其释义、例句，每 60 秒自动刷新。
## 截图



## 功能

- 大号 ASCII 艺术数字时钟
- 随机展示英语单词（含音标、中英文释义、例句）
- **熟词生义**功能：展示单词生僻用法（调用 Free Dictionary API）
- 每 60 秒自动切换单词
- 按 `r` 手动刷新，按 `q` / `Esc` / `Ctrl+C` 退出

## 安装

```bash
git clone https://github.com/yangstafiltra/choptick.git
cd choptick
cargo build --release
./target/release/choptick
```

## 依赖

- Rust 2021 edition
- 需要 `curl` 命令（用于调用 API）

## 数据来源

- 单词列表及基本释义：[baicizhan-word-meaning-API](https://github.com/lyc8503/baicizhan-word-meaning-API)
- 扩展释义：[Free Dictionary API](https://dictionaryapi.dev/)

## 操作

| 按键 | 功能     |
| ---- | -------- |
| `q`  | 退出     |
| `r`  | 刷新单词 |

---

*本项目由 AI 辅助编程完成。*
