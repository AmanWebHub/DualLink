use gilrs::{Axis, Button as PadButton};
use vigem_client::{XButtons, XGamepad};

/// Reads an analog trigger. Most Windows backends report L2/R2 as an analog
/// *button* (not an axis), so check that first, then the Z axis, and finally
/// fall back to the digital pressed state so the trigger never reads as 0.
pub(crate) fn read_trigger(gamepad: &gilrs::Gamepad<'_>, button: PadButton, axis: Axis) -> f32 {
    let from_button = gamepad
        .button_data(button)
        .map(|data| data.value())
        .unwrap_or(0.0);
    let from_axis = trigger_value(gamepad.value(axis));
    let from_digital = if gamepad.is_pressed(button) { 1.0 } else { 0.0 };
    from_button.max(from_axis).max(from_digital).clamp(0.0, 1.0)
}

pub(crate) fn trigger_value(axis: f32) -> f32 {
    if axis < 0.0 {
        (axis + 1.0).clamp(0.0, 1.0)
    } else {
        axis.clamp(0.0, 1.0)
    }
}

pub(crate) fn mapping_targets() -> &'static [&'static str] {
    &[
        "Unmapped",
        "Xbox A",
        "Xbox B",
        "Xbox X",
        "Xbox Y",
        "Left Bumper",
        "Right Bumper",
        "Left Trigger",
        "Right Trigger",
        "D-pad Up",
        "D-pad Down",
        "D-pad Left",
        "D-pad Right",
        "Xbox Menu",
        "Xbox View",
        "Left Stick Click",
        "Right Stick Click",
    ]
}

pub(crate) fn mapped_button(binding: &str) -> u16 {
    match binding {
        "D-pad Up" | "D-pad up" => XButtons::UP,
        "D-pad Down" | "D-pad down" => XButtons::DOWN,
        "D-pad Left" | "D-pad left" => XButtons::LEFT,
        "D-pad Right" | "D-pad right" => XButtons::RIGHT,
        "Xbox A" | "Button 1" => XButtons::A,
        "Xbox B" | "Button 2" => XButtons::B,
        "Xbox X" | "Button 3" => XButtons::X,
        "Xbox Y" | "Button 4" => XButtons::Y,
        "Left Bumper" => XButtons::LB,
        "Right Bumper" => XButtons::RB,
        "Xbox Menu" | "Menu" => XButtons::START,
        "Xbox View" | "View" => XButtons::BACK,
        "Left Stick Click" | "Left Stick" => XButtons::LTHUMB,
        "Right Stick Click" | "Right Stick" => XButtons::RTHUMB,
        _ => 0,
    }
}

pub(crate) fn translate_controls(
    pressed: [bool; 18],
    left_trigger: f32,
    right_trigger: f32,
    bindings: &[String],
) -> XGamepad {
    let (mut buttons, mut output_left_trigger, mut output_right_trigger) = (0u16, 0.0f32, 0.0f32);
    for (index, is_pressed) in pressed.into_iter().enumerate() {
        if let Some(target) = bindings.get(index) {
            let pressed_value = if is_pressed { 1.0 } else { 0.0 };
            let value = match index {
                10 => left_trigger.clamp(0.0, 1.0),
                11 => right_trigger.clamp(0.0, 1.0),
                _ => pressed_value,
            };
            apply_mapping(
                target,
                value,
                &mut buttons,
                &mut output_left_trigger,
                &mut output_right_trigger,
            );
        }
    }
    XGamepad {
        buttons: XButtons::from(buttons),
        left_trigger: (output_left_trigger * 255.0).round() as u8,
        right_trigger: (output_right_trigger * 255.0).round() as u8,
        ..XGamepad::default()
    }
}

pub(crate) fn apply_mapping(
    target: &str,
    value: f32,
    buttons: &mut u16,
    left_trigger: &mut f32,
    right_trigger: &mut f32,
) {
    let value = value.clamp(0.0, 1.0);
    match target {
        "Left Trigger" => *left_trigger = left_trigger.max(value),
        "Right Trigger" => *right_trigger = right_trigger.max(value),
        "Unmapped" => {}
        _ if value >= 0.5 => *buttons |= mapped_button(target),
        _ => {}
    }
}

pub(crate) fn canonical_target(target: &str) -> String {
    match target {
        "Button 1" => "Xbox A",
        "Button 2" => "Xbox B",
        "Button 3" => "Xbox X",
        "Button 4" => "Xbox Y",
        "Menu" => "Xbox Menu",
        "View" => "Xbox View",
        "Left Stick" => "Left Stick Click",
        "Right Stick" => "Right Stick Click",
        other => other,
    }
    .to_owned()
}

pub(crate) fn migrate_profile_bindings(old: Vec<String>) -> Vec<String> {
    if old.len() == 14 {
        let mut migrated = default_bindings();
        for (index, binding) in old.iter().take(8).enumerate() {
            migrated[index] = canonical_target(binding);
        }
        migrated[10] = if old[8] == "Left Bumper" {
            "Left Trigger".into()
        } else {
            canonical_target(&old[8])
        };
        migrated[11] = if old[9] == "Right Bumper" {
            "Right Trigger".into()
        } else {
            canonical_target(&old[9])
        };
        migrated[12] = canonical_target(&old[10]);
        migrated[13] = canonical_target(&old[11]);
        migrated[14] = canonical_target(&old[12]);
        migrated[15] = canonical_target(&old[13]);
        return migrated;
    }

    let old_len = old.len();
    let mut bindings: Vec<String> = old.iter().map(|value| canonical_target(value)).collect();
    let defaults = default_bindings();

    // In the previous 17-control layout, index 16 was incorrectly labelled as
    // the touchpad even though gilrs::Mode actually reports the PS button.
    // Preserve any saved mapping at index 16 as the new touchpad mapping.
    if old_len == 17 {
        let previous_touchpad_binding = bindings[16].clone();
        bindings[16] = "Unmapped".into(); // New independent PS-button slot.
        bindings.push(previous_touchpad_binding);
    }

    bindings.resize_with(defaults.len(), String::new);
    for (index, binding) in bindings.iter_mut().enumerate() {
        if binding.is_empty() {
            *binding = defaults[index].clone();
        }
    }
    bindings
}

pub(crate) fn default_bindings() -> Vec<String> {
    [
        "D-pad Up",
        "D-pad Down",
        "D-pad Left",
        "D-pad Right",
        "Xbox A",
        "Xbox B",
        "Xbox X",
        "Xbox Y",
        "Left Bumper",
        "Right Bumper",
        "Left Trigger",
        "Right Trigger",
        "Xbox Menu",
        "Xbox View",
        "Left Stick Click",
        "Right Stick Click",
        "Unmapped", // PS button
        "Unmapped", // Touchpad click
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modern_face_buttons_map_to_xbox_layout() {
        assert_eq!(mapped_button("Xbox A"), XButtons::A);
        assert_eq!(mapped_button("Xbox B"), XButtons::B);
        assert_eq!(mapped_button("Xbox X"), XButtons::X);
        assert_eq!(mapped_button("Xbox Y"), XButtons::Y);
    }

    #[test]
    fn analog_triggers_map_independently_and_retain_pressure() {
        let mut pressed = [false; 18];
        pressed[10] = true;
        let bindings = super::default_bindings();
        let state = translate_controls(pressed, 0.42, 0.0, &bindings);
        assert_eq!(state.left_trigger, 107);
        assert_eq!(state.right_trigger, 0);

        let mut swapped = bindings;
        swapped[10] = "Right Trigger".into();
        let state = translate_controls(pressed, 0.42, 0.0, &swapped);
        assert_eq!(state.left_trigger, 0);
        assert_eq!(state.right_trigger, 107);
    }

    #[test]
    fn options_and_share_map_to_modern_menu_and_view_buttons() {
        let mut pressed = [false; 18];
        pressed[12] = true;
        pressed[13] = true;
        let state = translate_controls(pressed, 0.0, 0.0, &super::default_bindings());
        assert_eq!(
            state.buttons,
            XButtons::from(XButtons::START | XButtons::BACK)
        );
    }

    #[test]
    fn legacy_saved_layouts_migrate_to_the_eighteen_control_layout() {
        let legacy = vec![
            "D-pad Up",
            "D-pad Down",
            "D-pad Left",
            "D-pad Right",
            "Button 1",
            "Button 2",
            "Button 3",
            "Button 4",
            "Left Bumper",
            "Right Bumper",
            "Menu",
            "View",
            "Left Stick",
            "Right Stick",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        let migrated = migrate_profile_bindings(legacy);
        assert_eq!(migrated.len(), 18);
        assert_eq!(migrated[4], "Xbox A");
        assert_eq!(migrated[10], "Left Trigger");
        assert_eq!(migrated[11], "Right Trigger");
        assert_eq!(migrated[12], "Xbox Menu");
        assert_eq!(migrated[13], "Xbox View");
    }

    #[test]
    fn analog_trigger_values_are_normalized() {
        assert_eq!(trigger_value(-1.0), 0.0);
        assert_eq!(trigger_value(0.5), 0.5);
        assert_eq!(trigger_value(2.0), 1.0);
    }

    #[test]
    fn unmapped_controls_produce_neutral_output() {
        let state = translate_controls([true; 18], 1.0, 1.0, &vec!["Unmapped".into(); 18]);
        assert_eq!(state, XGamepad::default());
    }
}
