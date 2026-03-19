const puppeteer = require("puppeteer-core");
const fs = require("fs");

function pickExecutable() {
  const candidates = [
    process.env.PUPPETEER_EXECUTABLE_PATH,
    "/usr/bin/google-chrome",
    "/usr/bin/chromium-browser",
    "/usr/bin/chromium",
    "/usr/bin/firefox",
  ].filter(Boolean);
  for (const c of candidates) {
    try {
      if (fs.existsSync(c)) return c;
    } catch (_) {}
  }
  return "";
}

function toSearchUrl(baseUrl, query) {
  const q = encodeURIComponent(query);
  if (baseUrl.includes("x.com") || baseUrl.includes("twitter.com")) {
    return `${baseUrl.replace(/\/$/, "")}/search?q=${q}&src=typed_query&f=live`;
  }
  return `${baseUrl.replace(/\/$/, "")}/search?f=tweets&q=${q}`;
}

function cookieDomainFromBaseUrl(baseUrl) {
  try {
    const host = new URL(baseUrl).hostname || "x.com";
    return host.startsWith(".") ? host : `.${host}`;
  } catch (_) {
    return ".x.com";
  }
}

function parseCookieString(raw) {
  const out = [];
  for (const pair of String(raw || "").split(";")) {
    const trimmed = pair.trim();
    if (!trimmed) continue;
    const eq = trimmed.indexOf("=");
    if (eq <= 0) continue;
    const name = trimmed.slice(0, eq).trim();
    const value = trimmed.slice(eq + 1).trim();
    if (!name || !value) continue;
    out.push({ name, value });
  }
  return out;
}

async function applyLoginCookies(page, baseUrl) {
  const cookieString = process.env.TWITTER_COOKIE_STRING || process.env.X_COOKIE_STRING || "";
  const authToken = process.env.TWITTER_AUTH_TOKEN || process.env.X_AUTH_TOKEN || "";
  const ct0 = process.env.TWITTER_CT0 || process.env.X_CT0 || "";
  const domain = cookieDomainFromBaseUrl(baseUrl);

  const cookies = [];
  if (cookieString.trim()) {
    for (const c of parseCookieString(cookieString)) {
      cookies.push({
        ...c,
        domain,
        path: "/",
        secure: true,
      });
    }
  } else {
    if (authToken.trim()) {
      cookies.push({
        name: "auth_token",
        value: authToken.trim(),
        domain,
        path: "/",
        secure: true,
      });
    }
    if (ct0.trim()) {
      cookies.push({
        name: "ct0",
        value: ct0.trim(),
        domain,
        path: "/",
        secure: true,
      });
    }
  }

  if (!cookies.length) {
    return false;
  }

  await page.setCookie(...cookies);
  return true;
}

async function collectFromPage(page, baseUrl, query, perQueryLimit) {
  const url = toSearchUrl(baseUrl, query);
  await page.goto(url, { waitUntil: "networkidle2", timeout: 45000 });
  await page.waitForTimeout(1800);

  // Scroll a few times to load additional posts.
  for (let i = 0; i < 4; i++) {
    await page.evaluate(() => window.scrollBy(0, 1000));
    await page.waitForTimeout(1200);
  }

  const items = await page.evaluate((queryInner, limitInner) => {
    function clean(s) {
      return (s || "").replace(/\s+/g, " ").trim();
    }

    const out = [];
    const seen = new Set();

    // X/Twitter format
    const tweetArticles = Array.from(document.querySelectorAll('article[data-testid="tweet"]'));
    for (const a of tweetArticles) {
      const textNode = a.querySelector('[data-testid="tweetText"]');
      const text = clean(textNode ? textNode.innerText : a.innerText);
      const anchors = Array.from(a.querySelectorAll('a[href*="/status/"]'));
      let href = "";
      for (const an of anchors) {
        const raw = an.getAttribute("href") || "";
        if (raw.includes("/status/")) {
          href = raw.startsWith("http") ? raw : `https://x.com${raw}`;
          break;
        }
      }
      if (!text || !href || seen.has(href)) continue;
      seen.add(href);
      out.push({
        title: text.slice(0, 180),
        summary: text.slice(0, 400),
        url: href,
        published: "recent",
        query: queryInner,
      });
      if (out.length >= limitInner) break;
    }

    // Nitter format fallback
    if (out.length < limitInner) {
      const cards = Array.from(document.querySelectorAll(".timeline-item"));
      for (const c of cards) {
        const textEl = c.querySelector(".tweet-content");
        const text = clean(textEl ? textEl.innerText : c.innerText);
        const linkEl = c.querySelector('a[href*="/status/"]');
        const hrefRaw = linkEl ? linkEl.getAttribute("href") || "" : "";
        if (!text || !hrefRaw) continue;
        const href = hrefRaw.startsWith("http")
          ? hrefRaw
          : `https://x.com${hrefRaw.replace(/^\/+/, "")}`;
        if (seen.has(href)) continue;
        seen.add(href);
        out.push({
          title: text.slice(0, 180),
          summary: text.slice(0, 400),
          url: href,
          published: "recent",
          query: queryInner,
        });
        if (out.length >= limitInner) break;
      }
    }

    return out;
  }, query, perQueryLimit);

  return items;
}

async function main() {
  const raw = process.argv[2] || "{}";
  const input = JSON.parse(raw);
  const baseUrl = input.base_url || "https://x.com";
  const queries = Array.isArray(input.queries) ? input.queries : [];
  const perQueryLimit = Number(input.per_query_limit || 10);
  const headless = input.headless !== false;

  if (!queries.length) {
    process.stdout.write("[]");
    return;
  }

  const executablePath = pickExecutable();
  if (!executablePath) {
    process.stderr.write("No browser executable found. Set PUPPETEER_EXECUTABLE_PATH.\n");
    process.exit(1);
  }

  const browser = await puppeteer.launch({
    executablePath,
    headless,
    product: executablePath.includes("firefox") ? "firefox" : undefined,
    args: [
      "--no-sandbox",
      "--disable-setuid-sandbox",
      "--disable-dev-shm-usage",
      "--disable-gpu",
    ],
  });

  try {
    const page = await browser.newPage();
    // Basic hardening helps reduce bot-style blocks on social pages.
    await page.setUserAgent(
      "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36"
    );
    await page.setViewport({ width: 1440, height: 2200 });
    await applyLoginCookies(page, baseUrl);
    const out = [];

    for (const q of queries) {
      try {
        const rows = await collectFromPage(page, baseUrl, q, perQueryLimit);
        out.push(...rows);
      } catch (err) {
        // Keep collecting other queries even if one fails.
      }
    }

    process.stdout.write(JSON.stringify(out));
  } finally {
    await browser.close();
  }
}

main().catch((err) => {
  process.stderr.write(`${err?.message || String(err)}\n`);
  process.exit(1);
});
