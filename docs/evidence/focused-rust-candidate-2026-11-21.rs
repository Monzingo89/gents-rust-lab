pub fn bounded_chunk_end(text: &str, offset: usize, limit: usize) -> Option<usize> {
    if limit == 0 {
        return None;
    }

    let len = text.len();
    if offset > len || !text.is_char_boundary(offset) {
        return None;
    }

    let target = offset.saturating_add(limit).min(len);
    if target == len {
        return Some(len);
    }

    let mut end = target;
    while !text.is_char_boundary(end) {
        if end == 0 {
            return None;
        }
        end -= 1;
    }

    if end == offset {
        let next = text
            .get(offset..)
            .and_then(|rest| rest.char_indices().next())
            .map(|(_, c)| offset + c.len_utf8())
            .unwrap_or(len);
        if next > target {
            return None;
        }
    }

    Some(end)
}