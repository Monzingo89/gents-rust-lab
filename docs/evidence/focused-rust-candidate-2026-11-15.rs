pub fn bounded_chunk_end(text: &str, offset: usize, limit: usize) -> Option<usize> {
    if limit == 0 || offset > text.len() || !text.is_char_boundary(offset) {
        return None;
    }

    let target = if offset >= text.len().saturating_sub(limit) {
        text.len()
    } else {
        offset + limit
    };

    if target == offset {
        return Some(offset);
    }

    let mut end = target;
    while end > offset && !text.is_char_boundary(end) {
        end -= 1;
    }

    if end == offset {
        None
    } else {
        Some(end)
    }
}