use std::collections::BTreeMap;

use crate::utils::MapExt as _;

#[test]
fn test_keep_last() {
    let mut map = BTreeMap::new();
    let expected = 4;
    for i in 0..10 {
        let v = i * 2 + 1;
        map.insert(v, v);
    }
    for i in 0..10 {
        let v = i * 2;
        map.insert(v, v);
    }
    map.keep_last(expected);
    assert_eq!(
        map.len(),
        expected,
        "map's length is incorrect, expect {} but actual is {}",
        expected,
        map.len()
    );
    for v in (20 - expected)..20 {
        assert!(map.contains_key(&v), "map should contain {} but not", v);
    }
}
