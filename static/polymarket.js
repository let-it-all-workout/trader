(function() {
    "use strict";

    /* ── Tabs ── */

    window.initPmTabs = function() {
        var saved = (typeof window._pmTab === "string") ? window._pmTab : "negrisk";
        var tabs = document.querySelectorAll("#pmTabs .pm__tab");
        if (!tabs.length) return;

        function activate(name) {
            window._pmTab = name;
            tabs.forEach(function(t) {
                t.classList.toggle("active", t.getAttribute("data-tab") === name);
            });
            document.querySelectorAll(".arb-pane").forEach(function(p) {
                p.classList.toggle("show", p.id === "pm-" + name);
            });
        }

        tabs.forEach(function(t) {
            t.addEventListener("click", function(e) {
                e.preventDefault();
                activate(this.getAttribute("data-tab"));
            });
        });

        activate(saved);
    };

    /* ── Detail Overlay ── */

    function openDetail(card) {
        var overlay = document.getElementById("pmDetail");
        if (!overlay) return;

        var type = card.getAttribute("data-pm");
        var title = card.getAttribute("data-title");

        document.getElementById("pdTitle").textContent = title;

        var meta = "";
        var content = "";
        var links = "";

        if (type === "crossplatform") {
            var polyUrl = card.getAttribute("data-poly-url") || "";
            var kalshiUrl = card.getAttribute("data-kalshi-url") || "";
            if (polyUrl) links += '<a href="' + esc(polyUrl) + '" target="_blank" rel="noopener" class="pd-link">Polymarket &#8599;</a>';
            if (kalshiUrl) links += '<a href="' + esc(kalshiUrl) + '" target="_blank" rel="noopener" class="pd-link">Kalshi &#8599;</a>';
        } else {
            var url = card.getAttribute("data-url") || "";
            if (url) links = '<a href="' + esc(url) + '" target="_blank" rel="noopener" class="pd-link">View on Polymarket &#8599;</a>';
        }

        if (type === "negrisk") {
            var pct = parseFloat(card.getAttribute("data-pct"));
            var n = parseInt(card.getAttribute("data-outcomes"));
            var yesSum = parseFloat(card.getAttribute("data-sum"));
            var mis = parseFloat(card.getAttribute("data-mispricing"));
            var vol = parseFloat(card.getAttribute("data-vol"));
            var liq = parseFloat(card.getAttribute("data-liq"));

            meta = '<span class="big ' + (pct > 5 ? "hot" : "") + '">' + pct.toFixed(1) + '%</span>'
                 + '<span>' + n + ' outcomes</span>'
                 + '<span class="mono">&Sigma; $' + yesSum.toFixed(3) + '</span>'
                 + (mis > 0 ? '<span class="red">OVERPRICED</span>' : '<span class="green">UNDERPRICED</span>')
                 + '<span>Vol $' + fmt(vol) + '</span>'
                 + '<span>Liq $' + fmt(liq) + '</span>';

            var outcomes = buildOutcomes(card);

            var strat;
            if (mis > 0) {
                var noCost = n - yesSum;
                strat = '<ol class="pd-strat">'
                    + '<li>Buy <strong>NO</strong> on all ' + n + ' outcomes</li>'
                    + '<li>Cost per set = <strong>$' + noCost.toFixed(4) + '</strong> (sum of NO prices)</li>'
                    + '<li>One resolves YES &rarr; ' + (n - 1) + ' NOs pay <strong>$1.00</strong> each</li>'
                    + '<li>Payout = <strong>$' + (n - 1) + '</strong>, profit = <strong>$' + mis.toFixed(4) + '</strong>/set (' + pct.toFixed(1) + '%)</li>'
                    + '</ol>';
            } else {
                var absMis = Math.abs(mis);
                strat = '<ol class="pd-strat">'
                    + '<li>Buy <strong>YES</strong> on all ' + n + ' outcomes</li>'
                    + '<li>Cost per set = <strong>$' + yesSum.toFixed(4) + '</strong> (sum of YES prices)</li>'
                    + '<li>One resolves YES &rarr; payout <strong>$1.00</strong></li>'
                    + '<li>Profit = <strong>$' + absMis.toFixed(4) + '</strong>/set (' + pct.toFixed(1) + '%)</li>'
                    + '</ol>';
            }

            content = '<div class="pd-grid">'
                + '<div class="pd-col"><div class="pd-label">Outcomes</div><div class="pd-outcomes">' + outcomes + '</div></div>'
                + '<div class="pd-col pd-col--right">'
                + '<div class="pd-label">Strategy</div>' + strat
                + '<div class="pd-calc"><div class="pd-label">Calculator</div>'
                + '<div class="pd-calc-input"><span>$</span><input type="number" class="pm-demo-input" data-calc="negrisk" data-n="' + n + '" data-sum="' + yesSum + '" data-mispricing="' + mis + '" placeholder="1,000" min="1"></div>'
                + '<div class="pd-calc-out"></div></div>'
                + '</div></div>';

        } else if (type === "endgame") {
            var side = card.getAttribute("data-side");
            var price = parseFloat(card.getAttribute("data-price"));
            var profitPct = parseFloat(card.getAttribute("data-profit"));
            var days = parseInt(card.getAttribute("data-days"));
            var vol = parseFloat(card.getAttribute("data-vol"));
            var profitPerShare = 1.0 - price;

            meta = '<span class="big">+' + profitPct.toFixed(1) + '%</span>'
                 + '<span class="mono">$' + profitPerShare.toFixed(3) + '/share profit</span>'
                 + '<span class="' + (side === "YES" ? "green" : "red") + '">' + side + '</span>'
                 + '<span class="mono">Buy @ $' + price.toFixed(3) + '</span>'
                 + '<span>Resolves in ' + days + 'd</span>'
                 + '<span>Vol $' + fmt(vol) + '</span>';

            var strat = '<ol class="pd-strat">'
                + '<li>Buy <strong>' + side + '</strong> at <strong>$' + price.toFixed(3) + '</strong> per share</li>'
                + '<li>Hold <strong>' + days + ' days</strong> until market resolves</li>'
                + '<li>' + side + ' wins &rarr; each share pays <strong>$1.00</strong></li>'
                + '<li>Profit per share: <strong>$' + profitPerShare.toFixed(3) + '</strong> (' + profitPct.toFixed(1) + '% return)</li>'
                + '</ol>';

            content = '<div class="pd-grid">'
                + '<div class="pd-col"><div class="pd-label">Strategy</div>' + strat + '</div>'
                + '<div class="pd-col pd-col--right">'
                + '<div class="pd-label">Calculator</div>'
                + '<div class="pd-calc-input"><span>$</span><input type="number" class="pm-demo-input" data-calc="endgame" data-price="' + price + '" data-days="' + days + '" placeholder="1,000" min="1"></div>'
                + '<div class="pd-calc-out"></div>'
                + '</div></div>';

        } else if (type === "correlation") {
            var roi = parseFloat(card.getAttribute("data-pct"));
            var spread = parseFloat(card.getAttribute("data-spread"));
            var num = parseInt(card.getAttribute("data-num"));
            var sum = parseFloat(card.getAttribute("data-sum"));

            var profitPerSet = Math.abs(sum - 1.0);
            var costPerSet = sum > 1.0 ? (num - sum) : sum;

            meta = '<span class="big">+' + roi.toFixed(1) + '% return</span>'
                 + '<span class="mono">$' + profitPerSet.toFixed(3) + ' profit/set</span>'
                 + '<span>' + num + ' markets</span>'
                 + '<span class="mono">&Sigma; $' + sum.toFixed(3) + '</span>'
                 + '<span>Spread: ' + spread.toFixed(1) + '%</span>';

            var outcomes = buildOutcomes(card);

            var strat;
            if (sum > 1.0) {
                var noCost = num - sum;
                strat = '<ol class="pd-strat">'
                    + '<li>Buy <strong>NO</strong> on all ' + num + ' markets</li>'
                    + '<li>Cost per set = <strong>$' + noCost.toFixed(4) + '</strong> (sum of NO prices)</li>'
                    + '<li>One resolves YES &rarr; ' + (num - 1) + ' NOs pay <strong>$1.00</strong> each = <strong>$' + (num - 1) + '</strong></li>'
                    + '<li>Profit per set: <strong>$' + (sum - 1).toFixed(4) + '</strong> (+' + roi.toFixed(1) + '% return)</li>'
                    + '</ol>';
            } else {
                strat = '<ol class="pd-strat">'
                    + '<li>Buy <strong>YES</strong> on all ' + num + ' markets</li>'
                    + '<li>Cost per set = <strong>$' + sum.toFixed(4) + '</strong> (sum of YES prices)</li>'
                    + '<li>One resolves YES &rarr; payout <strong>$1.00</strong></li>'
                    + '<li>Profit per set: <strong>$' + (1 - sum).toFixed(4) + '</strong> (+' + roi.toFixed(1) + '% return)</li>'
                    + '</ol>';
            }

            content = '<div class="pd-grid">'
                + '<div class="pd-col"><div class="pd-label">Markets</div><div class="pd-outcomes">' + outcomes + '</div></div>'
                + '<div class="pd-col pd-col--right">'
                + '<div class="pd-label">Strategy</div>' + strat
                + '<div class="pd-calc"><div class="pd-label">Calculator</div>'
                + '<div class="pd-calc-input"><span>$</span><input type="number" class="pm-demo-input" data-calc="correlation" data-sum="' + sum + '" data-num="' + num + '" placeholder="1,000" min="1"></div>'
                + '<div class="pd-calc-out"></div></div>'
                + '</div></div>';

        } else if (type === "marketmaking") {
            var bid = parseFloat(card.getAttribute("data-bid"));
            var ask = parseFloat(card.getAttribute("data-ask"));
            var spreadPct = parseFloat(card.getAttribute("data-spread"));
            var mid = parseFloat(card.getAttribute("data-mid"));
            var vol = parseFloat(card.getAttribute("data-vol"));
            var liq = parseFloat(card.getAttribute("data-liq"));
            var bidDepth = parseFloat(card.getAttribute("data-bid-depth"));
            var askDepth = parseFloat(card.getAttribute("data-ask-depth"));
            var spreadAbs = ask - bid;

            meta = '<span class="big">' + spreadPct.toFixed(1) + '% spread</span>'
                 + '<span class="mono">Bid $' + bid.toFixed(3) + '</span>'
                 + '<span class="mono">Ask $' + ask.toFixed(3) + '</span>'
                 + '<span class="mono">Mid $' + mid.toFixed(3) + '</span>'
                 + '<span>Vol $' + fmt(vol) + '</span>'
                 + '<span>Liq $' + fmt(liq) + '</span>';

            var strat = '<ol class="pd-strat">'
                + '<li>Place a <strong>BUY</strong> limit order at <strong>$' + bid.toFixed(3) + '</strong> (bid)</li>'
                + '<li>Place a <strong>SELL</strong> limit order at <strong>$' + ask.toFixed(3) + '</strong> (ask)</li>'
                + '<li>When both fill, you earn the spread: <strong>$' + spreadAbs.toFixed(4) + '</strong> per share (' + spreadPct.toFixed(1) + '%)</li>'
                + '<li>Repeat as market trades back and forth around the mid price</li>'
                + '</ol>';

            var depthHtml = '<div class="pd-label" style="margin-top:1rem">Book Depth (Top 5)</div>'
                + '<div class="pd-outcomes">'
                + '<div class="pd-o"><span class="pd-o__name">Bid side</span><div class="pd-o__bar"><div style="width:' + Math.min(bidDepth / Math.max(bidDepth, askDepth, 1) * 100, 100).toFixed(0) + '%"></div></div><span class="pd-o__price green">' + bidDepth.toFixed(0) + ' shares</span></div>'
                + '<div class="pd-o"><span class="pd-o__name">Ask side</span><div class="pd-o__bar"><div style="width:' + Math.min(askDepth / Math.max(bidDepth, askDepth, 1) * 100, 100).toFixed(0) + '%"></div></div><span class="pd-o__price red">' + askDepth.toFixed(0) + ' shares</span></div>'
                + '</div>';

            content = '<div class="pd-grid">'
                + '<div class="pd-col"><div class="pd-label">Strategy</div>' + strat + depthHtml + '</div>'
                + '<div class="pd-col pd-col--right">'
                + '<div class="pd-label">Calculator</div>'
                + '<div class="pd-calc-input"><span>$</span><input type="number" class="pm-demo-input" data-calc="marketmaking" data-bid="' + bid + '" data-ask="' + ask + '" placeholder="1,000" min="1"></div>'
                + '<div class="pd-calc-out"></div>'
                + '</div></div>';

        } else if (type === "crossplatform") {
            var polyYes = parseFloat(card.getAttribute("data-poly-yes"));
            var kalshiYes = parseFloat(card.getAttribute("data-kalshi-yes"));
            var spreadPct = parseFloat(card.getAttribute("data-spread"));
            var buyPlatform = card.getAttribute("data-buy");
            var sellPlatform = card.getAttribute("data-sell");
            var vol = parseFloat(card.getAttribute("data-vol"));
            var polyQ = card.getAttribute("data-poly-q");
            var kalshiTicker = card.getAttribute("data-kalshi-ticker");
            var buyPrice = Math.min(polyYes, kalshiYes);
            var sellPrice = Math.max(polyYes, kalshiYes);

            meta = '<span class="big">' + spreadPct.toFixed(1) + '% spread</span>'
                 + '<span class="mono">PM YES $' + polyYes.toFixed(3) + '</span>'
                 + '<span class="mono">Kalshi YES $' + kalshiYes.toFixed(3) + '</span>'
                 + '<span class="green">Buy on ' + esc(buyPlatform) + '</span>'
                 + '<span class="red">Sell on ' + esc(sellPlatform) + '</span>'
                 + '<span>Vol $' + fmt(vol) + '</span>';

            var strat = '<ol class="pd-strat">'
                + '<li>Buy <strong>YES</strong> on <strong>' + esc(buyPlatform) + '</strong> at <strong>$' + buyPrice.toFixed(3) + '</strong></li>'
                + '<li>Sell <strong>YES</strong> on <strong>' + esc(sellPlatform) + '</strong> at <strong>$' + sellPrice.toFixed(3) + '</strong></li>'
                + '<li>If event resolves YES: both settle at $1, net = spread profit</li>'
                + '<li>If event resolves NO: both worthless, net = spread profit already locked</li>'
                + '</ol>';

            var details = '<div class="pd-label" style="margin-top:1rem">Market Details</div>'
                + '<div class="pd-outcomes">'
                + '<div class="pd-o"><span class="pd-o__name">Polymarket</span><div class="pd-o__bar"><div style="width:' + (polyYes * 100).toFixed(0) + '%"></div></div><span class="pd-o__price">$' + polyYes.toFixed(3) + '</span></div>'
                + '<div class="pd-o"><span class="pd-o__name">Kalshi (' + esc(kalshiTicker) + ')</span><div class="pd-o__bar"><div style="width:' + (kalshiYes * 100).toFixed(0) + '%"></div></div><span class="pd-o__price">$' + kalshiYes.toFixed(3) + '</span></div>'
                + '</div>';

            content = '<div class="pd-grid">'
                + '<div class="pd-col"><div class="pd-label">Strategy</div>' + strat + details + '</div>'
                + '<div class="pd-col pd-col--right">'
                + '<div class="pd-label">Calculator</div>'
                + '<div class="pd-calc-input"><span>$</span><input type="number" class="pm-demo-input" data-calc="crossplatform" data-buy-price="' + buyPrice + '" data-sell-price="' + sellPrice + '" placeholder="1,000" min="1"></div>'
                + '<div class="pd-calc-out"></div>'
                + '</div></div>';

        } else if (type === "newsarb") {
            var price = parseFloat(card.getAttribute("data-price"));
            var changePct = parseFloat(card.getAttribute("data-change"));
            var dir = card.getAttribute("data-dir");
            var vol = parseFloat(card.getAttribute("data-vol"));
            var liq = parseFloat(card.getAttribute("data-liq"));
            var eventTitle = card.getAttribute("data-event");

            meta = '<span class="big ' + (changePct > 15 ? "hot" : "") + '">' + changePct.toFixed(1) + '% move</span>'
                 + '<span class="' + (dir === "UP" ? "green" : "red") + '">' + dir + '</span>'
                 + '<span class="mono">Current $' + price.toFixed(3) + '</span>'
                 + '<span>Vol $' + fmt(vol) + '</span>'
                 + '<span>Liq $' + fmt(liq) + '</span>';

            var strat;
            if (dir === "UP") {
                strat = '<ol class="pd-strat">'
                    + '<li>Price surged <strong>' + changePct.toFixed(1) + '%</strong> &mdash; likely news-driven</li>'
                    + '<li>If news is priced in: price may <strong>revert</strong> &rarr; consider buying <strong>NO</strong> at $' + (1 - price).toFixed(3) + '</li>'
                    + '<li>If momentum continues: ride the trend &rarr; buy <strong>YES</strong> at $' + price.toFixed(3) + '</li>'
                    + '<li>Check the news source before acting &mdash; verify the catalyst</li>'
                    + '</ol>';
            } else {
                strat = '<ol class="pd-strat">'
                    + '<li>Price dropped <strong>' + changePct.toFixed(1) + '%</strong> &mdash; likely news-driven</li>'
                    + '<li>If news is priced in: price may <strong>revert</strong> &rarr; consider buying <strong>YES</strong> at $' + price.toFixed(3) + '</li>'
                    + '<li>If momentum continues: short via <strong>NO</strong> at $' + (1 - price).toFixed(3) + '</li>'
                    + '<li>Check the news source before acting &mdash; verify the catalyst</li>'
                    + '</ol>';
            }

            content = '<div class="pd-grid">'
                + '<div class="pd-col"><div class="pd-label">Strategy</div>' + strat + '</div>'
                + '<div class="pd-col pd-col--right">'
                + '<div class="pd-label">Calculator (Trend Trade)</div>'
                + '<div class="pd-calc-input"><span>$</span><input type="number" class="pm-demo-input" data-calc="newsarb" data-price="' + price + '" data-dir="' + dir + '" placeholder="1,000" min="1"></div>'
                + '<div class="pd-calc-out"></div>'
                + '</div></div>';
        }

        document.getElementById("pdMeta").innerHTML = meta + (links ? '<div class="pd-links">' + links + '</div>' : '');
        document.getElementById("pdContent").innerHTML = content;
        overlay.classList.add("open");
        document.body.style.overflow = "hidden";
    }

    function buildOutcomes(card) {
        var data = card.querySelector(".pm-c__outcomes-data");
        if (!data) return "";
        var rows = data.querySelectorAll("[data-q]");
        var html = "";
        rows.forEach(function(r) {
            var q = r.getAttribute("data-q");
            var p = parseFloat(r.getAttribute("data-p"));
            var w = Math.max(p * 100, 1);
            html += '<div class="pd-o">'
                + '<span class="pd-o__name">' + esc(q) + '</span>'
                + '<div class="pd-o__bar"><div style="width:' + w.toFixed(0) + '%"></div></div>'
                + '<span class="pd-o__price">$' + p.toFixed(2) + '</span>'
                + '</div>';
        });
        return html;
    }

    function closeDetail() {
        var overlay = document.getElementById("pmDetail");
        if (overlay) {
            overlay.classList.remove("open");
            document.body.style.overflow = "";
        }
    }

    function esc(s) {
        var d = document.createElement("div");
        d.textContent = s;
        return d.innerHTML;
    }

    function fmt(n) {
        if (n >= 1e6) return (n / 1e6).toFixed(1) + "M";
        if (n >= 1e3) return (n / 1e3).toFixed(0) + "k";
        return n.toFixed(0);
    }

    /* ── Event Delegation ── */

    document.addEventListener("click", function(e) {
        var card = e.target.closest(".pm-c");
        if (card) { openDetail(card); return; }

        if (e.target.closest("#pdClose")) { closeDetail(); return; }

        if (e.target.closest(".pm-detail__backdrop")) { closeDetail(); return; }
    });

    document.addEventListener("keydown", function(e) {
        if (e.key === "Escape") closeDetail();
    });

    /* ── Calculator ── */

    document.addEventListener("input", function(e) {
        var input = e.target;
        if (!input.classList.contains("pm-demo-input")) return;

        var stake = parseFloat(input.value);
        var out = input.closest(".pd-col--right, .pd-calc");
        if (!out) return;
        var calcOut = out.querySelector(".pd-calc-out");
        if (!calcOut) return;

        if (!stake || stake <= 0) { calcOut.innerHTML = ""; return; }

        var type = input.getAttribute("data-calc");
        var html = "";

        if (type === "negrisk") {
            var mis = parseFloat(input.getAttribute("data-mispricing"));
            var n = parseInt(input.getAttribute("data-n"));
            var yesSum = parseFloat(input.getAttribute("data-sum"));

            if (mis > 0) {
                var costPerSet = n - yesSum;
                if (costPerSet <= 0) costPerSet = 0.001;
                var sets = stake / costPerSet;
                var payout = sets * (n - 1);
                var profit = payout - stake;
                var roi = (profit / stake) * 100;
                html = rw("NO cost / set", "$" + costPerSet.toFixed(4))
                     + rw("Sets", sets.toFixed(1))
                     + rw("Payout", "$" + payout.toFixed(2), "pos")
                     + rw("Net Profit", "$" + profit.toFixed(2) + " (" + roi.toFixed(1) + "%)", "pos", true);
            } else {
                var costPerSet = yesSum;
                if (costPerSet <= 0) costPerSet = 0.001;
                var sets = stake / costPerSet;
                var payout = sets * 1.0;
                var profit = payout - stake;
                var roi = (profit / stake) * 100;
                html = rw("YES cost / set", "$" + costPerSet.toFixed(4))
                     + rw("Sets", sets.toFixed(1))
                     + rw("Payout", "$" + payout.toFixed(2), "pos")
                     + rw("Net Profit", "$" + profit.toFixed(2) + " (" + roi.toFixed(1) + "%)", "pos", true);
            }

        } else if (type === "endgame") {
            var price = parseFloat(input.getAttribute("data-price"));
            var days = parseInt(input.getAttribute("data-days"));
            var shares = stake / price;
            var payout = shares * 1.0;
            var profit = payout - stake;
            var roi = (profit / stake) * 100;
            html = rw("Shares bought", shares.toFixed(1) + " @ $" + price.toFixed(3))
                 + rw("Payout on close", "$" + payout.toFixed(2), "pos")
                 + rw("Net Profit", "$" + profit.toFixed(2), "pos", true)
                 + rw("Return", "+" + roi.toFixed(1) + "% in " + days + "d", "pos", true);

        } else if (type === "correlation") {
            var sum = parseFloat(input.getAttribute("data-sum"));
            var num = parseInt(input.getAttribute("data-num"));
            if (sum > 1) {
                var noCost = num - sum;
                if (noCost <= 0) noCost = 0.001;
                var sets = stake / noCost;
                var payout = sets * (num - 1);
                var profit = payout - stake;
                var roi = (profit / stake) * 100;
                html = rw("NO cost / set", "$" + noCost.toFixed(4))
                     + rw("Sets bought", sets.toFixed(1))
                     + rw("Payout on close", "$" + payout.toFixed(2), "pos")
                     + rw("Net Profit", "$" + profit.toFixed(2), "pos", true)
                     + rw("Return", "+" + roi.toFixed(1) + "%", "pos", true);
            } else {
                var sets = stake / sum;
                var payout = sets * 1.0;
                var profit = payout - stake;
                var roi = (profit / stake) * 100;
                html = rw("YES cost / set", "$" + sum.toFixed(4))
                     + rw("Sets bought", sets.toFixed(1))
                     + rw("Payout on close", "$" + payout.toFixed(2), "pos")
                     + rw("Net Profit", "$" + profit.toFixed(2), "pos", true)
                     + rw("Return", "+" + roi.toFixed(1) + "%", "pos", true);
            }

        } else if (type === "marketmaking") {
            var bid = parseFloat(input.getAttribute("data-bid"));
            var ask = parseFloat(input.getAttribute("data-ask"));
            var spreadPerShare = ask - bid;
            if (spreadPerShare <= 0) spreadPerShare = 0.001;
            var sharesPerRound = stake / ask;
            var profitPerRound = sharesPerRound * spreadPerShare;
            var roi = (spreadPerShare / ask) * 100;
            html = rw("Spread / share", "$" + spreadPerShare.toFixed(4))
                 + rw("Shares / round", sharesPerRound.toFixed(1) + " @ $" + ask.toFixed(3))
                 + rw("Profit / round", "$" + profitPerRound.toFixed(2), "pos", true)
                 + rw("Return / round", "+" + roi.toFixed(1) + "%", "pos", true);

        } else if (type === "crossplatform") {
            var buyPrice = parseFloat(input.getAttribute("data-buy-price"));
            var sellPrice = parseFloat(input.getAttribute("data-sell-price"));
            var spreadPerShare = sellPrice - buyPrice;
            if (spreadPerShare <= 0) spreadPerShare = 0.001;
            var shares = stake / buyPrice;
            var revenue = shares * sellPrice;
            var profit = revenue - stake;
            var roi = (profit / stake) * 100;
            html = rw("Buy @ ", "$" + buyPrice.toFixed(3))
                 + rw("Sell @ ", "$" + sellPrice.toFixed(3))
                 + rw("Shares", shares.toFixed(1))
                 + rw("Revenue", "$" + revenue.toFixed(2))
                 + rw("Net Profit", "$" + profit.toFixed(2), "pos", true)
                 + rw("Return", "+" + roi.toFixed(1) + "%", "pos", true);

        } else if (type === "newsarb") {
            var price = parseFloat(input.getAttribute("data-price"));
            var dir = input.getAttribute("data-dir");
            var buyAt = (dir === "UP") ? price : (1 - price);
            var shares = stake / buyAt;
            var payout = shares * 1.0;
            var profit = payout - stake;
            var roi = (profit / stake) * 100;
            var sideName = (dir === "UP") ? "YES (ride momentum)" : "NO (ride momentum)";
            html = rw("Side", sideName)
                 + rw("Buy @", "$" + buyAt.toFixed(3))
                 + rw("Shares", shares.toFixed(1))
                 + rw("Payout if correct", "$" + payout.toFixed(2), "pos")
                 + rw("Net Profit", "$" + profit.toFixed(2), "pos", true)
                 + rw("Return", "+" + roi.toFixed(1) + "%", "pos", true);
        }

        calcOut.innerHTML = html;
    });

    function rw(label, value, cls, big) {
        var x = big ? " pd-row--big" : "";
        var v = cls ? ' class="' + cls + '"' : "";
        return '<div class="pd-row' + x + '"><span>' + label + '</span><span' + v + '>' + value + '</span></div>';
    }

})();
