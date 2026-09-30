//! Human-readable labels for archetype indices.

/// Map a 6-bit archetype to a descriptive 4-trait label.
/// Combines the 4 OCEA binary axes (Pragmatic/Curious ×
/// Spontaneous/Disciplined × Reserved/Outgoing × Critical/Agreeable)
/// with the 4-level N band (Calm / Steady / Sensitive / Anxious).
/// Index = ((N>>4)&3) * 16 + (archetype & 0xF). 64 distinct labels.
pub fn archetype_label(archetype: u8) -> &'static str {
    const LABELS: [&str; 64] = [
        // N band 0 (Calm)
        "Calm Pragmatic Reserved Critic", "Calm Curious Reserved Critic",
        "Calm Pragmatic Disciplined Critic", "Calm Curious Disciplined Critic",
        "Calm Pragmatic Reserved Ally", "Calm Curious Reserved Ally",
        "Calm Pragmatic Disciplined Ally", "Calm Curious Disciplined Ally",
        "Calm Pragmatic Outgoing Critic", "Calm Curious Outgoing Critic",
        "Calm Pragmatic Driven Critic", "Calm Curious Driven Critic",
        "Calm Pragmatic Outgoing Ally", "Calm Curious Outgoing Ally",
        "Calm Pragmatic Driven Ally", "Calm Curious Driven Ally",
        // N band 1 (Steady)
        "Steady Pragmatic Reserved Critic", "Steady Curious Reserved Critic",
        "Steady Pragmatic Disciplined Critic", "Steady Curious Disciplined Critic",
        "Steady Pragmatic Reserved Ally", "Steady Curious Reserved Ally",
        "Steady Pragmatic Disciplined Ally", "Steady Curious Disciplined Ally",
        "Steady Pragmatic Outgoing Critic", "Steady Curious Outgoing Critic",
        "Steady Pragmatic Driven Critic", "Steady Curious Driven Critic",
        "Steady Pragmatic Outgoing Ally", "Steady Curious Outgoing Ally",
        "Steady Pragmatic Driven Ally", "Steady Curious Driven Ally",
        // N band 2 (Sensitive)
        "Sensitive Pragmatic Reserved Critic", "Sensitive Curious Reserved Critic",
        "Sensitive Pragmatic Disciplined Critic", "Sensitive Curious Disciplined Critic",
        "Sensitive Pragmatic Reserved Ally", "Sensitive Curious Reserved Ally",
        "Sensitive Pragmatic Disciplined Ally", "Sensitive Curious Disciplined Ally",
        "Sensitive Pragmatic Outgoing Critic", "Sensitive Curious Outgoing Critic",
        "Sensitive Pragmatic Driven Critic", "Sensitive Curious Driven Critic",
        "Sensitive Pragmatic Outgoing Ally", "Sensitive Curious Outgoing Ally",
        "Sensitive Pragmatic Driven Ally", "Sensitive Curious Driven Ally",
        // N band 3 (Anxious)
        "Anxious Pragmatic Reserved Critic", "Anxious Curious Reserved Critic",
        "Anxious Pragmatic Disciplined Critic", "Anxious Curious Disciplined Critic",
        "Anxious Pragmatic Reserved Ally", "Anxious Curious Reserved Ally",
        "Anxious Pragmatic Disciplined Ally", "Anxious Curious Disciplined Ally",
        "Anxious Pragmatic Outgoing Critic", "Anxious Curious Outgoing Critic",
        "Anxious Pragmatic Driven Critic", "Anxious Curious Driven Critic",
        "Anxious Pragmatic Outgoing Ally", "Anxious Curious Outgoing Ally",
        "Anxious Pragmatic Driven Ally", "Anxious Curious Driven Ally",
    ];
    LABELS[(archetype & 0x3F) as usize]
}
