use bitflags::bitflags;

pub mod buffer;
pub mod error;
pub mod clock;

#[derive(Debug, PartialEq)]
pub struct OpenFlags(i32);

bitflags! {
    impl OpenFlags: i32 {
        const None = 0b00000000;
        const Create = 0b00000001;
        const ReadOnly = 0b00000010;
    }
}

impl Default for OpenFlags {
    fn default() -> Self {
        Self::Create
    }
}

#[cfg(test)]
mod tests {
    use super::OpenFlags;

    #[test]
    fn test_individual_flags() {
        let none_flag = OpenFlags::None;
        let create_flag = OpenFlags::Create;
        let read_only_flag = OpenFlags::ReadOnly;

        assert_eq!(none_flag.bits(), 0);
        assert_eq!(create_flag.bits(), 1);
        assert_eq!(read_only_flag.bits(), 2);
    }

    #[test]
    fn test_combined_flags() {
        let combined = OpenFlags::Create | OpenFlags::ReadOnly;
        assert_eq!(combined.bits(), 3);

        assert!(combined.contains(OpenFlags::Create));
        assert!(combined.contains(OpenFlags::ReadOnly));
        assert!(combined.contains(OpenFlags::None));
    }

    #[test]
    fn test_default_flags() {
        let default_flags = OpenFlags::default();
        assert_eq!(default_flags, OpenFlags::Create);
    }
}