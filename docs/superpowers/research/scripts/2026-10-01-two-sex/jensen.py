"""Jensen gap of two-sex functions on Poisson pools: E[g(F, M)] vs g(n, n)
for independent F, M ~ Poisson(n). g = harmonic FM/(F+M) and min(F, M)."""
import math
def pois(n, kmax):
    p = [math.exp(-n)]
    for k in range(1, kmax): p.append(p[-1] * n / k)
    return p
print('n | harmonic: E[g] / g(n,n) | min: E[g] / g(n,n)')
for n in (0.5, 1, 2, 5, 10, 30, 100):  # exp(-n) underflows past ~700
    kmax = int(n + 12 * math.sqrt(n) + 30)
    p = pois(n, kmax)
    eh = em = 0.0
    for f in range(kmax):
        for m in range(kmax):
            w = p[f] * p[m]
            if w < 1e-300: continue
            eh += w * (f * m / (f + m) if f + m else 0.0)
            em += w * min(f, m)
    print(f'{n:6} | {eh / (n / 2):.4f} | {em / n:.4f}')
