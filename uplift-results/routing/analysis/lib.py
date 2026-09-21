import json, math, random
from math import comb, sqrt, log, erf

ROWS = json.load(open('/tmp/mined-rows.json'))
RUN  = json.load(open('/tmp/openjev-run-v2.json'))

# --- post-adjudication reconstruction -------------------------------------
# mined-rows carries the PRE-adjudication label for i=14 ("browser").
# ADR of the paper: turn 14 re-labelled browser -> browser-automation.
def build(post=True):
    judge, lex, meta = [], [], []
    for r in ROWS:
        j, l = r['judge_ok'], r['lex_ok']
        if post and r['i'] == 14:
            # gold becomes browser-automation: judge picked it, lexical picked browser
            j = (r['judge_pick'] == 'browser-automation')
            l = (r['lex_final'] == 'browser-automation')
        judge.append(bool(j)); lex.append(bool(l))
        meta.append(dict(i=r['i'], cls=r['class'], late=bool(r['late']),
                         is_none=bool(r['is_none']), gold=r['expected'],
                         conf=r['judge_conf'], margin=r['lex_margin'],
                         lex_score=r['lex_score']))
    return judge, lex, meta

# --- exact tests ----------------------------------------------------------
def binom_two_sided_exact(k, m, p=0.5):
    """Two-sided exact binomial. For p=0.5 this is the symmetric doubling rule,
    which is also the exact McNemar p-value and the exact sign test."""
    if m == 0: return 1.0
    if p == 0.5:
        k = min(k, m - k)
        tail = sum(comb(m, i) for i in range(0, k + 1)) / 2**m
        return min(1.0, 2 * tail)
    # general: sum of all outcomes no more likely than observed
    pk = comb(m, k) * p**k * (1-p)**(m-k)
    tot = 0.0
    for i in range(m + 1):
        pi = comb(m, i) * p**i * (1-p)**(m-i)
        if pi <= pk * (1 + 1e-9): tot += pi
    return min(1.0, tot)

def norm_sf(z): return 0.5 * (1 - erf(z / sqrt(2)))

def mcnemar_cc(b, c):
    """McNemar chi-square with Yates continuity correction."""
    if b + c == 0: return float('nan'), 1.0
    chi = (abs(b - c) - 1)**2 / (b + c)
    p = 2 * norm_sf(sqrt(max(chi, 0.0)))
    return chi, p

def mcnemar_uncorrected(b, c):
    if b + c == 0: return float('nan'), 1.0
    chi = (b - c)**2 / (b + c)
    return chi, 2 * norm_sf(sqrt(chi))

# --- Tango (1998) score CI for the paired difference p10 - p01 ------------
def _tango_q(d, b, c, n):
    """Constrained MLE of q=p01 given delta=d, by bisection on the score eqn
       b/(q+d) + c/q - 2(n-b-c)/(1-2q-d) = 0."""
    lo, hi = max(0.0, -d) + 1e-12, (1.0 - d) / 2 - 1e-12
    if hi <= lo: return max(lo, 1e-12)
    def f(q):
        t = 1 - 2*q - d
        if t <= 0: return -1e18
        v = 0.0
        if b: v += b / (q + d) if (q + d) > 0 else 1e18
        if c: v += c / q
        v -= 2 * (n - b - c) / t
        return v
    flo, fhi = f(lo), f(hi)
    if flo < 0: return lo
    if fhi > 0: return hi
    for _ in range(300):
        mid = (lo + hi) / 2
        if f(mid) > 0: lo = mid
        else: hi = mid
    return (lo + hi) / 2

def tango_z(d, b, c, n):
    q = _tango_q(d, b, c, n)
    var = n * (2*q + d*(1-d))
    if var <= 0: return float('inf') if (b-c-n*d) > 0 else float('-inf')
    return (b - c - n*d) / sqrt(var)

def tango_ci(b, c, n, z=1.959963984540054):
    """Tango asymptotic score CI for delta = p10-p01. Recommended for paired
       binary differences (Newcombe 1998 / Tango 1998 comparison studies)."""
    dhat = (b - c) / n
    def solve(target, lo, hi):
        for _ in range(300):
            mid = (lo + hi) / 2
            if tango_z(mid, b, c, n) > target: lo = mid
            else: hi = mid
        return (lo + hi) / 2
    low  = solve( z, -0.999999, dhat)   # Z(d)=+z at lower limit
    high = solve(-z, dhat, 0.999999)    # Z(d)=-z at upper limit
    return low, high

def wald_ci(b, c, n, z=1.959963984540054):
    """What the rig currently does: normal approx on the per-item difference."""
    diffs = [1.0]*b + [-1.0]*c + [0.0]*(n-b-c)
    m = sum(diffs)/n
    v = sum((d-m)**2 for d in diffs)/(n-1)
    h = z*sqrt(v/n)
    return m, m-h, m+h, v

# --- paired bootstrap -----------------------------------------------------
def paired_bootstrap(diffs, B=10000, seed=42, clusters=None):
    rng = random.Random(seed)
    n = len(diffs)
    if clusters is None:
        out = []
        for _ in range(B):
            s = sum(diffs[rng.randrange(n)] for _ in range(n))
            out.append(s/n)
    else:
        keys = sorted(set(clusters))
        idx = {k: [i for i,c in enumerate(clusters) if c==k] for k in keys}
        K = len(keys)
        out = []
        for _ in range(B):
            tot, cnt = 0.0, 0
            for _ in range(K):
                for i in idx[keys[rng.randrange(K)]]:
                    tot += diffs[i]; cnt += 1
            out.append(tot/cnt)
    out.sort()
    lo = out[int(math.floor(0.025*B))]
    hi = out[int(math.ceil(0.975*B))-1]
    return sum(out)/B, lo, hi

def wilson(k, n, z=1.959963984540054):
    if n == 0: return (float('nan'),)*2
    p = k/n; d = 1 + z*z/n
    c = (p + z*z/(2*n))/d
    h = z*sqrt(p*(1-p)/n + z*z/(4*n*n))/d
    return c-h, c+h

# --- exact power for the McNemar / sign test ------------------------------
def mcnemar_exact_power(N, p_disc, psi, alpha=0.05):
    """Exact unconditional power: M ~ Bin(N, p_disc) discordants, then the
       two-sided exact binomial test at level alpha conditional on M."""
    tot = 0.0
    for M in range(N + 1):
        pM = comb(N, M) * p_disc**M * (1-p_disc)**(N-M)
        if pM < 1e-15: continue
        rej = 0.0
        for k in range(M + 1):
            if binom_two_sided_exact(k, M) <= alpha:
                rej += comb(M, k) * psi**k * (1-psi)**(M-k)
        tot += pM * rej
    return tot

def holm(pvals_named, alpha=0.05):
    items = sorted(pvals_named.items(), key=lambda kv: kv[1])
    m = len(items); out = {}; running = 0.0
    for r, (k, p) in enumerate(items):
        adj = min(1.0, (m - r) * p)
        running = max(running, adj)
        out[k] = running
    return out
