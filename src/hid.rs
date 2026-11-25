use hidapi::HidApi;
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
use std::thread;
use std::time::Duration;

/// Extracts the touchpad-click state from a DualShock 4 input report.
/// USB (and the simple Bluetooth mode) use report 0x01 with the three button bytes at 5..=7;
/// the full Bluetooth report 0x11 carries the same bytes two positions later (7..=9).
/// Returns None for anything else, such as an empty read or an unrelated report.
pub(crate) fn ds4_touchpad_click_from_report(report: &[u8]) -> Option<bool> {
    let button_byte = match report.first() {
        Some(0x01) if report.len() >= 8 => report[7],
        Some(0x11) if report.len() >= 10 => report[9],
        _ => return None,
    };
    Some(button_byte & 0x02 != 0)
}

/// Reads the DualShock 4 V2 touchpad-click bit directly from its USB HID input report.
///
/// Sony's DS4 USB report is 64 bytes (report ID 0x01). The three button bytes
/// begin at offsets 5..=7; bit 1 of the third button byte (offset 7) is the
/// touchpad click. The PS button remains independently available through gilrs::Mode.
pub(crate) fn start_ds4_touchpad_reader() -> Arc<AtomicBool> {
    const SONY_VENDOR_ID: u16 = 0x054C;
    // DualShock 4 v1, DualShock 4 v2, and Sony's USB wireless adapter.
    const DUALSHOCK4_PRODUCT_IDS: [u16; 3] = [0x05C4, 0x09CC, 0x0BA0];

    let pressed = Arc::new(AtomicBool::new(false));
    let thread_state = Arc::clone(&pressed);

    thread::spawn(move || {
        let mut api = match HidApi::new() {
            Ok(api) => api,
            Err(error) => {
                eprintln!("Could not initialize HID access for the DS4 touchpad: {error}");
                return;
            }
        };

        let mut device = None;

        loop {
            if device.is_none() {
                let _ = api.refresh_devices();
                device = api
                    .device_list()
                    .find(|info| {
                        info.vendor_id() == SONY_VENDOR_ID
                            && DUALSHOCK4_PRODUCT_IDS.contains(&info.product_id())
                    })
                    .and_then(|info| info.open_device(&api).ok());

                if device.is_none() {
                    thread_state.store(false, Ordering::Relaxed);
                    thread::sleep(Duration::from_millis(250));
                    continue;
                }
            }

            let mut report = [0u8; 96];
            let read_result = device
                .as_ref()
                .expect("HID device was just checked")
                .read_timeout(&mut report, 30);

            match read_result {
                Ok(read) => {
                    if let Some(is_pressed) = ds4_touchpad_click_from_report(&report[..read]) {
                        thread_state.store(is_pressed, Ordering::Relaxed);
                    }
                    // Ignore short or unrelated HID reports; don't invent a press.
                }
                Err(error) => {
                    eprintln!("DualShock 4 touchpad HID read stopped: {error}");
                    device = None;
                    thread_state.store(false, Ordering::Relaxed);
                    thread::sleep(Duration::from_millis(250));
                }
            }
        }
    });

    pressed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ds4_usb_touchpad_click_is_read_from_its_own_hid_bit() {
        let mut report = [0u8; 64];
        report[0] = 0x01;
        report[7] = 0x02;
        assert_eq!(ds4_touchpad_click_from_report(&report), Some(true));

        report[7] = 0x00;
        assert_eq!(ds4_touchpad_click_from_report(&report), Some(false));
        assert_eq!(ds4_touchpad_click_from_report(&report[..7]), None);
        // the PS button (bit 0) must not count as a touchpad click
        report[7] = 0x01;
        assert_eq!(ds4_touchpad_click_from_report(&report), Some(false));
    }

    #[test]
    fn ds4_bluetooth_touchpad_click_is_read_from_report_0x11() {
        let mut report = [0u8; 78];
        report[0] = 0x11;
        report[9] = 0x02;
        assert_eq!(ds4_touchpad_click_from_report(&report), Some(true));
        report[9] = 0x00;
        assert_eq!(ds4_touchpad_click_from_report(&report), Some(false));
        assert_eq!(ds4_touchpad_click_from_report(&report[..9]), None);
        // empty reads (timeouts) and unrelated reports never invent a press
        assert_eq!(ds4_touchpad_click_from_report(&[]), None);
        assert_eq!(ds4_touchpad_click_from_report(&[0x05, 0, 0, 0, 0, 0, 0, 0, 0, 0x02]), None);
    }
}
