//! Plain-text tables for terminal output, plus the number formats they use.

/// Column alignment.
#[derive(Clone, Copy)]
pub(crate) enum Align {
    Left,
    Right,
}

/// A table rendered with space-padded columns and a dashed rule under the
/// header. Widths count `char`s, so `µs` pads correctly.
pub(crate) struct Table {
    columns: Vec<(&'static str, Align)>,
    rows: Vec<Vec<String>>,
}

impl Table {
    pub(crate) fn new(columns: &[(&'static str, Align)]) -> Self {
        Self {
            columns: columns.to_vec(),
            rows: Vec::new(),
        }
    }

    pub(crate) fn row(&mut self, cells: Vec<String>) {
        debug_assert_eq!(
            cells.len(),
            self.columns.len(),
            "row width must match header"
        );
        self.rows.push(cells);
    }

    pub(crate) fn render(&self) -> String {
        let mut widths: Vec<usize> = self
            .columns
            .iter()
            .map(|(h, _)| h.chars().count())
            .collect();
        for row in &self.rows {
            for (width, cell) in widths.iter_mut().zip(row) {
                *width = (*width).max(cell.chars().count());
            }
        }
        let header: Vec<String> = self.columns.iter().map(|(h, _)| (*h).to_owned()).collect();
        let rule: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
        let mut out = String::new();
        for cells in std::iter::once(&header).chain([&rule]).chain(&self.rows) {
            let line: Vec<String> = cells
                .iter()
                .zip(&widths)
                .zip(&self.columns)
                .map(|((cell, &width), (_, align))| match align {
                    Align::Left => format!("{cell:<width$}"),
                    Align::Right => format!("{cell:>width$}"),
                })
                .collect();
            out.push_str(line.join("  ").trim_end());
            out.push('\n');
        }
        out
    }
}

/// A duration in nanoseconds, in the largest unit that keeps it ≥ 1.
pub fn format_ns(ns: f64) -> String {
    if ns < 1e3 {
        format!("{ns:.1} ns")
    } else if ns < 1e6 {
        format!("{:.2} µs", ns / 1e3)
    } else if ns < 1e9 {
        format!("{:.2} ms", ns / 1e6)
    } else {
        format!("{:.2} s", ns / 1e9)
    }
}

/// A rate per second with an SI suffix, e.g. `182.4M/s`.
pub(crate) fn format_rate(per_sec: f64) -> String {
    if per_sec >= 1e9 {
        format!("{:.1}G/s", per_sec / 1e9)
    } else if per_sec >= 1e6 {
        format!("{:.1}M/s", per_sec / 1e6)
    } else if per_sec >= 1e3 {
        format!("{:.1}k/s", per_sec / 1e3)
    } else {
        format!("{per_sec:.1}/s")
    }
}

/// An optional value with `decimals` decimals, `-` when missing.
pub(crate) fn format_opt(value: Option<f64>, decimals: usize) -> String {
    value.map_or_else(|| "-".to_owned(), |v| format!("{v:.decimals$}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_pick_a_readable_unit() {
        assert_eq!(format_ns(4.31), "4.3 ns");
        assert_eq!(format_ns(1_500.0), "1.50 µs");
        assert_eq!(format_ns(20_000_000.0), "20.00 ms");
        assert_eq!(format_ns(2.5e9), "2.50 s");
    }

    #[test]
    fn columns_pad_by_characters() {
        let mut table = Table::new(&[("name", Align::Left), ("p99", Align::Right)]);
        table.row(vec!["a".to_owned(), "1.50 µs".to_owned()]);
        table.row(vec!["longer".to_owned(), "9.0 ns".to_owned()]);
        let rendered = table.render();
        let lines: Vec<&str> = rendered.lines().collect();
        assert_eq!(lines[0], "name        p99");
        assert_eq!(lines[1], "------  -------");
        assert_eq!(lines[2], "a       1.50 µs");
        assert_eq!(lines[3], "longer   9.0 ns");
    }
}
