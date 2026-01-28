use bytemuck::Zeroable;

pub trait Interpolatable {
    fn interpolate(&mut self, len: usize);
}

impl<Z: Zeroable + Clone> Interpolatable for Vec<Z> {
    fn interpolate(&mut self, len: usize) {
        if self.len() != len {
            self.resize(len, Z::zeroed());
        }
    }
}

pub fn interpolate_zeros(len: usize, data: &mut [&mut dyn Interpolatable]) {
    data.iter_mut().for_each(|i| i.interpolate(len));
}

pub fn extract_data<T, O, F>(indices: &[u32], source: &[T], converter: F) -> Vec<O>
where
    F: Fn(&T) -> O,
{
    indices
        .iter()
        .filter_map(|&idx| source.get(idx as usize).map(&converter))
        .collect()
}

pub fn extend_data<T, O, F>(extendable: &mut Vec<T>, indices: &[u32], source: &[O], converter: F)
where
    F: Fn(&O) -> T,
{
    extendable.extend(extract_data(indices, source, converter));
}
