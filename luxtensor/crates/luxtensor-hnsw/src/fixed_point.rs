//! Fixed-Point Vector Arithmetic for Consensus-Safe Distance Calculations
//!
//! This module provides `FixedPointVector<D>` - a D-dimensional vector using
//! `I64F32` fixed-point numbers instead of floating-point.
//!
//! ## Why Fixed-Point?
//!
//! Floating-point operations (`f32`/`f64`) are **non-deterministic** across CPU
//! architectures due to different rounding modes:
//! - x86_64 AVX-512: Uses 80-bit extended precision internally
//! - ARM NEON: Uses strict 64-bit precision
//!
//! A difference of `0.0000001` in distance calculation causes different nearest
//! neighbor results, leading to different graph topologies and **consensus forks**.
//!
//! Fixed-point arithmetic guarantees **bit-perfect consistency** regardless of
//! the validator's hardware.

use fixed::FixedI64;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::ops::{Add, Sub};

use crate::error::{HnswError, Result};

/// Type alias for our fixed-point type: 32 integer bits, 32 fractional bits
pub type I64F32 = FixedI64<fixed::types::extra::U32>;

/// A D-dimensional vector using I64F32 fixed-point arithmetic.
///
/// Each component is stored as `I64F32` (64 integer bits, 32 fractional bits),
/// providing approximately 9 decimal digits of precision while maintaining
/// perfect cross-platform determinism.
#[derive(Clone, Debug)]
pub struct FixedPointVector<const D: usize> {
    /// The vector components
    pub components: [I64F32; D],
}

// Custom serialization: convert array to Vec<i64> (raw bits)
impl<const D: usize> Serialize for FixedPointVector<D> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize as Vec of raw i64 bits for maximum efficiency and determinism
        let bits: Vec<i64> = self.components.iter().map(|c| c.to_bits()).collect();
        bits.serialize(serializer)
    }
}

impl<'de, const D: usize> Deserialize<'de> for FixedPointVector<D> {
    fn deserialize<De>(deserializer: De) -> std::result::Result<Self, De::Error>
    where
        De: Deserializer<'de>,
    {
        let bits: Vec<i64> = Vec::deserialize(deserializer)?;
        if bits.len() != D {
            return Err(serde::de::Error::custom(format!(
                "Expected {} components, got {}",
                D,
                bits.len()
            )));
        }

        let mut components = [I64F32::ZERO; D];
        for (i, &b) in bits.iter().enumerate() {
            components[i] = I64F32::from_bits(b);
        }
        Ok(Self { components })
    }
}

impl<const D: usize> FixedPointVector<D> {
    /// Create a zero vector.
    pub fn zero() -> Self {
        Self {
            components: [I64F32::ZERO; D],
        }
    }

    /// Create a vector from an array of I64F32 values.
    pub fn new(components: [I64F32; D]) -> Self {
        Self { components }
    }

    /// Create a vector from a slice of f32 values.
    ///
    /// # Panics
    /// Panics if the slice length doesn't match D.
    pub fn from_f32_slice(slice: &[f32]) -> Result<Self> {
        if slice.len() != D {
            return Err(HnswError::DimensionMismatch {
                expected: D,
                actual: slice.len(),
            });
        }

        let mut components = [I64F32::ZERO; D];
        for (i, &val) in slice.iter().enumerate() {
            components[i] = I64F32::from_num(val);
        }
        Ok(Self { components })
    }

    /// Create a vector from a slice of i64 scaled integers.
    ///
    /// The integers are interpreted as fixed-point values with the given scale.
    /// For example, if `scale = 1_000_000`, then `1_500_000` represents `1.5`.
    pub fn from_scaled_integers(values: &[i64], scale: i64) -> Result<Self> {
        if values.len() != D {
            return Err(HnswError::DimensionMismatch {
                expected: D,
                actual: values.len(),
            });
        }

        let mut components = [I64F32::ZERO; D];
        let scale_fixed = I64F32::from_num(scale);
        for (i, &val) in values.iter().enumerate() {
            components[i] = I64F32::from_num(val) / scale_fixed;
        }
        Ok(Self { components })
    }

    /// Calculate the squared Euclidean distance to another vector.
    ///
    /// Returns the sum of squared differences: Σ(a_i - b_i)²
    ///
    /// This is preferred over regular distance to avoid the sqrt operation,
    /// which is expensive in fixed-point and not needed for comparisons.
    #[inline]
    pub fn squared_distance(&self, other: &Self) -> I64F32 {
        let mut sum = I64F32::ZERO;
        for i in 0..D {
            let diff = self.components[i] - other.components[i];
            // saturating_mul prevents overflow, clamping to MAX on overflow
            sum = sum.saturating_add(diff.saturating_mul(diff));
        }
        sum
    }

    /// Calculate the Euclidean distance to another vector.
    ///
    /// Note: This uses an iterative Newton-Raphson square root approximation
    /// that is deterministic across platforms.
    #[inline]
    pub fn euclidean_distance(&self, other: &Self) -> I64F32 {
        let squared = self.squared_distance(other);
        fixed_point_sqrt(squared)
    }

    /// Calculate the dot product with another vector.
    #[inline]
    pub fn dot(&self, other: &Self) -> I64F32 {
        let mut sum = I64F32::ZERO;
        for i in 0..D {
            sum = sum.saturating_add(self.components[i].saturating_mul(other.components[i]));
        }
        sum
    }

    /// Calculate the squared magnitude (L2 norm squared) of this vector.
    #[inline]
    pub fn squared_magnitude(&self) -> I64F32 {
        self.dot(self)
    }

    /// Calculate the cosine similarity with another vector.
    ///
    /// Returns a value in [-1, 1] where 1 means identical direction,
    /// 0 means orthogonal, and -1 means opposite direction.
    ///
    /// Formula: cos(θ) = (A · B) / (|A| × |B|)
    pub fn cosine_similarity(&self, other: &Self) -> I64F32 {
        let dot_product = self.dot(other);
        let mag_a = fixed_point_sqrt(self.squared_magnitude());
        let mag_b = fixed_point_sqrt(other.squared_magnitude());

        // Avoid division by zero
        let magnitude_product = mag_a.saturating_mul(mag_b);
        if magnitude_product == I64F32::ZERO {
            return I64F32::ZERO;
        }

        dot_product.saturating_div(magnitude_product)
    }

    /// Convert to a Vec<f32> for external use (e.g., display).
    pub fn to_f32_vec(&self) -> Vec<f32> {
        self.components.iter().map(|&c| c.to_num::<f32>()).collect()
    }

    /// Get the dimension of this vector.
    #[inline]
    pub const fn dimension(&self) -> usize {
        D
    }
}

impl<const D: usize> Default for FixedPointVector<D> {
    fn default() -> Self {
        Self::zero()
    }
}

impl<const D: usize> Add for FixedPointVector<D> {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        let mut result = [I64F32::ZERO; D];
        for i in 0..D {
            result[i] = self.components[i].saturating_add(other.components[i]);
        }
        Self { components: result }
    }
}

impl<const D: usize> Sub for FixedPointVector<D> {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        let mut result = [I64F32::ZERO; D];
        for i in 0..D {
            result[i] = self.components[i].saturating_sub(other.components[i]);
        }
        Self { components: result }
    }
}

/// Deterministic fixed-point square root using Newton-Raphson iteration.
///
/// This algorithm is guaranteed to produce identical results on all platforms
/// because it uses only fixed-point arithmetic operations.
#[inline]
pub fn fixed_point_sqrt(value: I64F32) -> I64F32 {
    if value <= I64F32::ZERO {
        return I64F32::ZERO;
    }

    // Initial guess: half of the value (works reasonably for most inputs)
    let mut guess = value / I64F32::from_num(2);

    // Newton-Raphson iterations: x_new = (x + value/x) / 2
    // 10 iterations provides sufficient precision for I64F32
    for _ in 0..10 {
        if guess == I64F32::ZERO {
            break;
        }
        let next = (guess + value.saturating_div(guess)) / I64F32::from_num(2);
        if next == guess {
            break; // Converged
        }
        guess = next;
    }

    guess
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_vector() {
        let v: FixedPointVector<3> = FixedPointVector::zero();
        assert_eq!(v.components[0], I64F32::ZERO);
        assert_eq!(v.components[1], I64F32::ZERO);
        assert_eq!(v.components[2], I64F32::ZERO);
    }

    #[test]
    fn test_from_f32_slice() {
        let v: FixedPointVector<3> = FixedPointVector::from_f32_slice(&[1.0, 2.0, 3.0]).unwrap();
        assert_eq!(v.components[0].to_num::<f32>(), 1.0);
        assert_eq!(v.components[1].to_num::<f32>(), 2.0);
        assert_eq!(v.components[2].to_num::<f32>(), 3.0);
    }

    #[test]
    fn test_dimension_mismatch() {
        let result: Result<FixedPointVector<3>> = FixedPointVector::from_f32_slice(&[1.0, 2.0]);
        assert!(result.is_err());
    }

    #[test]
    fn test_squared_distance() {
        let a: FixedPointVector<3> = FixedPointVector::from_f32_slice(&[0.0, 0.0, 0.0]).unwrap();
        let b: FixedPointVector<3> = FixedPointVector::from_f32_slice(&[3.0, 4.0, 0.0]).unwrap();
        let dist = a.squared_distance(&b);
        // 3² + 4² = 25
        assert!((dist.to_num::<f32>() - 25.0).abs() < 0.001);
    }

    #[test]
    fn test_euclidean_distance() {
        let a: FixedPointVector<3> = FixedPointVector::from_f32_slice(&[0.0, 0.0, 0.0]).unwrap();
        let b: FixedPointVector<3> = FixedPointVector::from_f32_slice(&[3.0, 4.0, 0.0]).unwrap();
        let dist = a.euclidean_distance(&b);
        // sqrt(25) = 5
        assert!((dist.to_num::<f32>() - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a: FixedPointVector<3> = FixedPointVector::from_f32_slice(&[1.0, 0.0, 0.0]).unwrap();
        let b: FixedPointVector<3> = FixedPointVector::from_f32_slice(&[1.0, 0.0, 0.0]).unwrap();
        let sim = a.cosine_similarity(&b);
        assert!((sim.to_num::<f32>() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a: FixedPointVector<3> = FixedPointVector::from_f32_slice(&[1.0, 0.0, 0.0]).unwrap();
        let b: FixedPointVector<3> = FixedPointVector::from_f32_slice(&[0.0, 1.0, 0.0]).unwrap();
        let sim = a.cosine_similarity(&b);
        assert!(sim.to_num::<f32>().abs() < 0.01);
    }

    #[test]
    fn test_fixed_point_sqrt_determinism() {
        // This test verifies that sqrt produces exactly the same result
        // regardless of how many times it's called (no state leakage)
        let value = I64F32::from_num(25);
        let result1 = fixed_point_sqrt(value);
        let result2 = fixed_point_sqrt(value);
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let v: FixedPointVector<3> = FixedPointVector::from_f32_slice(&[1.5, 2.5, 3.5]).unwrap();
        let bytes = bincode::serialize(&v).unwrap();
        let restored: FixedPointVector<3> = bincode::deserialize(&bytes).unwrap();

        for i in 0..3 {
            assert_eq!(v.components[i], restored.components[i]);
        }
    }
}
