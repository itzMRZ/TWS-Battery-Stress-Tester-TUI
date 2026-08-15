//! Bundled reference loop: quiet band-limited noise, 2 s, 48 kHz stereo 16-bit.

use std::io::Write;
use std::path::Path;

const RATE: u32 = 48_000;
const SECS: u32 = 2;
const CH: u16 = 2;

pub fn write_wav(path: &Path) -> std::io::Result<()> {
    let n = (RATE * SECS) as usize;
    let mut pcm = Vec::with_capacity(n * CH as usize * 2);
    let mut seed: u32 = 0xC0FFEE;
    let mut lp = 0.0f32;
    for i in 0..n {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let white = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
        lp = lp * 0.95 + white * 0.05;
        let tone = (2.0 * std::f32::consts::PI * 220.0 * i as f32 / RATE as f32).sin() * 0.08;
        let s = (lp * 0.22 + tone).clamp(-1.0, 1.0);
        let q = (s * 8000.0) as i16;
        pcm.extend_from_slice(&q.to_le_bytes());
        pcm.extend_from_slice(&q.to_le_bytes());
    }
    let data_len = pcm.len() as u32;
    let mut f = std::fs::File::create(path)?;
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + data_len).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?;
    f.write_all(&CH.to_le_bytes())?;
    f.write_all(&RATE.to_le_bytes())?;
    f.write_all(&(RATE * u32::from(CH) * 2).to_le_bytes())?;
    f.write_all(&(CH * 2).to_le_bytes())?;
    f.write_all(&16u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&data_len.to_le_bytes())?;
    f.write_all(&pcm)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_is_sane() {
        let dir = std::env::temp_dir().join("tws-tester-wav-test");
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("r.wav");
        write_wav(&p).unwrap();
        let b = std::fs::read(&p).unwrap();
        assert_eq!(&b[0..4], b"RIFF");
        assert_eq!(&b[8..12], b"WAVE");
        assert!(b.len() > 44);
    }
}
