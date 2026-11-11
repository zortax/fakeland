//! A plain text table with aligned columns.

/// A text table. The first row is the header; numeric-looking cells align right.
#[derive(Clone, Debug, Default)]
pub struct Table {
    rows: Vec<Vec<String>>,
}

impl Table {
    /// Builds a table with a header row.
    pub fn new<S: Into<String>>(header: impl IntoIterator<Item = S>) -> Self {
        Self {
            rows: vec![header.into_iter().map(Into::into).collect()],
        }
    }

    /// Adds a row.
    pub fn row<S: Into<String>>(&mut self, cells: impl IntoIterator<Item = S>) {
        self.rows.push(cells.into_iter().map(Into::into).collect());
    }

    /// Returns the count of rows below the header.
    pub fn len(&self) -> usize {
        self.rows.len() - 1
    }

    /// Returns true without rows below the header.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Renders the table with two spaces between columns and a rule below the header.
    pub fn render(&self) -> String {
        let cols = self.rows.iter().map(Vec::len).max().unwrap_or(0);
        let mut widths = vec![0; cols];
        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                widths[i] = widths[i].max(cell.chars().count());
            }
        }
        let mut out = String::new();
        for (r, row) in self.rows.iter().enumerate() {
            let mut line = String::new();
            for (i, width) in widths.iter().enumerate() {
                let cell = row.get(i).map(String::as_str).unwrap_or("");
                let pad = width - cell.chars().count();
                if i > 0 {
                    line.push_str("  ");
                }
                if r > 0 && is_numeric(cell) {
                    line.push_str(&" ".repeat(pad));
                    line.push_str(cell);
                } else {
                    line.push_str(cell);
                    line.push_str(&" ".repeat(pad));
                }
            }
            out.push_str(line.trim_end());
            out.push('\n');
            if r == 0 {
                let total = widths.iter().sum::<usize>() + 2 * cols.saturating_sub(1);
                out.push_str(&"-".repeat(total));
                out.push('\n');
            }
        }
        out
    }
}

fn is_numeric(cell: &str) -> bool {
    let c = cell.trim_start_matches(['+', '-']);
    !c.is_empty() && c.chars().next().is_some_and(|ch| ch.is_ascii_digit())
}

/// Formats a value with a precision that fits its size.
pub fn num(v: f64) -> String {
    if !v.is_finite() {
        return v.to_string();
    }
    let a = v.abs();
    if a >= 1000.0 {
        format!("{v:.0}")
    } else if a >= 100.0 {
        format!("{v:.1}")
    } else if a >= 1.0 || a == 0.0 {
        format!("{v:.2}")
    } else {
        format!("{v:.3}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligns_columns() {
        let mut t = Table::new(["name", "value"]);
        t.row(["fps", "60.00"]);
        t.row(["jank_count", "3"]);
        let text = t.render();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "name        value");
        assert_eq!(lines[1], "-----------------");
        assert_eq!(lines[2], "fps         60.00");
        assert_eq!(lines[3], "jank_count      3");
        assert_eq!(t.len(), 2);
    }

    #[test]
    fn number_precision() {
        assert_eq!(num(1234.56), "1235");
        assert_eq!(num(123.456), "123.5");
        assert_eq!(num(16.6667), "16.67");
        assert_eq!(num(0.01234), "0.012");
        assert_eq!(num(0.0), "0.00");
    }
}
