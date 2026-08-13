use super::*;

impl<T: VecPackable> PcoSerde for Vec<T>
where
    <T as VecPackable>::VecWriter: Default,
    <T as VecPackable>::VecReader: Clone,
{
    type Writer = <T as VecPackable>::VecWriter;
    type Reader = <T as VecPackable>::VecReader;

    fn write(data: Vec<Vec<T>>, float_round: u32, time_round: chrono::Duration) -> anyhow::Result<Vec<u8>> {
        let mut writer = <T as VecPackable>::VecWriter::default();
        for v in data {
            T::pack_vec(v, &mut writer);
        }
        let mut out = Vec::new();
        T::write_vec(writer, float_round, time_round, &mut out)?;
        Ok(out)
    }

    fn read(src: &mut Cursor<&[u8]>, float_round: u32, time_round: chrono::Duration) -> anyhow::Result<Self::Reader> {
        T::read_vec(src, float_round, time_round)
    }

    fn validate_bounds(reader: &mut Self::Reader) -> Result<Option<usize>> {
        T::validate_vec_bounds(reader)
    }

    fn get(reader: &mut Self::Reader, index: usize) -> Result<Option<Self>> {
        T::get_vec(reader, index)
    }
}

impl<T: VecPackable + PcoFilter> PcoFilter for Vec<T>
where
    <T as VecPackable>::VecWriter: Default,
    <T as VecPackable>::VecReader: Clone,
{
    fn filter_bulk(reader: &mut Self::Reader, _field: usize, filter: &Filter, matches: &mut FilterMask) -> Result<()> {
        T::filter_vec(reader, filter, matches)?;
        Ok(())
    }

    fn filter_match(value: &Self, filter: &Filter) -> bool {
        value.iter().any(|elem| T::filter_vec_match(elem, filter))
    }

    fn filter_nested(
        reader: &mut Self::Reader, path: &[usize], filter: &Filter, matches: &mut FilterMask,
    ) -> Result<()> {
        T::filter_vec_nested(reader, path, filter, matches)?;
        Ok(())
    }

    fn resolve_filter(path: &str, json: &serde_json::Value) -> ::anyhow::Result<ResolvedFilter> {
        T::resolve_filter(path, json)
    }
}
