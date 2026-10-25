use crate::IdPool;

#[test]
fn allocate_starts_at_the_given_id() {
    let mut pool = IdPool::new(1);
    assert_eq!(pool.allocate(), 1);
    assert_eq!(pool.allocate(), 2);
    assert_eq!(pool.allocate(), 3);
}

#[test]
fn release_then_allocate_reuses_the_freed_id() {
    let mut pool = IdPool::new(1);
    let a = pool.allocate();
    let _b = pool.allocate();
    pool.release(a);
    assert_eq!(pool.allocate(), a);
}

#[test]
fn released_ids_are_reused_before_new_ones() {
    let mut pool = IdPool::new(1);
    let a = pool.allocate();
    let b = pool.allocate();
    pool.release(a);
    pool.release(b);
    // Free list is LIFO — the most recently released id comes back first.
    assert_eq!(pool.allocate(), b);
    assert_eq!(pool.allocate(), a);
    assert_eq!(pool.allocate(), 3);
}
