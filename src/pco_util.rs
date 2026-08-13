//! Utilities for reading/writing pco-compressed data.
use super::*;
use pco::data_types::{Number, NumberType};
use pco::standalone::{DecompressorItem, FileDecompressor};

/// Default pco chunk config with 8-bit compression enabled.
pub(crate) fn config() -> pco::ChunkConfig {
    pco::ChunkConfig::default().with_enable_8_bit(true)
}

/// Decompress a pco-encoded block from a cursor, advancing the cursor past the consumed bytes.
/// Uses the pco FileDecompressor to determine exactly how many bytes were consumed.
pub(crate) fn decompress<T: Default + Clone + Number>(src: &mut Cursor<&[u8]>) -> Result<Vec<T>> {
    let buf = src.get_ref();
    let pos = src.position() as usize;
    if pos >= buf.len() {
        return Ok(Vec::new());
    }
    let remainder = &buf[pos..];
    let start_remainder_len = remainder.len();
    let (file_decompressor, next_remainder) = FileDecompressor::new(remainder)?;
    let mut remainder = next_remainder;
    let mut all_values = Vec::new();
    let mut incomplete_batch_buffer = vec![T::default(); 256];
    loop {
        match file_decompressor.chunk_decompressor(remainder)? {
            DecompressorItem::Chunk(mut chunk_decompressor) => {
                let n = chunk_decompressor.n();
                // read() requires dst length to be a multiple of 256 or at least the remaining count
                let full_batch_len = (n / 256) * 256;
                let mut chunk_values = vec![T::default(); n];
                let progress = chunk_decompressor.read(&mut chunk_values[..full_batch_len])?;
                // Handle remaining values via incomplete batch buffer
                let remaining = n - progress.n_processed;
                if remaining > 0 {
                    let new_progress = chunk_decompressor.read(&mut incomplete_batch_buffer)?;
                    let to_copy = remaining.min(new_progress.n_processed);
                    chunk_values[progress.n_processed..progress.n_processed + to_copy]
                        .copy_from_slice(&incomplete_batch_buffer[..to_copy]);
                }
                all_values.extend(chunk_values);
                remainder = chunk_decompressor.into_src();
            }
            DecompressorItem::EndOfData(final_remainder) => {
                remainder = final_remainder;
                break;
            }
        }
    }
    let bytes_consumed = start_remainder_len - remainder.len();
    src.set_position((pos + bytes_consumed) as u64);
    Ok(all_values)
}

/// Decompress numbers with type coercion support.
/// Detects the actual stored type and coerces to the target float type.
pub(crate) fn decompress_as<T: CoercibleNumber + Number>(src: &mut Cursor<&[u8]>) -> Result<Vec<T>> {
    let buf = src.get_ref();
    let pos = src.position() as usize;
    if pos >= buf.len() {
        return Ok(Vec::new());
    }
    let remainder = &buf[pos..];
    let (file_decompressor, next_remainder) = FileDecompressor::new(remainder)?;
    // Detect the actual stored number type
    let actual_type = match file_decompressor.peek_number_type_or_termination(next_remainder)? {
        Some(t) => t,
        None => return Ok(Vec::new()),
    };
    // Fast path: if stored type matches target type, decompress directly without per-value coercion.
    if actual_type == NumberType::new::<T>() {
        return decompress::<T>(src);
    }
    // Slow path: stored type differs from target, need to decompress and coerce.
    match actual_type {
        NumberType::U8 => Ok(decompress::<u8>(src)?.into_iter().map(|v| T::coerce_from_int(v as i64)).collect()),
        NumberType::U16 => Ok(decompress::<u16>(src)?.into_iter().map(|v| T::coerce_from_int(v as i64)).collect()),
        NumberType::U32 => Ok(decompress::<u32>(src)?.into_iter().map(|v| T::coerce_from_int(v as i64)).collect()),
        NumberType::U64 => Ok(decompress::<u64>(src)?.into_iter().map(|v| T::coerce_from_u64(v)).collect()),
        NumberType::I8 => Ok(decompress::<i8>(src)?.into_iter().map(|v| T::coerce_from_int(v as i64)).collect()),
        NumberType::I16 => Ok(decompress::<i16>(src)?.into_iter().map(|v| T::coerce_from_int(v as i64)).collect()),
        NumberType::I32 => Ok(decompress::<i32>(src)?.into_iter().map(|v| T::coerce_from_int(v as i64)).collect()),
        NumberType::I64 => Ok(decompress::<i64>(src)?.into_iter().map(|v| T::coerce_from_int(v)).collect()),
        NumberType::F16 => {
            Ok(decompress::<half::f16>(src)?.into_iter().map(|v| T::coerce_from_float(v.to_f64())).collect())
        }
        NumberType::F32 => Ok(decompress::<f32>(src)?.into_iter().map(|v| T::coerce_from_float(v as f64)).collect()),
        NumberType::F64 => Ok(decompress::<f64>(src)?.into_iter().map(|v| T::coerce_from_float(v)).collect()),
        _ => Err(anyhow::anyhow!("unsupported number type in pco header")),
    }
}
