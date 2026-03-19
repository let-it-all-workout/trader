# Polym — Funding Rate Scanner Development Log

## Overview

Built a personal trading stack web app in **Rust (Axum + HTMX)** that monitors funding rates across 13 DEX perpetual exchanges, detects arbitrage opportunities, and displays them on a live dashboard.

---

## Exchanges Integrated (13 total)

| Exchange | API Period | Notes |
|---|---|---|
| Hyperliquid | 1h | Settled hourly; rate from `metaAndAssetCtxs` |
| dYdX | 1h | `nextFundingRate` from v4 indexer |
| Paradex | 8h | `funding_rate` from `/v1/markets/summary` |
| Drift | 1h (percentage) | Rate is already a %, not decimal fraction |
| Backpack | 1h | Changed to hourly Aug 2025 |
| Orderly | 8h | `est_funding_rate` from bulk endpoint |
| Ethereal | 1h | `fundingRate1h` field confirms period |
| Aster | varies (1h/2h/4h/8h) | Per-symbol interval from `/fapi/v3/fundingInfo` |
| Variational | annual | `funding_rate` is annualized decimal |
| Pacifica | 1h | Per-symbol API calls |
| Hibachi | 8h | `estimatedFundingRate` from `/market/data/prices` |
| Vest | 1h | `oneHrFundingRate` from `/v2/funding/history` |
| Lighter | 8h | Requires `Accept` + `User-Agent` headers; filter for `exchange="lighter"` |

---

## Funding Rate Accuracy Fixes

### 1. Lighter — rates were 8x too high

**Problem:** Lighter's API returns rates in 8h-equivalent format (confirmed by cross-referencing Binance/Bybit/Hyperliquid in the same response), but our code treated them as 1h.

**Proof:** Lighter reports Binance BTC at `0.0000711`. As 1h rate → 62% APR (wrong). As 8h rate → 7.8% APR (correct, matches Binance).

**Fix:** Changed `to_8h_pct(fr.rate, 1.0)` → `to_8h_pct(fr.rate, 8.0)` in `src/funding/lighter.rs`.

### 2. Drift — rates were 100x too high

**Problem:** Drift docs state rates are *"expressed as percentages in decimal form (e.g., 0.00460%)"*. The value `-0.000286` means `-0.000286%` per hour — already a percentage. Our code multiplied by 100 as if it were a decimal fraction.

**Proof:**
- BTC raw = -0.000286. As decimal fraction: -250% APR (absurd). As percentage: -2.5% APR (correct).
- SOL raw = +0.001778. As decimal fraction: +1558% APR (absurd). As percentage: +15.6% APR (correct).

**Fix:** Changed `rate * 8.0 * 100.0` → `rate * 8.0` in `src/funding/drift.rs`.

### 3. Aster — most symbols had wrong rates (up to 8x off)

**Problem:** Aster has different funding intervals per symbol: 201 at 1h, 54 at 4h, 158 at 8h, 3 at 2h. Our code assumed all were 8h.

**Proof:** `/fapi/v3/fundingInfo` returns `fundingIntervalHours` per symbol. TRUTHUSDT (1h) was showing ~10% APR instead of the correct ~85% APR.

**Fix:** Added a prefetch of `/fapi/v3/fundingInfo` to build a per-symbol interval map, then used the correct interval in conversion. See `src/funding/aster.rs`.

### 4. Hyperliquid — rates were 8x too low

**Problem:** Hyperliquid settles funding hourly. The `funding` field from `metaAndAssetCtxs` is a per-hour rate, but our code treated it as per-8h.

**Proof:** The `predictedFundings` endpoint confirms `fundingIntervalHours: 1` for all HlPerp entries. BTC at 0.0000125 per 1h → 10.9% APR (correct), not 1.4% APR (wrong).

**Fix:** Changed `to_8h_pct(rate, 8.0)` → `to_8h_pct(rate, 1.0)` in `src/funding/hyperliquid.rs`.

---

## Verified Accuracy (all 13 exchanges)

| Exchange | BTC 8h Rate | BTC APR | Verified Against |
|---|---|---|---|
| Hyperliquid | +0.0100% | +10.9% | `predictedFundings` API |
| dYdX | +0.0029% | +3.2% | `perpetualMarkets` raw |
| Paradex | +0.0091% | +9.9% | `/v1/markets/summary` raw |
| Drift | -0.0006% | -0.7% | Drift docs + raw API |
| Backpack | +0.0077% | +8.4% | Backpack docs (hourly) |
| Orderly | -0.0017% | -1.8% | `/v1/public/funding_rates` raw |
| Ethereal | +0.0091% | +10.0% | `fundingRate1h` field |
| Aster | +0.0079% | +8.6% | `premiumIndex` + `fundingInfo` |
| Variational | +0.0100% | +10.9% | `/metadata/stats` raw |
| Pacifica | +0.0093% | +10.1% | per-symbol API raw |
| Hibachi | +0.0097% | +10.6% | `/market/data/prices` raw |
| Vest | +0.0100% | +10.9% | `oneHrFundingRate` raw |
| Lighter | +0.0096% | +10.5% | Cross-ref with Binance/Bybit in same API |

---

## Performance Fix — Parallel Exchange Fetching

**Problem:** Scanner fetched 13 exchanges sequentially, taking 60–90+ seconds. During this time, UI showed "No funding rate data yet."

**Fix (3 parts):**

1. **Parallel fetching** — All 13 exchanges now fetch concurrently via `futures::future::join_all`. Scan time dropped from ~90s → ~5–11s.

2. **Per-exchange timeout (30s)** — A hung API no longer blocks the entire scan. Uses `tokio::time::timeout`.

3. **Aggressive UI polling when empty** — Empty state div polls every 2s (not 15s) until data arrives. Dashboard also auto-refreshes every 15s.

**Result:** Data available in ~11 seconds after server restart (was 60–90s).

---

## Stable Arbitrage Strategies Feature

**Motivation:** Funding rates are volatile. "Hot" arbs may flip within minutes. Users want to see which opportunities have been *consistently* profitable.

**Implementation:**

- **Rolling rate history** — Scanner stores last 30 scans of 8h rates per (exchange, symbol) pair in `RateHistory`.
- **Averaged rates** — Computes mean rate over the history window for each pair.
- **Stability score** — Fraction of scan cycles where the spread between two exchanges was positive (0.0–1.0).
- **Stable arb detection** — Requires: ≥3 samples, ≥70% consistency, minimum spread threshold.
- **UI** — Two-tab view (Hot / Stable) with tab state persisted across HTMX auto-refreshes via `window._arbTab`.

**Stable card shows:** avg spread APR, consistency %, sample count, current vs average spread, long/short venues with avg APR.

---

## Architecture

```
src/
├── main.rs              # Axum router, handlers, templates, AppState
├── config.rs            # TOML config (polym.toml)
├── telegram.rs          # Telegram bot API client
├── agents/
│   ├── mod.rs           # Agent trait, AgentManager, tick loop
│   ├── scanner.rs       # ScannerAgent — parallel fetch, history, arb detection
│   ├── executor.rs      # (placeholder)
│   ├── analytics.rs     # (placeholder)
│   └── signals.rs       # (placeholder)
├── funding/
│   ├── mod.rs           # FundingRate, ArbOpportunity, StableArbOpportunity,
│   │                    # RateHistory, detect_arbs, detect_stable_arbs,
│   │                    # normalize_symbol, to_8h_pct, to_annual_pct
│   ├── hyperliquid.rs
│   ├── dydx.rs
│   ├── paradex.rs
│   ├── drift.rs
│   ├── backpack.rs
│   ├── orderly.rs
│   ├── ethereal.rs
│   ├── aster.rs
│   ├── variational.rs
│   ├── pacifica.rs
│   ├── hibachi.rs
│   ├── vest.rs
│   └── lighter.rs
templates/
├── index.html
├── dashboard.html
├── agent_view.html
├── funding_page.html    # Stats row, filter chips, funding data container
├── funding_table.html   # Hot/Stable tabs, arb cards, rates table, search
└── coming_soon.html
static/
└── style.css
```

---

## Key Design Decisions

- **Rate normalization:** `to_8h_pct(rate_decimal, period_hours)` converts any rate to 8h-equivalent percentage. `to_annual_pct(rate_8h_pct)` = rate × 3 × 365.
- **Symbol normalization:** Exchange-specific logic strips suffixes (-PERP, -USD, /USDT-P, etc.) to match symbols across exchanges.
- **Arb detection:** For each normalized symbol, finds the lowest and highest rate across exchanges. Spread = highest − lowest.
- **HTMX-driven UI:** Server-rendered HTML fragments; `hx-get`/`hx-trigger` for live updates without a JS framework.
- **Shared state:** `Arc<RwLock<Vec<FundingRate>>>` and similar for rates, arbs, stable arbs, filter — shared between scanner task and HTTP handlers.

---

## Polymarket URL & Price Fixes (2026-03-01)

### 1. Market URLs returning 404 for multi-outcome events

**Problem:** All 4 modules fetching from the `/markets` endpoint (`endgame.rs`, `marketmaking.rs`, `newsarb.rs`, `crossplatform.rs`) built URLs using the **market slug** (`m.get("slug")`). For multi-outcome events (negRisk), the market slug differs from the event slug:

- Market slug: `will-trump-nominate-judy-shelton-as-the-next-fed-chair`
- Event slug: `who-will-trump-nominate-as-fed-chair`

Only `https://polymarket.com/event/{event_slug}` works. Using the market slug returns **404**.

**Verified live:**
- `polymarket.com/event/will-trump-nominate-judy-shelton-as-the-next-fed-chair` → **404**
- `polymarket.com/event/who-will-trump-nominate-as-fed-chair` → **200**

**Fix:** Changed slug extraction in all 4 files to pull from the embedded events array first:

```rust
let slug = m.get("events")
    .and_then(|v| v.as_array())
    .and_then(|events| events.first())
    .and_then(|e| e.get("slug"))
    .and_then(|v| v.as_str())
    .or_else(|| m.get("slug").and_then(|v| v.as_str()))
    .unwrap_or("")
    .to_string();
```

The 2 modules using the `/events` endpoint (`negrisk.rs`, `correlation.rs`) already used the event slug directly and were unaffected.

### 2. Prices not matching Polymarket website

**Problem:** All `extract_yes_price()` functions used `outcomePrices` from the gamma API. While usually close to CLOB data, this field can lag behind the live order book. Polymarket's website displays the `lastTradePrice` (last CLOB trade), not the `outcomePrices` mid.

Additionally, card chips displayed prices with 2 decimal places (`{:.2}`), causing `$0.998` to round to `$1.00` — visibly wrong compared to Polymarket showing `99.8¢`.

**Fix (2 parts):**

1. **Price source priority** — Updated `extract_yes_price()` in `negrisk.rs`, `correlation.rs`, `newsarb.rs`, `crossplatform.rs`, and `extract_prices()` in `endgame.rs` to prefer CLOB data:
   - First: `lastTradePrice` (matches Polymarket display)
   - Second: CLOB mid `(bestBid + bestAsk) / 2`
   - Fallback: `outcomePrices`

2. **Display precision** — Updated `polymarket_table.html` to show prices with 3 decimal places (`{:.3}`) instead of 2 on all card chips (endgame buy price, market making bid/ask, cross-platform PM/KS prices, news arb current price).

### Files changed

| File | Changes |
|------|---------|
| `src/polymarket/endgame.rs` | Event slug extraction, CLOB-first price extraction |
| `src/polymarket/marketmaking.rs` | Event slug extraction |
| `src/polymarket/newsarb.rs` | Event slug extraction, CLOB-first price extraction |
| `src/polymarket/crossplatform.rs` | Event slug extraction, CLOB-first price extraction |
| `src/polymarket/negrisk.rs` | CLOB-first price extraction |
| `src/polymarket/correlation.rs` | CLOB-first price extraction, added `as_f64` helper |
| `templates/polymarket_table.html` | `{:.2}` → `{:.3}` for price display on cards |
