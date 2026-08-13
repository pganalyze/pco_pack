use super::*;

/// Lazily-decompressed reader for derived struct fields.
#[derive(Default, Clone)]
pub struct LazyReader<T: PcoSerde>
where
    T::Reader: Clone + Default,
{
    compressed: Arc<[u8]>,
    decompressed: Option<T::Reader>,
    float_round: u32,
    time_round: chrono::Duration,
}

impl<T: PcoSerde> LazyReader<T>
where
    T::Reader: Clone + Default,
{
    pub fn new(compressed: Vec<u8>, float_round: u32, time_round: chrono::Duration) -> Self {
        Self { compressed: compressed.into(), decompressed: None, float_round, time_round }
    }

    /// Decompress and return a reference to the reader, initializing on first call.
    /// If compressed data is empty, returns a default (empty) reader instead.
    #[inline]
    fn ensure_decompressed(&mut self) -> anyhow::Result<&mut T::Reader> {
        if self.decompressed.is_none() {
            let reader = if !self.compressed.is_empty() {
                let mut cursor = Cursor::new(&self.compressed[..]);
                T::read(&mut cursor, self.float_round, self.time_round).context("LazyReader decompression failed")?
            } else {
                T::Reader::default()
            };
            self.decompressed = Some(reader);
        }
        Ok(self.decompressed.as_mut().expect("decompressed already initialized"))
    }

    #[inline]
    pub fn row_count(&mut self) -> anyhow::Result<usize> {
        let reader = self.ensure_decompressed()?;
        Ok(T::validate_bounds(reader)?.unwrap_or(0))
    }
}

impl<T: PcoSerde + PcoFilter + Default> LazyReader<T>
where
    T::Reader: Default,
{
    #[inline]
    pub fn pop_inner(&mut self, index: usize) -> anyhow::Result<T> {
        let reader = self.ensure_decompressed()?;
        match T::get(reader, index) {
            Ok(Some(v)) => Ok(v),
            Ok(None) | Err(_) => Ok(T::default()),
        }
    }

    #[inline]
    pub fn filter_in_group(&mut self, filter: &Filter, matches: &mut FilterMask) -> anyhow::Result<()> {
        T::filter_bulk(self.ensure_decompressed()?, 0, filter, matches)?;
        Ok(())
    }

    #[inline]
    pub fn filter_nested(&mut self, path: &[usize], filter: &Filter, matches: &mut FilterMask) -> anyhow::Result<()> {
        if path.is_empty() {
            self.filter_in_group(filter, matches)?;
            return Ok(());
        }
        T::filter_nested(self.ensure_decompressed()?, path, filter, matches)?;
        Ok(())
    }
}
