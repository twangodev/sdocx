use std::collections::BTreeSet;

/// An invalid one-based page selection, such as `1-3,5`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PageSelectionError {
    #[error("Enter a page number or range, such as 1-3, 5.")]
    Empty,
    #[error("Invalid page selection '{0}'. Use page numbers and ranges, such as 1-3, 5.")]
    InvalidExpression(String),
    #[error("Page {page} is outside this document's range (1-{page_count}).")]
    OutOfBounds { page: usize, page_count: usize },
    #[error("Range {start}-{end} is reversed. Put the smaller page number first.")]
    Reversed { start: usize, end: usize },
}

/// Resolve one-based page numbers and inclusive ranges into sorted, unique
/// zero-based visible-page indices. Accepts hyphens or en dashes and whitespace.
/// Bounds are checked before a range is expanded.
pub fn parse_page_selection(
    input: &str,
    page_count: usize,
) -> Result<Vec<usize>, PageSelectionError> {
    if input.trim().is_empty() {
        return Err(PageSelectionError::Empty);
    }
    let mut selected = BTreeSet::new();
    for expression in input.split(',') {
        let expression = expression.trim();
        let invalid = || PageSelectionError::InvalidExpression(expression.into());
        let number = |value: &str| {
            let value = value.trim();
            if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(invalid());
            }
            let page = value.parse::<usize>().map_err(|_| invalid())?;
            if page == 0 || page > page_count {
                return Err(PageSelectionError::OutOfBounds { page, page_count });
            }
            Ok(page)
        };
        let mut endpoints = expression.split(['-', '–']);
        let start = number(endpoints.next().unwrap())?;
        let end = endpoints.next().map(number).transpose()?.unwrap_or(start);
        if endpoints.next().is_some() {
            return Err(invalid());
        }
        if start > end {
            return Err(PageSelectionError::Reversed { start, end });
        }
        selected.extend((start - 1)..end);
    }
    Ok(selected.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_ranges_and_duplicates_into_document_order() {
        assert_eq!(
            parse_page_selection(" 5, 1–3, 2 - 4, 1 ", 5).unwrap(),
            vec![0, 1, 2, 3, 4]
        );
        assert_eq!(parse_page_selection("3,1", 5).unwrap(), vec![0, 2]);
        assert_eq!(parse_page_selection("1", 1).unwrap(), vec![0]);
    }

    #[test]
    fn rejects_invalid_or_unbounded_selections() {
        for value in [
            "",
            " ",
            "0",
            "6",
            "1-6",
            "3-1",
            "1,",
            ",1",
            "1,,2",
            "1-2-3",
            "-1",
            "1-",
            "+1",
            "1.5",
            "all",
            "1 2",
            "1—3",
            "1-99999999999999999999999999999999999",
        ] {
            assert!(parse_page_selection(value, 5).is_err(), "{value:?}");
        }
        assert!(parse_page_selection("1", 0).is_err());
    }
}
