"""Seat imbalance and carry queues in one market under two rounding rules.

Mean-field: per year t, k cells per side with expected seats x[c,t] (women)
and y[c,t] (men), balanced so that sum_c x = sum_c y every year.
Integer seats per cell are drawn locally (each cell on its own):
  A  independent unbiased rounding each year: floor(x) + Bernoulli(frac(x))
  B  cumulative (systematic) rounding along time with a keyed offset u_c:
     n_t = floor(S_t + u_c) - floor(S_{t-1} + u_c), S_t = sum_{s<=t} x[c,s]
Then the market matches min(W + Qw, M + Qm) seats and carries the rest.
Reports: per-year imbalance |W - M|, the carry queue, and mean delay of a
carried seat (Little's law: mean queue / matched per year)."""
import random, math, statistics as st

def run(k, mean_seats, T, rule, seed):
    rng = random.Random(seed)
    # Cell weights vary across cells and drift slowly over time.
    base_w = [rng.gammavariate(2.0, 1.0) for _ in range(k)]
    base_m = [rng.gammavariate(2.0, 1.0) for _ in range(k)]
    uw = [rng.random() for _ in range(k)]
    um = [rng.random() for _ in range(k)]
    Sw = [0.0] * k; Sm = [0.0] * k
    qw = qm = 0
    imb, queue, matched = [], [], 0
    for t in range(T):
        total = mean_seats * k * (1.0 + 0.3 * math.sin(t / 15.0))
        ww = [b * (1 + 0.2 * math.sin(t / 7.0 + i)) for i, b in enumerate(base_w)]
        mm = [b * (1 + 0.2 * math.cos(t / 9.0 + i)) for i, b in enumerate(base_m)]
        sw, sm = sum(ww), sum(mm)
        xw = [total * w / sw for w in ww]
        xm = [total * m / sm for m in mm]
        W = M = 0
        for c in range(k):
            if rule == 'A':
                W += int(xw[c]) + (rng.random() < xw[c] - int(xw[c]))
                M += int(xm[c]) + (rng.random() < xm[c] - int(xm[c]))
            else:
                a = math.floor(Sw[c] + uw[c]); Sw[c] += xw[c]; W += math.floor(Sw[c] + uw[c]) - a
                b = math.floor(Sm[c] + um[c]); Sm[c] += xm[c]; M += math.floor(Sm[c] + um[c]) - b
        imb.append(abs(W - M))
        aw, am = W + qw, M + qm
        n = min(aw, am)
        matched += n
        qw, qm = aw - n, am - n
        queue.append(qw + qm)
    return imb, queue, matched / T

print('k cells/side | mean seats/cell/yr | rule | mean |W-M| | mean queue | queue p99 | queue at T | delay (yr)')
for k, ms in [(40, 0.25), (40, 2.0), (40, 25.0), (150, 2.0)]:
    for rule in 'AB':
        I, Q, Q_last, D = [], [], [], []
        for seed in range(40):
            imb, q, thr = run(k, ms, 250, rule, seed)
            I += imb; Q += q; Q_last.append(q[-1]); D.append(st.mean(q) / thr)
        Q.sort()
        print(f'{k:4d} | {ms:6.2f} | {rule} | {st.mean(I):6.2f} | {st.mean(Q):7.2f} | {Q[int(0.99*len(Q))]:5d} | {st.mean(Q_last):7.2f} | {st.mean(D):.3f}')

# Overflow: L local markets clear first; leftovers of both sexes go to one
# pooled (cross-area) overflow market the same year; only its leftover carries.
def run_overflow(L, k, mean_seats, T, seed):
    rng = random.Random(seed)
    cells = []
    for _ in range(L):
        cells.append(([rng.gammavariate(2.0, 1.0) for _ in range(k)], [rng.gammavariate(2.0, 1.0) for _ in range(k)],
                      [rng.random() for _ in range(k)], [rng.random() for _ in range(k)], [0.0] * k, [0.0] * k))
    qw = qm = 0
    seats = over = queue_sum = 0
    for t in range(T):
        lw = lm = 0
        for (bw, bm, uw, um, Sw, Sm) in cells:
            total = mean_seats * k * (1.0 + 0.3 * math.sin(t / 15.0))
            ww = [b * (1 + 0.2 * math.sin(t / 7.0 + i)) for i, b in enumerate(bw)]
            mm = [b * (1 + 0.2 * math.cos(t / 9.0 + i)) for i, b in enumerate(bm)]
            sw, sm = sum(ww), sum(mm)
            W = M = 0
            for c in range(k):
                x = total * ww[c] / sw; a = math.floor(Sw[c] + uw[c]); Sw[c] += x; W += math.floor(Sw[c] + uw[c]) - a
                y = total * mm[c] / sm; b = math.floor(Sm[c] + um[c]); Sm[c] += y; M += math.floor(Sm[c] + um[c]) - b
            seats += W
            n = min(W, M); lw += W - n; lm += M - n
        over += lw + lm
        aw, am = lw + qw, lm + qm
        n = min(aw, am); qw, qm = aw - n, am - n
        queue_sum += qw + qm
    return over / (2 * seats), (queue_sum / T) / (seats / T)

print()
print('L local markets x k cells/side x mean seats/cell/yr | share of seats overflowing to the pooled market | carried share (delay, yr)')
for L, k, ms in [(60, 40, 0.25), (60, 40, 2.0)]:
    O, D = [], []
    for seed in range(10):
        o, d = run_overflow(L, k, ms, 150, seed)
        O.append(o); D.append(d)
    print(f'{L} x {k} x {ms}: {st.mean(O):.3f} | {st.mean(D):.4f}')
