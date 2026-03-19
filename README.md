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

In Railway service variables, set:

- `BRAVE_API_KEY=...` (your Brave Search key)
- `RUST_LOG=info` (optional)
- `PUPPETEER_EXECUTABLE_PATH=/usr/bin/chromium` (already set in Dockerfile, but safe to keep)

`PORT` is provided by Railway automatically and is used by the app at runtime.

### 4) Deploy and verify

- Wait for build + deploy to finish
- Open generated Railway URL
- Confirm `/intel/status` and dashboard news cards load

### Notes

- You do not need your local machine on 24/7 once deployed.
- Twitter browser scanning can be heavier than RSS/API feeds; monitor Railway memory/CPU and tune limits if needed.

### Optional: Logged-in X/Twitter scan (recommended)

You can run the browser collector with your logged-in X session by setting one of these in Railway variables:

- Preferred: `TWITTER_COOKIE_STRING` (full cookie header from your browser for `x.com`)
- Or minimal pair: `TWITTER_AUTH_TOKEN` and `TWITTER_CT0`

How to get cookies safely:

1. Login to X in your local browser.
2. Open DevTools -> Application/Storage -> Cookies -> `https://x.com`.
3. Copy `auth_token` and `ct0` values, or copy full cookie string.
4. Set them as Railway environment variables (never commit to git).

If these variables are absent, the collector runs in anonymous mode automatically.
