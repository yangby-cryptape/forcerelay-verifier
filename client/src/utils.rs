use std::{clone::Clone, cmp::Ord, collections::BTreeMap};

pub(crate) trait MapExt<K, V> {
    /// Shortens the data, keeping the last `len` elements and dropping the rest.
    fn keep_last(&mut self, len: usize);
    /// Returns the last key-value pair in the map.
    /// The key in this pair is the maximum key in the map.
    ///
    /// TODO remove this method after rust v1.66.0 released.
    fn last_key_value(&self) -> Option<(&K, &V)>;
}

impl<K, V> MapExt<K, V> for BTreeMap<K, V>
where
    K: Clone + Ord,
{
    fn keep_last(&mut self, len: usize) {
        if self.len() > len {
            let split_size = self.len() - len;
            let split_key = self.keys().nth(split_size).cloned().expect("checked");
            *self = self.split_off(&split_key);
        }
    }

    fn last_key_value(&self) -> Option<(&K, &V)> {
        self.iter().next_back()
    }
}
