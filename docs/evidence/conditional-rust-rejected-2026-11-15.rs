pub fn bounded_chunk_end(text: &str, offset: usize, limit: usize) -> Option<usize> {
    if limit == 0 || offset > text.len() {
        return None;
    }

    let end_offset = offset.saturating_add(limit);
    if end_offset > text.len() {
        return None;
    }

    let slice = &text[offset..end_offset];
    if slice.is_empty() {
        return Some(offset);
    }

    let last_char_boundary = slice.char_indices().last().map(|(i, _)| i + offset);
    if let Some(boundary) = last_char_boundary {
        if boundary <= text.len() {
            Some(boundary)
        } else {
            None
        }
    } else {
        None
    }
}