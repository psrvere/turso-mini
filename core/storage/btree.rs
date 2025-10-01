/// The B-Tree page header is 12 bytes for interior pages and 8 bytes for leaf pages.
///
/// +--------+-----------------+-----------------+-----------------+--------+----- ..... ----+
/// | Page   | First Freeblock | Cell Count      | Cell Content    | Frag.  | Right-most     |
/// | Type   | Offset          |                 | Area Start      | Bytes  | pointer        |
/// +--------+-----------------+-----------------+-----------------+--------+----- ..... ----+
///     0        1        2        3        4        5        6        7        8       11
///
/// B-Tree Page: https://www.sqlite.org/fileformat.html
pub mod offset {
    pub const BTREE_PAGE_TYPE: usize = 0;
    pub const BTREE_FIRST_FREEBLOCK: usize = 1;
    pub const BTREE_RIGHTMOST_PTR: usize = 8;
}