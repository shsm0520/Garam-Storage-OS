use std::fs;

pub struct PowerStatus {
    pub ac_online: bool,
    pub battery_pct: u32,
}

pub fn fetch_power_status() -> PowerStatus {
    let mut ac_online = true;
    let mut battery_pct = 100;

    if let Ok(ac_status) = fs::read_to_string("/sys/class/power_supply/AC/online") {
        if ac_status.trim() == "0" { ac_online = false; }
    }
    if let Ok(bat_cap) = fs::read_to_string("/sys/class/power_supply/BAT0/capacity") {
        if let Ok(pct) = bat_cap.trim().parse::<u32>() { battery_pct = pct; }
    }

    PowerStatus { ac_online, battery_pct }
}