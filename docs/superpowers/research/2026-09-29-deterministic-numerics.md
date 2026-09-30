# Deterministic numerics: reproducible floats and exact samplers

**Date:** 2026-09-29
**Question:** Published seeds must build the same world on every machine (spec G1). Which floating-point operations are reproducible in Rust, and which sampling algorithms should the substrate own?

## 1. Rust's float math is not reproducible across machines

The `f64` documentation (doc.rust-lang.org/std/primitive.f64.html) labels most transcendental functions **"Unspecified precision"**:

> "The precision of this function is non-deterministic. This means it varies by platform, Rust version, and can even differ within the same execution from one invocation to the next."

This applies to:
- `exp`, `exp2`, `exp_m1`
- `ln`, `log`, `log2`, `log10`, `ln_1p`
- `sin`, `cos`, `tan` and the inverse and hyperbolic families
- `powf`, `powi`, `cbrt`, `hypot`
- `ln_gamma`, `erf`, `erfc`

By contrast, these are guaranteed to be the correctly rounded IEEE 754 result:
- `+ − * /` (standard IEEE 754 semantics);
- `sqrt` ("guaranteed not to change");
- `mul_add`, `floor`, `ceil`, `round`, `trunc`.

**Consequence for Internot.** Any `ln`/`exp` in the path from seed to fact can change a fact between machines, between Rust versions, or between two calls. Today this includes `hash_gaussian`, `sampler::{lognormal, pareto, exponential}`, trajectory `sin`/`cos`, the event enumerator, and `people` derivations that call them. That violates G1 for published seeds.

## 2. The `libm` crate is a reproducible replacement

`libm` 0.2.16 is the rust-lang pure-Rust math library. Read from the crate source:
- Most functions are ports of FreeBSD msun / musl (`log.rs`: "origin: FreeBSD /usr/src/lib/msun/src/e_log.c").
- Architecture-specific code paths (`math/arch/`) exist only for operations IEEE 754 already defines exactly: `sqrt`, `fma`, `rint`, `floor`, `ceil`, `trunc`, `fabs`.
- The only other `target_arch` branches (`pow.rs`, `rem_pio2.rs`) apply to 32-bit x86 **without SSE2** (x87), which is not a supported target.
- `ln`, `exp`, `log1p`, `expm1`, `pow` and `lgamma` are therefore plain IEEE arithmetic sequences. They are bit-identical on every modern target for a given crate version.

**Decision:**
- All transcendental math in the substrate goes through `procedural_core::dmath`, which wraps `libm`.
- The version is pinned exactly (`libm = "=0.2.16"`) with default features off.
- Golden-value tests pin representative outputs, so a dependency or compiler change that alters a single bit fails loudly.
- New code must not call `f64::ln` and friends. Existing call sites migrate in Phase 0.

## 3. Samplers the substrate owns

Library samplers such as `rand_distr` are not used for world facts, for two reasons:
- they use `std` transcendentals;
- their algorithms can change between versions, which would silently re-roll every world.

The substrate owns these, with golden tests:

| Distribution | Algorithm | Reference |
|---|---|---|
| Uniform stream | counter-based: `u_i = H(key, i)`, top 53 bits | Salmon et al., SC11 (Random123) |
| Exponential | `−ln(1 − u)` | inversion |
| Poisson, λ < 10 | multiplication method: count uniforms until the product ≤ e^−λ | Knuth; numpy `random_poisson_mult` |
| Poisson, λ ≥ 10 | PTRS, transformed rejection with squeeze | Hörmann 1993, *Insurance: Math. & Econ.* 12:39–45; numpy `random_poisson_ptrs` |
| Binomial, min(p,1−p)·n ≤ 30 | inversion with restart bound `n·p + 10·sqrt(n·p·q + 1)` | numpy `random_binomial_inversion` |
| Binomial, otherwise | BTPE | Kachitvichyanukul & Schmeiser 1988; numpy `random_binomial_btpe`, including numpy's correction (the third and fourth error terms in Step 52 are subtracted, fixing the 1988 paper) |
| log Γ(x) | Stirling series with 10 coefficients plus recursion below 7 | numpy `random_loggam` |
| Sorted uniforms (event times in a bucket) | exponential spacings: S_k / S_{n+1} | Devroye 1986, ch. V |

Rejection samplers consume a variable number of uniforms. With a counter-based stream this is still a pure function of the key: the i-th attempt always reads the same numbers.

## Sources

- Rust `f64` documentation, precision notes: https://doc.rust-lang.org/std/primitive.f64.html
- `libm` crate: https://docs.rs/libm/latest/libm/ ; source inspected: `libm-0.2.16/src/math/{arch/mod.rs, log.rs, pow.rs, rem_pio2.rs}`
- numpy `distributions.c` (the Poisson, binomial and loggam implementations): https://github.com/numpy/numpy/blob/main/numpy/random/src/distributions/distributions.c
- Salmon, Moraes, Dror, Shaw, *Parallel random numbers: as easy as 1, 2, 3*, SC11 (cited in `2026-09-29-time-consistent-evolution.md`)
- Devroye, *Non-Uniform Random Variate Generation*, 1986, ch. V–VI
