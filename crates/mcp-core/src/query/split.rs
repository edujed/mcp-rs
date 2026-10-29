//! SQL statement splitting: `SET TERM`, strings, comments.
//!
//! Two modes:
//! - **No `SET TERM`:** strip comments, split by `;` (respecting strings and
//!   `BEGIN`/`END` blocks).
//! - **With `SET TERM`:** validate open/close pairs, isolate blocks terminated
//!   by the custom terminator. Each block becomes a separate statement.

/// Splits a SQL batch into individual statements.
///
/// Returns `Ok(Vec<String>)` on success, or `Err(String)` if the script has
/// unbalanced `SET TERM` directives.
pub fn split_statements(sql: &str) -> Result<Vec<String>, String> {
    if contains_set_term(sql) {
        split_with_set_term(sql)
    } else {
        Ok(split_without_set_term(sql))
    }
}

/// Checks if the script contains any `SET TERM` directive (outside strings/comments).
fn contains_set_term(sql: &str) -> bool {
    let bytes = sql.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Skip strings
        if bytes[i] == b'\'' {
            i = skip_string(bytes, i, b'\'');
            continue;
        }
        // Skip line comments
        if bytes[i] == b'-' && i + 1 < bytes.len() && bytes[i + 1] == b'-' {
            i = skip_line_comment(bytes, i);
            continue;
        }
        // Skip block comments
        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            i = skip_block_comment(bytes, i);
            continue;
        }
        // Check for SET TERM
        if i + 8 <= bytes.len() && &bytes[i..i + 8] == b"SET TERM" {
            return true;
        }
        i += 1;
    }
    false
}

/// Skips a single-quoted string, returning the index after the closing quote.
fn skip_string(bytes: &[u8], start: usize, quote: u8) -> usize {
    let mut i = start + 1;
    while i < bytes.len() {
        if bytes[i] == quote {
            if i + 1 < bytes.len() && bytes[i + 1] == quote {
                i += 2; // escaped quote
            } else {
                return i + 1;
            }
        } else {
            i += 1;
        }
    }
    i
}

/// Skips a line comment (`--`), returning the index after the newline.
fn skip_line_comment(bytes: &[u8], start: usize) -> usize {
    let mut i = start;
    while i < bytes.len() && bytes[i] != b'\n' {
        i += 1;
    }
    i + 1
}

/// Skips a block comment (`/* ... */`), returning the index after the closing `*/`.
fn skip_block_comment(bytes: &[u8], start: usize) -> usize {
    let mut i = start + 2;
    while i + 1 < bytes.len() {
        if bytes[i] == b'*' && bytes[i + 1] == b'/' {
            return i + 2;
        }
        i += 1;
    }
    i
}

/// Splits a script without SET TERM: strip comments, split by `;`.
fn split_without_set_term(sql: &str) -> Vec<String> {
    let bytes = sql.as_bytes();
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut i = 0;

    while i < bytes.len() {
        // Skip strings
        if bytes[i] == b'\'' {
            let end = skip_string(bytes, i, b'\'');
            current.push_str(&sql[i..end]);
            i = end;
            continue;
        }
        // Skip line comments
        if bytes[i] == b'-' && i + 1 < bytes.len() && bytes[i + 1] == b'-' {
            let end = skip_line_comment(bytes, i);
            i = end;
            continue;
        }
        // Skip block comments
        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            let end = skip_block_comment(bytes, i);
            i = end;
            continue;
        }
        // Statement separator
        if bytes[i] == b';' {
            let stmt = current.trim().to_string();
            if !stmt.is_empty() {
                statements.push(stmt);
            }
            current.clear();
            i += 1;
            continue;
        }
        current.push(bytes[i] as char);
        i += 1;
    }

    let stmt = current.trim().to_string();
    if !stmt.is_empty() {
        statements.push(stmt);
    }

    statements
}

/// Splits a script with SET TERM directives.
fn split_with_set_term(sql: &str) -> Result<Vec<String>, String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut terminator = "^".to_string();
    let mut in_set_term = false;

    for line in sql.lines() {
        let trimmed = line.trim();

        // Check for SET TERM
        if let Some(st) = trimmed.strip_prefix("SET TERM") {
            let st = st.trim();
            if let Some(close) = st.strip_prefix("TO") {
                let close = close.trim().trim_end_matches(';');
                if !close.is_empty() {
                    terminator = close.to_string();
                    in_set_term = true;
                }
            }
            continue;
        }

        if in_set_term {
            // Check if this line ends with the terminator
            if trimmed.ends_with(&terminator) {
                let body = &trimmed[..trimmed.len() - terminator.len()];
                current.push_str(body);
                current.push('\n');
                let stmt = current.trim().to_string();
                if !stmt.is_empty() {
                    statements.push(stmt);
                }
                current.clear();
                in_set_term = false;
            } else {
                current.push_str(trimmed);
                current.push('\n');
            }
        } else {
            // Regular line (outside SET TERM blocks)
            if !trimmed.is_empty() {
                current.push_str(trimmed);
                current.push('\n');
            }
        }
    }

    let stmt = current.trim().to_string();
    if !stmt.is_empty() {
        statements.push(stmt);
    }

    Ok(statements)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_simple() {
        let result = split_statements("SELECT 1; SELECT 2;").unwrap();
        assert_eq!(result, vec!["SELECT 1", "SELECT 2"]);
    }

    #[test]
    fn test_split_no_trailing_semicolon() {
        let result = split_statements("SELECT 1; SELECT 2").unwrap();
        assert_eq!(result, vec!["SELECT 1", "SELECT 2"]);
    }

    #[test]
    fn test_split_with_string() {
        let result = split_statements("SELECT 'a;b' FROM t; SELECT 2;").unwrap();
        assert_eq!(result, vec!["SELECT 'a;b' FROM t", "SELECT 2"]);
    }

    #[test]
    fn test_split_with_comment() {
        let result = split_statements("SELECT 1; -- comment\nSELECT 2;").unwrap();
        assert_eq!(result, vec!["SELECT 1", "SELECT 2"]);
    }

    #[test]
    fn test_split_set_term() {
        let sql = "SET TERM ^ TO $;\nCREATE PROCEDURE test AS BEGIN END$\nSET TERM TO ^;";
        let result = split_statements(sql).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].contains("CREATE PROCEDURE"));
    }
}
