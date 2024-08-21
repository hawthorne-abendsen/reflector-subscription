pub trait U128Extensions {
    fn sqrt(self) -> u128;
}

impl U128Extensions for u128 {
    fn sqrt(self) -> u128 {
        if self == 0 {
            return 0;
        }

        // Compute bit, the largest power of 4 <= n
        let max_shift: u32 = 127; // Since u128 has 128 bits, the maximum shift is 127
        let shift: u32 = (max_shift - self.leading_zeros()) & !1;
        let mut bit = 1u128 << shift;

        // Algorithm based on the implementation in:
        // https://en.wikipedia.org/wiki/Methods_of_computing_square_roots#Binary_numeral_system_(base_2)
        let mut result = 0u128;
        let mut n = self;

        while bit != 0 {
            if n >= result + bit {
                n -= result + bit;
                result = (result >> 1) + bit;
            } else {
                result >>= 1;
            }
            bit >>= 2;
        }

        result
    }
}