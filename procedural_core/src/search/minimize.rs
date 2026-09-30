//! Value-set minimization via Quine-McCluskey logic minimization.

use crate::search::BitPattern;

/// Reduce a set of field values into the minimal set of `BitPattern`s whose
/// union is exactly the input set (duplicates are deduplicated).
///
/// Uses Quine-McCluskey minimization (via the `quine-mccluskey` crate).
///
/// # Panics
/// * if `width > 26` (the `quine-mccluskey` crate's internal variable-count
///   ceiling; truth tables also grow as 2^width, so large widths are impractical)
/// * if any value has bits set outside the `width`-bit domain
pub fn minimize_patterns(values: &[u64], width: u8) -> Vec<BitPattern> {
    assert!(
        width <= 26,
        "width must be ≤ 26 (quine-mccluskey crate ceiling)"
    );
    if values.is_empty() {
        return Vec::new();
    }
    let field_mask: u64 = if width == 0 { 0 } else { (1u64 << width) - 1 };
    for &v in values {
        assert!(
            v & !field_mask == 0,
            "value {v} has bits outside the {width}-bit domain"
        );
    }

    // Deduplicate.
    let mut minterms: Vec<u64> = values.to_vec();
    minterms.sort_unstable();
    minterms.dedup();

    let domain_size: u64 = if width == 0 { 1 } else { 1u64 << width };

    // Trivial short-circuits.
    if minterms.len() as u64 == domain_size {
        return vec![BitPattern::any(width)];
    }
    if minterms.len() == 1 {
        return vec![BitPattern::exact(minterms[0], width)];
    }

    // Build variable names: variables[k] corresponds to bit position (width - 1 - k),
    // since QM's `to_variables` iterates MSB-first and emits `variable_names[variable_count - i - 1]`
    // for bit position `i`. Naming them "b<bitpos>" lets us recover the bit position from the name.
    let variable_names: Vec<String> = (0..width).rev().map(|b| format!("b{b}")).collect();

    let minterms_u32: Vec<u32> = minterms.iter().map(|&v| v as u32).collect();

    let solutions = quine_mccluskey::minimize_minterms(
        &variable_names,
        &minterms_u32,
        &[], // no don't-cares: cover is exact
        false,
        None,
    )
    .expect("quine-mccluskey minimize_minterms failed");

    // All returned solutions are equally minimal; pick the first.
    let solution = solutions
        .into_iter()
        .next()
        .expect("QM returned no solutions");

    match solution {
        quine_mccluskey::Solution::One => vec![BitPattern::any(width)],
        quine_mccluskey::Solution::Zero => Vec::new(),
        quine_mccluskey::Solution::SOP(implicants) => implicants
            .into_iter()
            .map(|vars| implicant_to_bitpattern(&vars))
            .collect(),
        quine_mccluskey::Solution::POS(_) => {
            unreachable!("minimize_minterms always returns SOP form")
        }
    }
}

/// Decode one SOP implicant (a list of `Variable`s) into a [`BitPattern`].
///
/// Each variable's `name` is of the form "b<bit_position>", and `is_negated`
/// tells us whether the bit is pinned to 0 (negated) or 1 (not negated).
/// Variables not appearing in the implicant are don't-cares (free bits).
fn implicant_to_bitpattern(variables: &[quine_mccluskey::Variable]) -> BitPattern {
    let mut fixed: u64 = 0;
    let mut mask: u64 = 0;
    for var in variables {
        let bit_pos: u32 = var
            .name
            .strip_prefix('b')
            .and_then(|s| s.parse().ok())
            .expect("variable name should be of the form b<N>");
        mask |= 1u64 << bit_pos;
        if !var.is_negated {
            fixed |= 1u64 << bit_pos;
        }
    }
    BitPattern::new(fixed, mask)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_yields_empty_result() {
        let ps = minimize_patterns(&[], 4);
        assert!(ps.is_empty());
    }

    #[test]
    fn single_value_yields_one_exact_pattern() {
        let ps = minimize_patterns(&[5], 4);
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0], BitPattern::exact(5, 4));
    }

    #[test]
    fn full_domain_yields_any_pattern() {
        let all: Vec<u64> = (0..16).collect();
        let ps = minimize_patterns(&all, 4);
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0], BitPattern::any(4));
    }

    #[test]
    fn adjacent_pair_minimizes_to_one_pattern() {
        // {0, 1} on 1-bit: just any(1).
        let ps = minimize_patterns(&[0, 1], 1);
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0], BitPattern::any(1));

        // {2, 3} on 2-bit: pattern with bit 1 pinned to 1, bit 0 free.
        let ps = minimize_patterns(&[2, 3], 2);
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0], BitPattern::new(0b10, 0b10));
    }

    #[test]
    fn non_adjacent_pair_stays_two_patterns() {
        let ps = minimize_patterns(&[0, 3], 2);
        assert_eq!(ps.len(), 2);
        for v in 0..4u64 {
            let c = ps.iter().filter(|p| p.matches(v)).count();
            let expected = if v == 0 || v == 3 { 1 } else { 0 };
            assert_eq!(c, expected);
        }
    }

    #[test]
    fn duplicates_dedupe_internally() {
        let ps1 = minimize_patterns(&[2, 3], 2);
        let ps2 = minimize_patterns(&[2, 3, 3, 2, 3], 2);
        assert_eq!(ps1, ps2);
    }

    #[test]
    fn cover_is_exact_for_mixed_set() {
        // {1, 3, 5, 7} on 3 bits = odd values = bit 0 pinned to 1.
        let ps = minimize_patterns(&[1, 3, 5, 7], 3);
        for v in 0..8u64 {
            let c = ps.iter().filter(|p| p.matches(v)).count();
            let expected = if v & 1 == 1 { 1 } else { 0 };
            assert_eq!(c, expected, "v={v}: matched {c}, expected {expected}");
        }
    }

    #[test]
    #[should_panic(expected = "width must be ≤ 26")]
    fn panics_on_width_27() {
        let _ = minimize_patterns(&[0], 27);
    }

    #[test]
    #[should_panic(expected = "bits outside")]
    fn panics_on_out_of_domain_value() {
        let _ = minimize_patterns(&[8], 3);
    }
}
