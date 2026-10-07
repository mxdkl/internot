"""One market-year, two mean-field matching rules, under a cohort-size squeeze.

Singles by single year of age: women 18..45, men 18..50. Attraction kernel
K(gap) = exp(-(gap - 2.1)^2 / (2 * 5.1^2)) over gap = man's age - woman's age
(CPS 2023 mean and SD, see 2026-09-29 note §4). Seekers: a one-sex yearly
first-union hazard h(age) (same schedule both sexes, shifted 2 years later
for men) times the singles.

Rule H (the current ledger): total = harmonic mean of the two sides' wants;
each side's wants scaled to the total; IPF of K to those margins.
Rule C (Choo-Siow with singles): mu_ij = P_ij sqrt(mu_i0 mu_0j), solved by
the Galichon-Salanie IPFP (their Algorithm 1). P is calibrated so that the
baseline reproduces rule H's matrix exactly (Choo-Siow is nonparametric), so
the two rules differ only in how they respond to a change in the pools.
(Choo-Siow is a stock model; here it runs as a flow model on one year's
seekers, as the ledger would use it.)

Scenarios: flat pools (1000 per single year of age), a boom entering the
market (pools 3% larger per year of age below 30) and a bust (3% smaller).
"""
import math

WA = list(range(18, 46)); MA = list(range(18, 51))
def h(a, shift=0):
    a = a - shift
    return 0.02 + 0.13 * math.exp(-((a - 25) ** 2) / (2 * 4.0 ** 2))
K = [[math.exp(-((m - w - 2.1) ** 2) / (2 * 5.1 ** 2)) for m in MA] for w in WA]

def ipf(r, c, iters=500):
    a = [1.0] * len(r); b = [1.0] * len(c)
    for _ in range(iters):
        for i in range(len(r)):
            s = sum(K[i][j] * b[j] for j in range(len(c))); a[i] = r[i] / s if s else 0
        for j in range(len(c)):
            s = sum(K[i][j] * a[i] for i in range(len(r))); b[j] = c[j] / s if s else 0
    return [[a[i] * K[i][j] * b[j] for j in range(len(c))] for i in range(len(r))]

def rule_h(f, m):
    rw = [h(a) * f[i] for i, a in enumerate(WA)]; rm = [h(a, 2) * m[j] for j, a in enumerate(MA)]
    sw, sm = sum(rw), sum(rm); tot = 2 * sw * sm / (sw + sm)
    return ipf([x * tot / sw for x in rw], [x * tot / sm for x in rm])

def rule_cs(f, m, P, iters=3000):
    # Choo-Siow with a calibrated Pi matrix, by IPFP.
    T = [math.sqrt(x) for x in m]
    for _ in range(iters):
        S = [sum(P[i][j] * T[j] for j in range(len(m))) for i in range(len(f))]
        t = [(math.sqrt(S[i] ** 2 + 4 * f[i]) - S[i]) / 2 for i in range(len(f))]
        S = [sum(P[i][j] * t[i] for i in range(len(f))) for j in range(len(m))]
        T = [(math.sqrt(S[j] ** 2 + 4 * m[j]) - S[j]) / 2 for j in range(len(m))]
    return [[P[i][j] * t[i] * T[j] for j in range(len(m))] for i in range(len(f))]

def pools(g):
    # Younger cohorts larger by g per year of age below 30 (a boom entering the market).
    f = [1000.0 * (1 + g) ** max(0, 30 - a) for a in WA]
    m = [1000.0 * (1 + g) ** max(0, 30 - a) for a in MA]
    return f, m

f0, m0 = pools(0.0)
H0 = rule_h(f0, m0)
# Calibrate Choo-Siow to reproduce H0 exactly (it is nonparametric).
s0 = [f0[i] - sum(H0[i]) for i in range(len(WA))]
s1 = [m0[j] - sum(H0[i][j] for i in range(len(WA))) for j in range(len(MA))]
P = [[H0[i][j] / math.sqrt(s0[i] * s1[j]) for j in range(len(MA))] for i in range(len(WA))]
print('rule | younger cohorts larger by | unions | mean gap (y) | rate, women 18-24 | rate, men 20-32')
for g in (0.0, 0.03, -0.03):
    f, m = pools(g)
    for name, mu in (('H', rule_h(f, m)), ('C', rule_cs(f, m, P))):
        tot = sum(map(sum, mu))
        gap = sum(mu[i][j] * (MA[j] - WA[i]) for i in range(len(WA)) for j in range(len(MA))) / tot
        yw = sum(sum(mu[i]) for i, a in enumerate(WA) if a <= 24) / sum(f[i] for i, a in enumerate(WA) if a <= 24)
        mm = sum(mu[i][j] for i in range(len(WA)) for j, a in enumerate(MA) if 20 <= a <= 32) / sum(m[j] for j, a in enumerate(MA) if 20 <= a <= 32)
        print(f'{name} | {g:+.2f}/yr | {tot:8.1f} | {gap:5.2f} | {yw:.4f} | {mm:.4f}')
