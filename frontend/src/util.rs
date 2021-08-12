use std::time::Duration;

pub fn duration_to_m_s_ms(duration: Duration) -> (u64, u64, u32) {
    let m = duration.as_secs() / 60;
    let s = duration.as_secs() % 60;
    let ms = duration.subsec_millis();
    (m, s, ms)
}

pub fn format_time(duration: Duration) -> String {
    let (m, s, ms) = duration_to_m_s_ms(duration);
    format!("{:0>2}:{:0>2}.{:0>3}", m, s, ms)
}

pub fn format_time_with_units(duration: Duration) -> String {
    let (m, s, ms) = duration_to_m_s_ms(duration);
    format!("{:0>2}m {:0>2}.{:0>3}s", m, s, ms)
}
