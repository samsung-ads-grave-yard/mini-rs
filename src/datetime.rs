/// Date time and formatting.

// TODO: Add UTC and timezones.

use std::mem;
use std::ptr;

const DAY_NAMES_LONG: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const MONTH_NAMES_LONG: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August",
    "September", "October", "November", "December"];

#[derive(Debug)]
#[repr(C)]
struct tm {
    tm_sec: i32,
    tm_min: i32,
    tm_hour: i32,
    tm_mday: i32,
    tm_mon: i32,
    tm_year: i32,
    tm_wday: i32,
    tm_yday: i32,
    tm_isdst: i32,
    tm_gmtoff: i64,
    tm_zone: *const i8,
}

#[allow(non_camel_case_types)]
type time_t = i64;

extern "C" {
    fn localtime_r(timep: *const time_t, result: *mut tm) -> *mut tm;
    fn time(tloc: *mut time_t) -> time_t;
}

/// Whether the daylight saving time is active, inactive or unknown.
pub enum DaylightSavingTime {
    Active,
    Inactive,
    Unknown,
}

/// Local date-time structure.
pub struct DateTime {
    /// Seconds after the minute, 0-60 (60 being the leap second).
    pub second: u32,
    /// Minutes after the hour, 0-59.
    pub minute: u32,
    /// Hours since midnight, 0-23.
    pub hour: u32,
    /// Day of the month, 1-31.
    pub month_day: u32,
    /// Month since January, 0-11.
    pub month: u32,
    /// Current year, 0-9999.
    pub year: u32,
    /// Days since Sunday, 0-6.
    pub week_day: u32,
    /// Days since January 1st, 0-365.
    pub year_day: u32,
    pub daylight_saving_time: DaylightSavingTime,
}

impl DateTime {
    /// Get the current datetime.
    pub fn now() -> Self {
        let result =
            unsafe {
                let mut result: tm = mem::zeroed();
                let time = time(ptr::null_mut());
                localtime_r(&time, &mut result);
                result
            };
        let daylight_saving_time =
            if result.tm_isdst < 0 {
                DaylightSavingTime::Unknown
            }
            else if result.tm_isdst != 0 {
                DaylightSavingTime::Active
            }
            else {
                DaylightSavingTime::Inactive
            };
        Self {
            second: result.tm_sec as u32,
            minute: result.tm_min as u32,
            hour: result.tm_hour as u32,
            month_day: result.tm_mday as u32,
            month: result.tm_mon as u32,
            year: result.tm_year as u32 + 1900,
            week_day: result.tm_wday as u32,
            year_day: result.tm_yday as u32,
            daylight_saving_time,
        }
    }

    /// | %Format String | Description                                                         |
    /// |----------------|---------------------------------------------------------------------|
    /// | %%             | a literal %                                                         |
    /// | %a             | locale’s abbreviated weekday name (e.g., Sun)                       |
    /// | %A             | locale’s full weekday name (e.g., Sunday)                           |
    /// | %b             | locale’s abbreviated month name (e.g., Jan)                         |
    /// | %B             | locale’s full month name (e.g., January)                            |
    /// | %c             | locale’s date and time (e.g., Thu Mar 3 23:05:25 2005)              |
    /// | %C             | previous century; like %Y, except omit last two digits (e.g., 21)   |
    /// | %d             | day of month (e.g, 01)                                              |
    /// | %D             | date; same as %m/%d/%y                                              |
    /// | %e             | day of month, space padded; same as %_d                             |
    /// | %F             | full date; same as %Y-%m-%d                                         |
    /// | %g             | last two digits of year of ISO week number (see %G)                 |
    /// | %G             | year of ISO week number (see %V); normally useful only with %V      |
    /// | %h             | same as %b                                                          |
    /// | %H             | hour (00..23)                                                       |
    /// | %I             | hour (01..12)                                                       |
    /// | %j             | day of year (001..366)                                              |
    /// | %k             | hour ( 0..23)                                                       |
    /// | %l             | hour ( 1..12)                                                       |
    /// | %m             | month (01..12)                                                      |
    /// | %M             | minute (00..59)                                                     |
    /// | %n             | a newline                                                           |
    /// | %N             | nanoseconds (000000000..999999999)                                  |
    /// | %p             | locale’s equivalent of either AM or PM; blank if not known          |
    /// | %P             | like %p, but lower case                                             |
    /// | %r             | locale’s 12-hour clock time (e.g., 11:11:04 PM)                     |
    /// | %R             | 24-hour hour and minute; same as %H:%M                              |
    /// | %s             | seconds since 1970-01-01 00:00:00 UTC                               |
    /// | %S             | second (00..60)                                                     |
    /// | %t             | a tab                                                               |
    /// | %T             | time; same as %H:%M:%S                                              |
    /// | %u             | day of week (1..7); 1 is Monday                                     |
    /// | %U             | week number of year, with Sunday as first day of week (00..53)      |
    /// | %V             | ISO week number, with Monday as first day of week (01..53)          |
    /// | %w             | day of week (0..6); 0 is Sunday                                     |
    /// | %W             | week number of year, with Monday as first day of week (00..53)      |
    /// | %x             | locale’s date representation (e.g., 12/31/99)                       |
    /// | %X             | locale’s time representation (e.g., 23:13:48)                       |
    /// | %y             | last two digits of year (00..99)                                    |
    /// | %Y             | year                                                                |
    /// | %z             | +hhmm numeric timezone (e.g., -0400)                                |
    /// | %:z            | +hh:mm numeric timezone (e.g., -04:00)                              |
    /// | %::z           | +hh:mm:ss numeric time zone (e.g., -04:00:00)                       |
    /// | %:::z          | numeric time zone with : to necessary precision (e.g., -04, +05:30) |
    /// | %Z             | alphabetic time zone abbreviation (e.g., EDT)                       |
    pub fn format(&self, format: &str) -> String {
        let mut result = String::new();
        let mut is_control_char = false;
        for char in format.chars() {
            if is_control_char {
                let string =
                    match char {
                        '%' => '%'.to_string(),
                        'a' => DAY_NAMES_LONG[self.week_day as usize][..3].to_string(),
                        'A' => DAY_NAMES_LONG[self.week_day as usize].to_string(),
                        'b' | 'h' => MONTH_NAMES_LONG[self.month as usize][..3].to_string(),
                        'B' => MONTH_NAMES_LONG[self.month as usize].to_string(),
                        'c' => self.format("%a %b %d %H:%M:%S %Y"),
                        'C' => (self.year / 100).to_string(),
                        'd' => (self.month_day).to_string(),
                        'D' => self.format("%m/%d/%y"),
                        'e' => format!("{:2}", self.month_day),
                        'F' => self.format("%Y-%m-%d"),
                        'g' => unimplemented!(),
                        'G' => unimplemented!(),
                        'H' => format!("{:02}", self.hour),
                        'I' => format!("{:02}", self.hour % 12),
                        'j' => format!("{:03}", self.year_day),
                        'k' => self.hour.to_string(),
                        'l' => (self.hour % 12).to_string(),
                        'm' => format!("{:02}", self.month + 1),
                        'M' => format!("{:02}", self.minute),
                        'n' => "\n".to_string(),
                        'N' => unimplemented!(),
                        'p' => unimplemented!(),
                        'P' => unimplemented!(),
                        'r' => self.format("%I:%M:%S"), // FIXME: should be localized.
                        'R' => self.format("%H:%M"),
                        's' => unimplemented!(),
                        'S' => format!("{:02}", self.second),
                        't' => "\t".to_string(),
                        'T' => self.format("%H:%M:%S"),
                        'u' =>
                            if self.week_day == 0 {
                                "7".to_string()
                            }
                            else {
                                self.week_day.to_string()
                            },
                        'U' => unimplemented!(),
                        'V' => unimplemented!(),
                        'w' => self.week_day.to_string(),
                        'W' => unimplemented!(),
                        'x' => self.format("%m/%e/%y"), // FIXME: should be localized.
                        'X' => self.format("%H:%M:%S"), // FIXME: should be localized.
                        'y' => (self.year % 100).to_string(),
                        'Y' => self.year.to_string(),
                        _ => panic!("unexpected control character: {}", char),
                    };
                result.push_str(&string);
            }
            else if char != '%' {
                result.push(char);
            }
            is_control_char = !is_control_char && char == '%';
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::{DateTime, DaylightSavingTime};

    #[test]
    fn now() {
        let _datetime = DateTime::now();
    }

    // TODO: switch to clippy::cyclomatic_complexity when #44690 is fixed.
    #[allow(unknown_lints, renamed_and_removed_lints, cyclomatic_complexity)]
    #[test]
    fn format() {
        let datetime = DateTime {
            second: 12,
            minute: 3,
            hour: 14,
            month_day: 15,
            month: 8,
            year: 2018,
            week_day: 6,
            year_day: 200,
            daylight_saving_time: DaylightSavingTime::Active,
        };

        assert_eq!(datetime.format("year %Y"), "year 2018");
        assert_eq!(datetime.format("year %Y is"), "year 2018 is");

        assert_eq!(datetime.format("%%x"), "%x");
        assert_eq!(datetime.format("%a"), "Sat");
        assert_eq!(datetime.format("%A"), "Saturday");
        assert_eq!(datetime.format("%b"), "Sep");
        assert_eq!(datetime.format("%B"), "September");
        assert_eq!(datetime.format("%c"), "Sat Sep 15 14:03:12 2018");
        assert_eq!(datetime.format("%C"), "20");
        assert_eq!(datetime.format("%d"), "15");
        assert_eq!(datetime.format("%D"), "09/15/18");
        assert_eq!(datetime.format("%e"), "15");
        assert_eq!(datetime.format("%F"), "2018-09-15");
        assert_eq!(datetime.format("%h"), "Sep");
        assert_eq!(datetime.format("%H"), "14");
        assert_eq!(datetime.format("%I"), "02");
        assert_eq!(datetime.format("%j"), "200");
        assert_eq!(datetime.format("%k"), "14");
        assert_eq!(datetime.format("%l"), "2");
        assert_eq!(datetime.format("%M"), "03");
        assert_eq!(datetime.format("%n"), "\n");
        assert_eq!(datetime.format("%r"), "02:03:12");
        assert_eq!(datetime.format("%R"), "14:03");
        assert_eq!(datetime.format("%S"), "12");
        assert_eq!(datetime.format("%t"), "\t");
        assert_eq!(datetime.format("%T"), "14:03:12");
        assert_eq!(datetime.format("%u"), "6");
        assert_eq!(datetime.format("%w"), "6");
        assert_eq!(datetime.format("%x"), "09/15/18");
        assert_eq!(datetime.format("%X"), "14:03:12");
        assert_eq!(datetime.format("%y"), "18");
        assert_eq!(datetime.format("%Y"), "2018");
    }
}
