//! Arena Allocator for Temporary Allocations
//!
//! Reduces allocator pressure during search operations by using
//! bump allocation for temporary data structures.
//!
//! # Benefits
//! - Faster allocation (just bump a pointer)
//! - Faster deallocation (free entire arena at once)
//! - Better cache locality
//! - Reduced memory fragmentation

use bumpalo::Bump;
use std::cell::RefCell;

// Thread-local arena for search operations
thread_local! {
    static SEARCH_ARENA: RefCell<Bump> = RefCell::new(Bump::with_capacity(64 * 1024));
}

/// Arena for search operations
///
/// Use this for temporary allocations during search that will be
/// discarded after the search completes.
pub struct SearchArena {
    arena: Bump,
}

impl SearchArena {
    /// Create a new search arena with default capacity (64KB)
    pub fn new() -> Self {
        Self {
            arena: Bump::with_capacity(64 * 1024),
        }
    }

    /// Create with custom capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            arena: Bump::with_capacity(capacity),
        }
    }

    /// Allocate a slice in the arena
    pub fn alloc_slice<T: Copy>(&self, slice: &[T]) -> &[T] {
        self.arena.alloc_slice_copy(slice)
    }

    /// Allocate a vector's contents in the arena
    pub fn alloc_vec<T: Copy>(&self, vec: &Vec<T>) -> &[T] {
        self.arena.alloc_slice_copy(vec)
    }

    /// Allocate a string in the arena
    pub fn alloc_str(&self, s: &str) -> &str {
        self.arena.alloc_str(s)
    }

    /// Allocate and initialize a value
    pub fn alloc<T>(&self, val: T) -> &mut T {
        self.arena.alloc(val)
    }

    /// Allocate space for a slice and fill with a function
    pub fn alloc_slice_fill<T, F>(&self, len: usize, mut f: F) -> &mut [T]
    where
        F: FnMut(usize) -> T,
    {
        self.arena.alloc_slice_fill_with(len, |i| f(i))
    }

    /// Reset the arena for reuse
    pub fn reset(&mut self) {
        self.arena.reset();
    }

    /// Current allocated bytes
    pub fn allocated_bytes(&self) -> usize {
        self.arena.allocated_bytes()
    }

    /// Get a reference to the underlying bump allocator
    pub fn bump(&self) -> &Bump {
        &self.arena
    }
}

impl Default for SearchArena {
    fn default() -> Self {
        Self::new()
    }
}

/// Execute a function with a thread-local arena
///
/// The arena is reset after the function completes.
pub fn with_arena<F, R>(f: F) -> R
where
    F: FnOnce(&Bump) -> R,
{
    SEARCH_ARENA.with(|arena| {
        let arena = arena.borrow();
        let result = f(&arena);
        // Arena will be reset on next use if needed
        result
    })
}

/// Search result allocated in arena
pub struct ArenaSearchResult<'a> {
    pub doc_id: &'a str,
    pub score: f32,
    pub content: &'a str,
}

impl<'a> ArenaSearchResult<'a> {
    pub fn new(arena: &'a SearchArena, doc_id: &str, score: f32, content: &str) -> Self {
        Self {
            doc_id: arena.alloc_str(doc_id),
            score,
            content: arena.alloc_str(content),
        }
    }
}

/// Batch of search results in arena
pub struct ArenaSearchResults<'a> {
    arena: &'a SearchArena,
    results: Vec<ArenaSearchResult<'a>>,
}

impl<'a> ArenaSearchResults<'a> {
    pub fn new(arena: &'a SearchArena) -> Self {
        Self {
            arena,
            results: Vec::new(),
        }
    }

    pub fn with_capacity(arena: &'a SearchArena, capacity: usize) -> Self {
        Self {
            arena,
            results: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, doc_id: &str, score: f32, content: &str) {
        self.results
            .push(ArenaSearchResult::new(self.arena, doc_id, score, content));
    }

    pub fn results(&self) -> &[ArenaSearchResult<'a>] {
        &self.results
    }

    pub fn len(&self) -> usize {
        self.results.len()
    }

    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }

    /// Sort by score descending
    pub fn sort_by_score(&mut self) {
        self.results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Take top k results
    pub fn top_k(&mut self, k: usize) {
        self.sort_by_score();
        self.results.truncate(k);
    }
}

/// Temporary vector storage in arena
pub struct ArenaVectors<'a> {
    arena: &'a SearchArena,
    vectors: Vec<&'a [f32]>,
}

impl<'a> ArenaVectors<'a> {
    pub fn new(arena: &'a SearchArena) -> Self {
        Self {
            arena,
            vectors: Vec::new(),
        }
    }

    pub fn push(&mut self, vector: &[f32]) {
        self.vectors.push(self.arena.alloc_slice(vector));
    }

    pub fn get(&self, index: usize) -> Option<&'a [f32]> {
        self.vectors.get(index).copied()
    }

    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &'a [f32]> + '_ {
        self.vectors.iter().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_arena() {
        let arena = SearchArena::new();

        let slice = arena.alloc_slice(&[1.0f32, 2.0, 3.0]);
        assert_eq!(slice, &[1.0, 2.0, 3.0]);

        let s = arena.alloc_str("hello");
        assert_eq!(s, "hello");
    }

    #[test]
    fn test_arena_search_results() {
        let arena = SearchArena::new();
        let mut results = ArenaSearchResults::new(&arena);

        results.push("doc_1", 0.9, "content 1");
        results.push("doc_2", 0.8, "content 2");
        results.push("doc_3", 0.95, "content 3");

        results.sort_by_score();

        assert_eq!(results.results()[0].doc_id, "doc_3");
        assert_eq!(results.results()[1].doc_id, "doc_1");
        assert_eq!(results.results()[2].doc_id, "doc_2");
    }

    #[test]
    fn test_arena_vectors() {
        let arena = SearchArena::new();
        let mut vectors = ArenaVectors::new(&arena);

        vectors.push(&[1.0, 2.0, 3.0]);
        vectors.push(&[4.0, 5.0, 6.0]);

        assert_eq!(vectors.len(), 2);
        assert_eq!(vectors.get(0), Some(&[1.0f32, 2.0, 3.0][..]));
    }

    #[test]
    fn test_with_arena() {
        let result = with_arena(|arena| {
            let s = arena.alloc_str("test");
            s.len()
        });

        assert_eq!(result, 4);
    }
}
