pub(crate) fn ease_out(progress: f64) -> f64 {
    1.0 - (1.0 - progress).powi(2)
}

#[cfg(test)]
mod tests {
    use super::ease_out;

    #[test]
    fn ease_out_stays_within_the_expected_range() {
        assert_eq!(ease_out(0.0), 0.0);
        assert_eq!(ease_out(0.5), 0.75);
        assert_eq!(ease_out(1.0), 1.0);
    }
}
