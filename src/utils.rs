
pub fn format_duration(duration_ms: Option<u64>) -> String {
    match duration_ms {
        Some(ms) => {
            let total_seconds = ms / 1000;
            let hours = total_seconds / 3600;
            let minutes = (total_seconds % 3600) / 60;
            let seconds = total_seconds % 60;
            if hours > 0 {
                format!("{hours}:{minutes:02}:{seconds:02}")
            } else {
                format!("{minutes}:{seconds:02}")
            }
        }
        None => "".to_string(),
    }
}


#[cfg(test)]
mod tests {
    use super::format_duration;


    #[test]
    fn test_format_duration_cases_empty_input() {
        assert_eq!(format_duration(None), "");
    }


    #[test]
    fn test_format_duration_cases_subsecond() {

        // should floor to 0:00
        assert_eq!(format_duration(Some(0)), "0:00");
        assert_eq!(format_duration(Some(1)), "0:00");
        assert_eq!(format_duration(Some(10)), "0:00");
        assert_eq!(format_duration(Some(100)), "0:00");
        assert_eq!(format_duration(Some(500)), "0:00");
        assert_eq!(format_duration(Some(501)), "0:00");
        assert_eq!(format_duration(Some(900)), "0:00");
        assert_eq!(format_duration(Some(999)), "0:00");
    }


    #[test]
    fn test_format_duration_cases_second_to_minute() {
        // 1 second = 1000 ms
        assert_eq!(format_duration(Some(1000)), "0:01");
        // 59 seconds = 59000 ms
        assert_eq!(format_duration(Some(59000)), "0:59");
    }

    #[test]
    fn test_format_duration_cases_minute_to_hour() {
        // 60 seconds = 60000 ms = 1:00
        assert_eq!(format_duration(Some(60000)), "1:00");
        // 61 seconds = 61000 ms = 1:01
        assert_eq!(format_duration(Some(61000)), "1:01");
        // 119 seconds = 119000 ms = 1:59
        assert_eq!(format_duration(Some(119000)), "1:59");
        // 120 seconds = 120000 ms = 2:00
        assert_eq!(format_duration(Some(120000)), "2:00");
        // 121 seconds = 121000 ms = 2:01
        assert_eq!(format_duration(Some(121000)), "2:01");
        // 599 seconds = 599000 ms = 9:59
        assert_eq!(format_duration(Some(599000)), "9:59");
        // 600 seconds = 600000 ms = 10:00
        assert_eq!(format_duration(Some(600000)), "10:00");
        // 601 seconds = 601000 ms = 10:01
        assert_eq!(format_duration(Some(601000)), "10:01");
    }


    #[test]
    fn test_format_duration_cases_suprahour() {
        // 1 hour = 3600 seconds = 3_600_000 ms
        assert_eq!(format_duration(Some(3_600_000)), "1:00:00");
        assert_eq!(format_duration(Some(3_601_000)), "1:00:01");
        assert_eq!(format_duration(Some(3_610_000)), "1:00:10");
        assert_eq!(format_duration(Some(3_660_000)), "1:01:00");
        assert_eq!(format_duration(Some(4_200_000)), "1:10:00");
        // 1 hour, 16 minutes, 33 seconds = 4593 seconds = 4_593_000 ms
        assert_eq!(format_duration(Some(4_593_000)), "1:16:33");
    }
}
