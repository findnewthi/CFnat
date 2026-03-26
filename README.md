# CFnat

Cloudflare IP 延迟优选 + 负载均衡转发工具

![Rust Version](https://img.shields.io/badge/rustc-latest-orange?style=flat-square&logo=rust)
[![zread](https://img.shields.io/badge/Ask_Zread-_.svg?style=flat&color=00b0aa&labelColor=000000&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB3aWR0aD0iMTYiIGhlaWdodD0iMTYiIHZpZXdCb3g9IjAgMCAxNiAxNiIgZmlsbD0ibm9uZSIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj4KPHBhdGggZD0iTTQuOTYxNTYgMS42MDAxSDIuMjQxNTZDMS44ODgxIDEuNjAwMSAxLjYwMTU2IDEuODg2NjQgMS42MDE1NiAyLjI0MDFWNC45NjAxQzEuNjAxNTYgNS4zMTM1NiAxLjg4ODEgNS42MDAxIDIuMjQxNTYgNS42MDAxSDQuOTYxNTZDNS4zMTUwMiA1LjYwMDEgNS42MDE1NiA1LjMxMzU2IDUuNjAxNTYgNC45NjAxVjIuMjQwMUM1LjYwMTU2IDEuODg2NjQgNS4zMTUwMiAxLjYwMDEgNC45NjE1NiAxLjYwMDFaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00Ljk2MTU2IDEwLjM5OTlIMi4yNDE1NkMxLjg4ODEgMTAuMzk5OSAxLjYwMTU2IDEwLjY4NjQgMS42MDE1NiAxMS4wMzk5VjEzLjc1OTlDMS42MDE1NiAxNC4xMTM0IDEuODg4MSAxNC4zOTk5IDIuMjQxNTYgMTQuMzk5OUg0Ljk2MTU2QzUuMzE1MDIgMTQuMzk5OSA1LjYwMTU2IDE0LjExMzQgNS42MDE1NiAxMy43NTk5VjExLjAzOTlDNS42MDE1NiAxMC42ODY0IDUuMzE1MDIgMTAuMzk5OSA0Ljk2MTU2IDEwLjM5OTlaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik0xMy43NTg0IDEuNjAwMUgxMS4wMzg0QzEwLjY4NSAxLjYwMDEgMTAuMzk4NCAxLjg4NjY0IDEwLjM5ODQgMi4yNDAxVjQuOTYwMUMxMC4zOTg0IDUuMzEzNTYgMTAuNjg1IDUuNjAwMSAxMS4wMzg0IDUuNjAwMUgxMy43NTg0QzE0LjExMTkgNS42MDAxIDE0LjM5ODQgNS4zMTM1NiAxNC4zOTg0IDQuOTYwMVYyLjI0MDFDMTQuMzk4NCAxLjg4NjY0IDE0LjExMTkgMS42MDAxIDEzLjc1ODQgMS42MDAxWiIgZmlsbD0iI2ZmZiIvPgo8cGF0aCBkPSJNNCAxMkwxMiA0TDQgMTJaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00IDEyTDEyIDQiIHN0cm9rZT0iI2ZmZiIgc3Ryb2tlLXdpZHRoPSIxLjUiIHN0cm9rZS1saW5lY2FwPSJyb3VuZCIvPgo8L3N2Zz4K&logoColor=ffffff)](https://zread.ai/GuangYu-yu/CFnat)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/GuangYu-yu/CFnat)

# 免责声明
- 项目以学习和研究为目的而设计和开发。
- 使用本项目代码或程序时，必须严格遵守所在地区的法律法规。
- 对使用本软件由此产生的任何后果完全自负。

# 演示图

<img alt="演示图" src="https://raw.githubusercontent.com/GuangYu-yu/CFnat/refs/heads/main/png/终端.png" />
<img alt="演示图" src="https://raw.githubusercontent.com/GuangYu-yu/CFnat/refs/heads/main/png/前端.png" />

## 参数说明

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `-addr` | 本地监听地址 | `127.6.6.6:1234` |
| `-colo` | 数据中心过滤，多个用逗号分隔 | 无 |
| `-dl` | 延迟上限（毫秒） | `500` |
| `-tlr` | 丢包率上限 | `0.1` |
| `-http` | 测速地址 | `http://cp.cloudflare.com/cdn-cgi/trace` |
| `-ips` | 目标负载 IP 数量 | `10` |
| `-n` | 测速并发数 | `16` |
| `-tp` | TLS 流量转发端口 | `443` |
| `-p` | HTTP 流量转发端口 | `80` |
| `-f` | IP 文件路径 | `ip.txt` |

## 示例

```bash

curl -OL https://github.com/GuangYu-yu/CFnat/raw/refs/heads/main/binaries/Linux_ARM64/CFnat && chmod +x CFnat

# 基本使用
./CFnat -f ip.txt

# 指定数据中心
./CFnat -f ip.txt -colo HKG,LAX,SJC

# 自定义参数
./CFnat -f ip.txt -ips 20 -dl 500 -addr 127.0.0.1:1234
```
## License

GNU Affero General Public License v3.0
