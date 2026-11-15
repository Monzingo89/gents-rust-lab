pub fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::from(*id);
    bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(data);
    if data.len() % 2 == 1 {
        bytes.push(0);
    }
    bytes
}
pub fn riff(chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut body = Vec::from(*b"WAVE");
    for c in chunks {
        body.extend_from_slice(c);
    }
    let mut bytes = Vec::from(*b"RIFF");
    bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&body);
    bytes
}
pub fn format(channels: u16, rate: u32) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&channels.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes());
    b.extend_from_slice(&(channels * 2).to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b
}
pub fn wav(channels: u16, rate: u32, samples: &[i16]) -> Vec<u8> {
    let data = samples
        .iter()
        .flat_map(|s| s.to_le_bytes())
        .collect::<Vec<_>>();
    riff(&[
        chunk(b"fmt ", &format(channels, rate)),
        chunk(b"data", &data),
    ])
}
