#[derive(Debug, Clone, Copy, PartialEq, Hash)]
pub struct BitPath {
    length: u32,
    value: u64,
}

impl BitPath {
    pub fn new(length: u32, value: u64) -> Self {
        BitPath { length, value }
    }

    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    pub fn length(&self) -> u32 {
        self.length
    }

    pub fn value(&self) -> u64 {
        self.value
    }

    pub fn push(&mut self, bit: bool) {
        self.value = self.value | ((bit as u64) << self.length);
        self.length += 1;
    }

    pub fn pop(&mut self) -> Option<bool> {
        if self.length == 0 {
            return None;
        }
        let bit = (self.value >> (self.length - 1)) & 1;
        // mask out the bit
        self.value = self.value & !(1 << (self.length - 1));
        self.length -= 1;
        Some(bit == 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bit_path() {
        let mut path = BitPath::new(0, 0);
        assert_eq!(path.is_empty(), true);
        assert_eq!(path.length(), 0);
        assert_eq!(path.value(), 0);

        path.push(true);
        assert_eq!(path.is_empty(), false);
        assert_eq!(path.length(), 1);
        assert_eq!(path.value(), 1);

        path.push(false);
        assert_eq!(path.is_empty(), false);
        assert_eq!(path.length(), 2);
        assert_eq!(path.value(), 1);

        assert_eq!(path.pop(), Some(false));
        assert_eq!(path.is_empty(), false);
        assert_eq!(path.length(), 1);
        assert_eq!(path.value(), 1);

        assert_eq!(path.pop(), Some(true));
        assert_eq!(path.is_empty(), true);
        assert_eq!(path.length(), 0);
        assert_eq!(path.value(), 0);

        assert_eq!(path.pop(), None);
        assert_eq!(path.is_empty(), true);
        assert_eq!(path.length(), 0);
        assert_eq!(path.value(), 0);
    }
}
