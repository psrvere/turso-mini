/* Record Format:
Documentation: https://sqlite.org/fileformat2.html#serialtype

A record in sqlite is how the database stores data in B-Trees
A record is made of two parts - header and body
Header store column type information in following format:

┌────┬────┬────┬────┬────┬────┬────┬────┬────┬────
│ HS │ S1 │ S2 │ S3 │ S4 │ ... │ SN │ V1 │ V2 │ ...
└────┴────┴────┴────┴────┴────┴────┴────┴────┴────
│<──── Header ──────────────>│<──── Body ────────>

Where:
HS = Header Size (varint, includes itself)
S1 = Serial Type for column 1 (varint)
S2 = Serial Type for column 2 (varint)
S3 = Serial Type for column 3 (varint)
*/
pub struct SerialType(u64);

pub enum SerialTypeKind {
    Null,       // 0
    I8,         // 1
    I16,        // 2
    I24,        // 3
    I32,        // 4
    I48,        // 5
    I64,        // 6
    F64,        // 7
    ConstInt0,  // 8
    ConstInt1,  // 9
    Text,       // >=12 and even
    Blob,       // >=13 and odd
}

impl SerialType {
    #[inline(always)]
    pub fn u64_is_valid_serial_type(n: u64) -> bool {
        n != 10 && n!= 11
    }

    const NULL: Self = Self(0);
    const I8: Self = Self(1);
    const I16: Self = Self(2);
    const I24: Self = Self(3);
    const I32: Self = Self(4);
    const I48: Self = Self(5);
    const I64: Self = Self(6);
    const F64: Self = Self(7);
    const CONST_INT0: Self = Self(8);
    const CONST_INT1: Self = Self(9);

    pub fn null() -> Self {
        Self::NULL
    }

    pub fn i8() -> Self {
        Self::I8
    }

    pub fn i16() -> Self {
        Self::I16
    }

    pub fn i24() -> Self {
        Self::I16
    }

    pub fn i32() -> Self {
        Self::I32
    }

    pub fn i48() -> Self {
        Self::I48
    }

    pub fn i64() -> Self {
        Self::I64
    }

    pub fn f64() -> Self {
        Self::F64
    }

    pub fn const_int0() -> Self {
        Self::CONST_INT0
    }

    pub fn const_int1() -> Self {
        Self::CONST_INT1
    }

    pub fn blob(size: u64) -> Self {
        Self(12 + size * 2)
    }

    pub fn text(size: u64) -> Self {
        Self(13 + size * 2)
    }

    pub fn kind(&self) -> SerialTypeKind {
        match self.0 {
            0 => SerialTypeKind::Null,
            1 => SerialTypeKind::I8,
            2 => SerialTypeKind::I16,
            3 => SerialTypeKind::I24,
            4 => SerialTypeKind::I32,
            5 => SerialTypeKind::I48,
            6 => SerialTypeKind::I64,
            7 => SerialTypeKind::F64,
            8 => SerialTypeKind::ConstInt0,
            9 => SerialTypeKind::ConstInt1,
            n if n >= 12 => match n % 2 {
                0 => SerialTypeKind::Blob,
                1 => SerialTypeKind::Text,
                _ => unreachable!(),
            }
            _ => unreachable!(),
        }
    }

    pub fn size(&self) -> usize {
        match self.kind() {
            SerialTypeKind::Null => 0,
            SerialTypeKind::I8 => 1,
            SerialTypeKind::I16 => 2,
            SerialTypeKind::I24 => 3,
            SerialTypeKind::I32 => 4,
            SerialTypeKind::I48 => 6,
            SerialTypeKind::I64 => 8,
            SerialTypeKind::F64 => 8,
            SerialTypeKind::ConstInt0 => 0,
            SerialTypeKind::ConstInt1 => 0,
            SerialTypeKind::Blob => (self.0 as usize -12) / 2,
            SerialTypeKind::Text => (self.0 as usize - 13) / 2,
        }
    }
}