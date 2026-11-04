use foldhash::fast::RandomState;

pub type FastHashMap<K, V> = hashbrown::HashMap<K, V, RandomState>;
pub type FastHashSet<T> = hashbrown::HashSet<T, RandomState>;
pub type FastIndexMap<K, V> = indexmap::IndexMap<K, V, RandomState>;
pub type FastIndexSet<T> = indexmap::IndexSet<T, RandomState>;
