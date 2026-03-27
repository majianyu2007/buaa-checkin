# BUAA 智慧教室 · 自动签到系统

All-in-One 自动签到管理系统，支持多用户托管签到。

## ✨ 功能

- **学号登录**：使用北航学号直接登录（iclass 无密码认证）
- **今日课表**：查看当天课程安排与签到状态
- **手动签到**：一键签到尚未打卡的课程
- **自动签到**：为注册用户自动在课程开始前 0-10 分钟随机时间执行签到
- **多用户管理**：支持添加/删除多个用户并配置各自的自动签到课程
- **签到通知**：签到成功后通过 [Server酱](https://sct.ftqq.com) 或自定义 Webhook 推送通知（支持前端手动测试）
- **单文件运行**：前端静态资源已嵌入二进制文件，支持单文件分发与运行
- **Windows 服务**：原生支持安装为 Windows 后台服务，实现开机自启无感运行
- **防控策略**：随机 UA 池、随机签到时间、会话过期自动重登、失败重试

## 🚀 部署

### Docker Compose（推荐）

```bash
# 1. 生成 JWT_SECRET
./gen_secret.sh

# 2. 设置到 docker-compose.yml 的 JWT_SECRET 环境变量
vim docker-compose.yml

# 3. 启动
docker compose up -d

# 3. 访问
open http://localhost
```

### Docker 手动运行

```bash
docker build -t buaa-checkin .
docker run -d \
  -p 80:3000 \
  -v $(pwd)/data:/app/data \
  -e JWT_SECRET=your-secret-here \
  --name buaa-checkin \
  buaa-checkin
```

### 本地运行 (独立二进制)

您可以直接运行编译后的二进制文件，无需 `static/` 文件夹：

```bash
# 默认监听 3000 端口，数据储存在 ./data
./buaa-checkin

# 自定义端口和数据目录
./buaa-checkin --port 8080 --data-dir /path/to/data
```

### Windows 系统服务

在 Windows 环境下，直接用 **管理员权限** 打开 PowerShell 或 CMD，即可通过内置命令将其注册为系统服务，实现开机自启无感运行：

```bat
# 安装为 Windows 服务 (服务名为 buaa-checkin)
buaa-checkin.exe --install

# 卸载 Windows 服务
buaa-checkin.exe --uninstall
```
*提示：安装服务后，可通过任务管理器的“服务”选项卡或运行 `services.msc` 找到名为 `buaa-checkin` 的服务进行手动启动/停止操作。默认配置为“自动（延迟启动）”以确保网络环境就绪。*

## ⚙️ 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `PORT` | `3000` | HTTP 监听端口 |
| `JWT_SECRET` | `buaa-checkin-default-secret` | JWT 签名密钥（**生产环境必须修改**） |
| `DATA_DIR` | `./data` | 持久化数据目录 |
| `RUST_LOG` | `buaa_checkin=info` | 日志级别 |

## 📁 数据持久化

用户配置保存在 `data/config.json`，格式如下：

```json
{
  "poll_interval_minutes": 10,
  "auto_window_minutes": 15,
  "students": [
    {
      "student_id": "20231234",
      "name": "张三",
      "course_ids": ["course_id_1", "course_id_2"]
    }
  ]
}
```

通过 Docker volume 挂载 `data/` 目录即可实现持久化。

## 🔔 Webhook 通知

签到成功后自动推送通知。在 Web 界面的「通知设置」标签页中配置，或直接编辑 `data/config.json`：

```json
{
  "webhook": {
    "enabled": true,
    "provider": "serverchan",
    "key": "SCT..."
  }
}
```

支持的通知渠道：
- **Server酱** (`serverchan`)：填入 SendKey，通知通过微信推送
- **自定义 Webhook** (`custom`)：POST JSON `{"title": "...", "body": "..."}` 到指定 URL

## 🔒 安全提示

> ⚠️ 本系统设计为**自部署内部工具**，不建议直接暴露在公网。如需公网访问，请配合反向代理（如 Nginx）和 HTTPS。

## 📜 License

MIT
