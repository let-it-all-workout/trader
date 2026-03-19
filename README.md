# polym

Personal trading stack dashboard. Rust + Axum + HTMX.

## Run

```bash
cargo run
```

Open [http://localhost:3000](http://localhost:3000).

## Stack

- **Axum** - async web framework
- **Askama** - compile-time HTML templates
- **HTMX** - server-driven interactivity
- **tower-http** - static file serving

## Deploy On Railway (Docker)

This repo now includes a `Dockerfile` ready for Railway.

### 1) Push code to GitHub

Railway deploys from your GitHub repo.

### 2) Create Railway service

- New Project -> Deploy from GitHub repo
- Railway will auto-detect the `Dockerfile`

### 3) Set environment variables

News uses three sources. Set these in Railway service variables:

| Source | Env var | Required |
|--------|---------|----------|
| Google News | — | Always on |
| Brave Search | `BRAVE_API_KEY=...` | For alpha / block-trade / whale queries |
| Twitter (logged-in) | `TWITTER_COOKIE_STRING=...` or `TWITTER_AUTH_TOKEN` + `TWITTER_CT0` | For social alpha |

Also optional: `RUST_LOG=info`, `PUPPETEER_EXECUTABLE_PATH=/usr/bin/chromium` (set in Dockerfile).

`PORT` is provided by Railway automatically.

### 4) Deploy and verify

- Wait for build + deploy to finish
- Open generated Railway URL
- Confirm `/intel/status` and dashboard news cards load

### Notes

- You do not need your local machine on 24/7 once deployed.
- Twitter browser scanning can be heavier than RSS/API feeds; monitor Railway memory/CPU and tune limits if needed.

**Twitter cookie:** Login to X → DevTools → Application → Cookies → `x.com` → copy `auth_token` and `ct0`, or full cookie string. Set as env vars (never commit).
