//! Display capability probe.
//!
//! Pills are colored only where color actually renders. Getting this wrong is
//! not cosmetic: a hue that a grayscale panel dithers into mid-gray is *less*
//! legible than the plain black-on-white pill it replaced, so the probe fails
//! toward monochrome whenever the answer is genuinely unclear.
//!
//! Build architecture settles most of it. armv7 is rM1/rM2, which have no color
//! panel at all, so no runtime probe can change the answer. aarch64 covers Paper
//! Pro and Paper Pro Move (color) *and* Paper Pro Pure (monochrome), so those
//! are separated by the device model string.

use std::path::Path;

/// Files that carry a device model string, most specific first. reMarkable has
/// moved this between kernels, so all three are tried.
const MODEL_PATHS: [&str; 3] = [
    "/sys/devices/soc0/machine",
    "/proc/device-tree/model",
    "/sys/firmware/devicetree/base/model",
];

/// Model-string markers for aarch64 devices whose panel is monochrome. Paper Pro
/// Pure is the one aarch64 device that cannot show color, so it is called out by
/// name rather than inferred.
const MONOCHROME_MARKERS: [&str; 2] = ["pure", "reMarkable 2"];

/// What the display can render, as resolved from config plus hardware.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayCapability {
    pub color: bool,
    /// Raw model string the probe read, or `""` when nothing was readable.
    /// Surfaced in diagnostics so an unrecognized device can be identified from
    /// a user's log rather than guessed at.
    pub model: String,
    /// How `color` was decided: `config`, `arch`, or `model`.
    pub source: &'static str,
}

/// Resolve display capability, honoring an explicit `[ui] color` override.
/// `auto` (the default) probes; `on` and `off` short-circuit it, which is the
/// escape hatch when the probe misjudges an unrecognized device.
pub fn capability(color_mode: &str) -> DisplayCapability {
    match color_mode.trim().to_ascii_lowercase().as_str() {
        "on" | "true" | "always" => DisplayCapability {
            color: true,
            model: read_model().unwrap_or_default(),
            source: "config",
        },
        "off" | "false" | "never" => DisplayCapability {
            color: false,
            model: read_model().unwrap_or_default(),
            source: "config",
        },
        _ => probe(),
    }
}

fn probe() -> DisplayCapability {
    let model = read_model().unwrap_or_default();

    // rM1/rM2 are the only armv7 targets and neither has a color panel, so the
    // build target is a complete answer — no model string required.
    if cfg!(target_arch = "arm") {
        return DisplayCapability {
            color: false,
            model,
            source: "arch",
        };
    }

    if is_monochrome_model(&model) {
        return DisplayCapability {
            color: false,
            model,
            source: "model",
        };
    }

    // Remaining aarch64 devices are the color Paper Pro family. The host build
    // (the AppLoad PC emulator) also lands here, which is deliberate: development
    // should exercise the colored path by default.
    DisplayCapability {
        color: true,
        model,
        source: if cfg!(target_arch = "aarch64") {
            "model"
        } else {
            "arch"
        },
    }
}

fn is_monochrome_model(model: &str) -> bool {
    if model.is_empty() {
        return false;
    }
    let lowered = model.to_ascii_lowercase();
    MONOCHROME_MARKERS
        .iter()
        .any(|marker| lowered.contains(&marker.to_ascii_lowercase()))
}

fn read_model() -> Option<String> {
    for path in MODEL_PATHS {
        let Ok(raw) = std::fs::read(Path::new(path)) else {
            continue;
        };
        // device-tree strings are NUL-terminated; sysfs ones are newline-terminated.
        let text = String::from_utf8_lossy(&raw);
        let trimmed = text.trim_matches(|c: char| c == '\0' || c.is_whitespace());
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_override_wins_over_hardware() {
        assert!(capability("on").color);
        assert_eq!(capability("on").source, "config");
        assert!(!capability("off").color);
        assert_eq!(capability("off").source, "config");
        // Case and surrounding whitespace are tolerated: the value is
        // hand-editable in config.toml.
        assert!(capability("  ON  ").color);
        assert!(!capability("Never").color);
    }

    #[test]
    fn auto_falls_through_to_the_probe() {
        // Whatever the host decides, `auto` must never report itself as a
        // config-sourced decision -- that distinction is what makes the
        // diagnostics line useful.
        assert_ne!(capability("auto").source, "config");
        assert_ne!(capability("").source, "config");
    }

    #[test]
    fn paper_pro_pure_is_monochrome_despite_aarch64() {
        assert!(is_monochrome_model("reMarkable Paper Pro Pure"));
        assert!(is_monochrome_model("remarkable ferrari pure"));
    }

    #[test]
    fn color_paper_pro_models_are_not_flagged_monochrome() {
        assert!(!is_monochrome_model("reMarkable Paper Pro"));
        assert!(!is_monochrome_model("reMarkable Paper Pro Move"));
    }

    #[test]
    fn unreadable_model_is_not_treated_as_monochrome() {
        // An empty model string means the probe found nothing, which must not
        // be confused with a positive match on a monochrome marker.
        assert!(!is_monochrome_model(""));
    }
}
