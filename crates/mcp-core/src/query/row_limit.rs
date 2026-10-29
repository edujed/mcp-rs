//! Row-limit injection: `FIRST {max} SKIP {skip}` clause for SELECT statements.

/// Returns `true` when a `FIRST`/`ROWS` clause can be appended to the
/// statement. Only `SELECT` and `WITH` tolerate a row-limit clause in
/// Firebird; appending it to `EXECUTE` forms is a syntax error.
#[must_use]
pub fn accepts_row_limit(sql: &str) -> bool {
    let trimmed = sql.trim_start();
    let first = trimmed.split_whitespace().next().unwrap_or("");
    matches!(first.to_ascii_uppercase().as_str(), "SELECT" | "WITH")
}

/// Default rows returned by a single query call when the caller does not
/// specify `max_rows`. Kept small so a bare `SELECT *` does not flood the
/// LLM context window.
pub const DEFAULT_ROWS: u32 = 100;

/// Hard cap on rows per query call. Even if the caller asks for more, the
/// injected `FIRST` clause never exceeds this value.
pub const MAX_ROWS: u32 = 1000;

/// Injects a Firebird `FIRST {max} SKIP {skip}` clause into a SELECT
/// statement so the result set is bounded. The clause is inserted
/// immediately after the `SELECT` keyword (or after `WITH ... AS (...)`
/// for CTEs), which is where Firebird's grammar requires it.
///
/// Statements that already contain a row-limiting clause (`FIRST`,
/// `ROWS`, `FETCH FIRST`) are returned unchanged so the user-supplied
/// cap wins.
#[must_use]
pub fn inject_row_limit(sql: &str, max_rows: u32, skip: u32) -> String {
    let capped = max_rows.min(MAX_ROWS);

    // If the statement already has a row-limit clause, don't inject.
    let upper = sql.to_ascii_uppercase();
    if upper.contains(" FIRST ")
        || upper.contains(" FIRST\n")
        || upper.contains(" ROWS ")
        || upper.contains(" FETCH FIRST ")
    {
        return sql.to_string();
    }

    let trimmed = sql.trim_start();
    let upper_trimmed = trimmed.to_ascii_uppercase();

    if upper_trimmed.starts_with("WITH") {
        // For CTEs: insert after the closing paren of the CTE definition.
        // Find the last `)` before the main SELECT.
        if let Some(select_pos) = find_main_select(&upper_trimmed) {
            let before = &trimmed[..select_pos];
            let after = &trimmed[select_pos..];
            let clause = if skip > 0 {
                format!("FIRST {capped} SKIP {skip} ")
            } else {
                format!("FIRST {capped} ")
            };
            return format!("{before}{clause}{after}");
        }
        return sql.to_string();
    }

    if upper_trimmed.starts_with("SELECT") {
        let select_end = 6; // len("SELECT")
        let rest = &trimmed[select_end..];
        let clause = if skip > 0 {
            format!(" FIRST {capped} SKIP {skip}")
        } else {
            format!(" FIRST {capped}")
        };
        return format!("SELECT{clause}{rest}");
    }

    sql.to_string()
}

/// Finds the position of the main SELECT keyword in a CTE statement.
/// The main SELECT is the one that is NOT inside the CTE definition.
fn find_main_select(upper: &str) -> Option<usize> {
    // Count parens to find where the CTE definition ends.
    let mut depth = 0i32;
    let mut in_cte = false;
    for (i, c) in upper.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 && in_cte {
                    // CTE definition ended; the next SELECT is the main one.
                    in_cte = false;
                }
            }
            ' ' if i + 6 <= upper.len() => {
                if &upper[i..i + 6] == "SELECT" {
                    if in_cte {
                        continue;
                    }
                    return Some(i);
                }
                if &upper[i..i + 4] == "WITH" {
                    in_cte = true;
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accepts_row_limit() {
        assert!(accepts_row_limit("SELECT * FROM t"));
        assert!(accepts_row_limit("WITH x AS (SELECT 1) SELECT * FROM x"));
        assert!(!accepts_row_limit("EXECUTE PROCEDURE p()"));
        assert!(!accepts_row_limit("INSERT INTO t VALUES (1)"));
    }

    #[test]
    fn test_inject_simple() {
        let result = inject_row_limit("SELECT * FROM t", 100, 0);
        assert_eq!(result, "SELECT FIRST 100 * FROM t");
    }

    #[test]
    fn test_inject_with_skip() {
        let result = inject_row_limit("SELECT * FROM t", 100, 50);
        assert_eq!(result, "SELECT FIRST 100 SKIP 50 * FROM t");
    }

    #[test]
    fn test_inject_caps_at_max() {
        let result = inject_row_limit("SELECT * FROM t", 5000, 0);
        assert_eq!(result, "SELECT FIRST 1000 * FROM t");
    }

    #[test]
    fn test_inject_existing_first() {
        let sql = "SELECT FIRST 10 * FROM t";
        let result = inject_row_limit(sql, 100, 0);
        assert_eq!(result, sql);
    }

    #[test]
    fn test_inject_non_select() {
        let sql = "INSERT INTO t VALUES (1)";
        let result = inject_row_limit(sql, 100, 0);
        assert_eq!(result, sql);
    }
}
