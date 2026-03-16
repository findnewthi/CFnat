# CFnat

Cloudflare IP 延迟优选 + 负载均衡转发工具

# 免责声明
- 项目以学习和研究为目的而设计和开发。
- 使用本项目代码或程序时，必须严格遵守所在地区的法律法规。
- 对使用本软件由此产生的任何后果完全自负。

# 演示图

[演示图](https://raw.githubusercontent.com/GuangYu-yu/CFnat/refs/heads/main/png/前端.png)

## 参数说明

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `-addr` | 本地监听地址 | `127.6.6.6:6` |
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
# 基本使用
./CFnat -f ip.txt

# 指定数据中心
./CFnat -f ip.txt -colo HKG,LAX,SJC

# 自定义参数
./CFnat -f ip.txt -ips 20 -dl 500 -n 256 -addr 127.0.0.1:1234
```

## License

GNU Affero General Public License v3.0
