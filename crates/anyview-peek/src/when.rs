//! A modification time as a person reads it.

use anyview_core::{FactTime, FactValue, ModTime};

/// `2 Oct 2026 at 14:30`, in the person's own time zone: the same words and the same zone the
/// Info panel's dates use.
pub fn modified_text(modified: ModTime) -> String {
    FactValue::date(FactTime::from_mod_time(modified))
        .as_str()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_read_as_the_panel_words_dates() {
        let time = ModTime(1_790_951_400 * 1_000_000_000);
        let text = modified_text(time);
        assert_eq!(
            text,
            FactValue::date(FactTime::from_mod_time(time)).as_str()
        );
        // `D Mon YYYY at HH:MM`, with no zone name after it.
        let (date, clock) = text.split_once(" at ").unwrap();
        assert_eq!(date.split(' ').count(), 3, "{text}");
        assert_eq!(clock.len(), 5, "{text}");
        assert!(!text.contains("UTC"), "{text}");
    }
}
