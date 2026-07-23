# Vibe Dashboard

本地单用户的 AI 编程管理工具。

## 开发启动

### 后端（端口 8787）

```powershell
cd backend
cargo run -p api
```

### 前端（端口 5173）

```powershell
cd frontend
npm install
npm run dev
```

浏览器打开 http://localhost:5173

## 技术栈

- 后端：Rust + Axum + SQLx + SQLite
- 前端：Vite + React + TypeScript + shadcn/ui
