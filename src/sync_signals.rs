/// A trait to provide a common interface for all signal calculations
pub trait StockSignal {
    /// A signal's data type
    type SignalType;

    /// Calculate a signal on the provided series
    ///
    /// # Returns
    /// Calculated signal of the provided type, or `None` on error/invalid data
    ///
    /// We needed to mark this trait as public because we extracted it into a separate file.
    /// Initially, it was in [logic.rs](src/logic.rs), where it is used, and it didn't have to be marked
    /// public then.
    ///
    /// Its original return type was `Option<Self::SignalType>`, just like with all its implementors.
    /// Implementors don't require changing their return type.
    ///
    /// Since we intend to use the trait only locally, and we don't need to use it in generic functions,
    /// but we want to use it in a multithreaded context, we decided to de-sugar its return type and to
    /// add the `Send` trait which is necessary for multithreaded execution.
    ///
    /// We could instead just suppress the lint.
    fn calculate(&self, series: &[f64]) -> Option<Self::SignalType>;
}

/// Find the minimum in a series of `f64`
pub struct MinPrice {}

impl StockSignal for MinPrice {
    type SignalType = f64;

    /// Returns the minimum in a series of `f64` or `None` if it's empty.
    fn calculate(&self, series: &[f64]) -> Option<Self::SignalType> {
        if series.is_empty() {
            None
        } else {
            Some(
                series
                    .iter()
                    .fold(f64::MAX, |min_elt, curr_elt| min_elt.min(*curr_elt)),
            )
        }
    }
}

/// Find the maximum in a series of `f64`
pub struct MaxPrice {}

impl StockSignal for MaxPrice {
    type SignalType = f64;

    /// Returns the maximum in a series of `f64` or `None` if it's empty.
    fn calculate(&self, series: &[f64]) -> Option<Self::SignalType> {
        if series.is_empty() {
            None
        } else {
            Some(
                series
                    .iter()
                    .fold(f64::MIN, |max_elt, curr_elt| max_elt.max(*curr_elt)),
            )
        }
    }
}

/// Calculates the absolute and relative difference between the last and the first element of an f64 series.
///
/// The relative difference is calculated as `(last - first) / first`.
pub struct PriceDifference {}

impl StockSignal for PriceDifference {
    type SignalType = (f64, f64);

    /// Calculates the absolute and relative difference between the last and the first element of an f64 series.
    ///
    /// The relative difference is calculated as `(last - first) / first`.
    ///
    /// # Returns
    /// A tuple of `(absolute, relative)` differences, or `None` if the series is empty.
    fn calculate(&self, series: &[f64]) -> Option<Self::SignalType> {
        if series.is_empty() {
            None
        } else {
            let first = series.first().expect("Expected first.");
            let last = series.last().unwrap_or(first);

            let abs_diff = last - first;

            let first = if *first == 0.0 { 1.0 } else { *first };
            let rel_diff = abs_diff / first;

            Some((abs_diff, rel_diff))
        }
    }
}

/// Window function to create a simple moving average
pub struct WindowedSMA {
    pub window_size: usize,
}

impl StockSignal for WindowedSMA {
    type SignalType = Vec<f64>;

    /// Window function to create a simple moving average
    ///
    /// # Returns
    /// A vector with the series' windowed averages;
    /// or `None` in case the series is empty or window size <= 1.
    fn calculate(&self, series: &[f64]) -> Option<Self::SignalType> {
        if !series.is_empty() && self.window_size > 1 {
            Some(
                series
                    .windows(self.window_size)
                    .map(|window| window.iter().sum::<f64>() / window.len() as f64)
                    .collect(),
            )
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_min_price_calculate() {
        let signal = MinPrice {};
        assert_eq!(signal.calculate(&[]), None);
        assert_eq!(signal.calculate(&[1.0]), Some(1.0));
        assert_eq!(signal.calculate(&[1.0, 0.0]), Some(0.0));
        assert_eq!(
            signal.calculate(&[2.0, 3.0, 5.0, 6.0, 1.0, 2.0, 10.0]),
            Some(1.0)
        );
        assert_eq!(
            signal.calculate(&[0.0, 3.0, 5.0, 6.0, 1.0, 2.0, 1.0]),
            Some(0.0)
        );
    }

    #[test]
    fn test_max_price_calculate() {
        let signal = MaxPrice {};
        assert_eq!(signal.calculate(&[]), None);
        assert_eq!(signal.calculate(&[1.0]), Some(1.0));
        assert_eq!(signal.calculate(&[1.0, 0.0]), Some(1.0));
        assert_eq!(
            signal.calculate(&[2.0, 3.0, 5.0, 6.0, 1.0, 2.0, 10.0]),
            Some(10.0)
        );
        assert_eq!(
            signal.calculate(&[0.0, 3.0, 5.0, 6.0, 1.0, 2.0, 1.0]),
            Some(6.0)
        );
    }

    #[test]
    fn test_price_difference_calculate() {
        let signal = PriceDifference {};
        assert_eq!(signal.calculate(&[]), None);
        assert_eq!(signal.calculate(&[1.0]), Some((0.0, 0.0)));
        assert_eq!(signal.calculate(&[1.0, 0.0]), Some((-1.0, -1.0)));
        assert_eq!(
            signal.calculate(&[2.0, 3.0, 5.0, 6.0, 1.0, 2.0, 10.0]),
            Some((8.0, 4.0))
        );
        assert_eq!(
            signal.calculate(&[0.0, 3.0, 5.0, 6.0, 1.0, 2.0, 1.0]),
            Some((1.0, 1.0))
        );
    }

    #[test]
    fn test_windowed_sma_calculate() {
        let series = vec![2.0, 4.5, 5.3, 6.5, 4.7];

        let signal = WindowedSMA { window_size: 3 };
        assert_eq!(
            signal.calculate(&series),
            Some(vec![3.9333333333333336, 5.433333333333334, 5.5])
        );

        let signal = WindowedSMA { window_size: 5 };
        assert_eq!(signal.calculate(&series), Some(vec![4.6]));

        let signal = WindowedSMA { window_size: 10 };
        assert_eq!(signal.calculate(&series), Some(vec![]));
    }
}
