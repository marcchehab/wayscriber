use log::{info, warn};
use std::env;

use crate::config::Config;
use crate::env_vars::{
    DESKTOP_SESSION_ENV, HYPRLAND_INSTANCE_SIGNATURE_ENV, MAIN_LAYER_ENV, NIRI_SOCKET_ENV,
    SWAYSOCK_ENV, XDG_CURRENT_DESKTOP_ENV, XDG_FULLSCREEN_ENV, XDG_FULLSCREEN_FORCE_ENV,
    XDG_OUTPUT_ENV, XDG_SESSION_DESKTOP_ENV,
};

pub(super) struct OutputPreferences {
    pub(super) preferred_output_identity: Option<String>,
    pub(super) xdg_fullscreen: bool,
    pub(super) main_surface_uses_overlay_layer: bool,
}

pub(super) fn resolve(config: &Config) -> OutputPreferences {
    let preferred_output_identity = env::var(XDG_OUTPUT_ENV)
        .ok()
        .or_else(|| config.ui.preferred_output.clone());
    if let Some(ref output) = preferred_output_identity {
        info!(
            "Preferring xdg fullscreen on output '{}' (env or config override)",
            output
        );
    }

    let mut xdg_fullscreen = env::var(XDG_FULLSCREEN_ENV)
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(config.ui.xdg_fullscreen);
    let desktop_env = env::var(XDG_CURRENT_DESKTOP_ENV).unwrap_or_default();
    let session_env = env::var(XDG_SESSION_DESKTOP_ENV).unwrap_or_default();
    let desktop_session = env::var(DESKTOP_SESSION_ENV).unwrap_or_default();
    let sway_sock = env::var(SWAYSOCK_ENV).unwrap_or_default();
    let niri_socket = env::var(NIRI_SOCKET_ENV).unwrap_or_default();
    let hyprland_signature = env::var(HYPRLAND_INSTANCE_SIGNATURE_ENV).unwrap_or_default();
    let force_fullscreen = env::var(XDG_FULLSCREEN_FORCE_ENV)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if xdg_fullscreen && desktop_env.to_uppercase().contains("GNOME") && !force_fullscreen {
        warn!(
            "GNOME fullscreen xdg fallback is opaque; falling back to maximized. Set {XDG_FULLSCREEN_FORCE_ENV}=1 to force fullscreen anyway."
        );
        xdg_fullscreen = false;
    }
    let main_layer_override = env::var(MAIN_LAYER_ENV).ok();
    let main_surface_uses_overlay_layer =
        match parse_main_layer_override(main_layer_override.as_deref()) {
            Some(overlay) => {
                info!("Main surface layer forced via {MAIN_LAYER_ENV}");
                overlay
            }
            None => {
                if let Some(value) = main_layer_override.as_deref() {
                    warn!("Ignoring {MAIN_LAYER_ENV}='{value}'; expected 'overlay' or 'top'");
                }
                main_surface_uses_overlay_layer_with_env(
                    &desktop_env,
                    &session_env,
                    &desktop_session,
                    &sway_sock,
                    &niri_socket,
                    &hyprland_signature,
                )
            }
        };
    if main_surface_uses_overlay_layer {
        info!(
            "Compositor requires fullscreen overlays above normal top-layer surfaces; mapping the main overlay surface in the overlay layer so fullscreen windows cannot cover Wayscriber"
        );
    }

    OutputPreferences {
        preferred_output_identity,
        xdg_fullscreen,
        main_surface_uses_overlay_layer,
    }
}

fn main_surface_uses_overlay_layer_with_env(
    desktop_env: &str,
    session_env: &str,
    desktop_session: &str,
    sway_sock: &str,
    niri_socket: &str,
    hyprland_signature: &str,
) -> bool {
    desktop_matches_any(desktop_env)
        || desktop_matches_any(session_env)
        || desktop_matches_any(desktop_session)
        || !sway_sock.trim().is_empty()
        || !niri_socket.trim().is_empty()
        || !hyprland_signature.trim().is_empty()
}

/// `Some(true)` forces the overlay layer, `Some(false)` the top layer,
/// `None` keeps compositor detection.
fn parse_main_layer_override(value: Option<&str>) -> Option<bool> {
    match value?.trim().to_ascii_lowercase().as_str() {
        "overlay" => Some(true),
        "top" => Some(false),
        _ => None,
    }
}

fn desktop_matches_any(value: &str) -> bool {
    ["niri", "sway", "hyprland"]
        .iter()
        .any(|target| desktop_matches(value, target))
}

fn desktop_matches(value: &str, target: &str) -> bool {
    value
        .split(':')
        .map(str::trim)
        .any(|entry| entry.eq_ignore_ascii_case(target))
}

#[cfg(test)]
mod tests {
    use super::{main_surface_uses_overlay_layer_with_env, parse_main_layer_override};

    #[test]
    fn main_surface_uses_overlay_layer_for_niri_desktop() {
        assert!(main_surface_uses_overlay_layer_with_env(
            "niri", "", "", "", "", ""
        ));
        assert!(main_surface_uses_overlay_layer_with_env(
            "Hyprland:Niri",
            "",
            "",
            "",
            "",
            ""
        ));
    }

    #[test]
    fn main_surface_uses_overlay_layer_for_niri_session() {
        assert!(main_surface_uses_overlay_layer_with_env(
            "", "NIRI", "", "", "", ""
        ));
    }

    #[test]
    fn main_surface_uses_overlay_layer_for_niri_desktop_session() {
        assert!(main_surface_uses_overlay_layer_with_env(
            "", "", "niri", "", "", ""
        ));
    }

    #[test]
    fn main_surface_uses_overlay_layer_for_niri_socket() {
        assert!(main_surface_uses_overlay_layer_with_env(
            "",
            "",
            "",
            "",
            "/run/user/1000/niri.wayland-1.1234.sock",
            ""
        ));
    }

    #[test]
    fn main_surface_uses_overlay_layer_for_sway_desktop() {
        assert!(main_surface_uses_overlay_layer_with_env(
            "sway", "", "", "", "", ""
        ));
        assert!(main_surface_uses_overlay_layer_with_env(
            "wlroots:Sway",
            "",
            "",
            "",
            "",
            ""
        ));
    }

    #[test]
    fn main_surface_uses_overlay_layer_for_sway_session() {
        assert!(main_surface_uses_overlay_layer_with_env(
            "", "SWAY", "", "", "", ""
        ));
        assert!(main_surface_uses_overlay_layer_with_env(
            "", "", "sway", "", "", ""
        ));
    }

    #[test]
    fn main_surface_uses_overlay_layer_for_sway_socket() {
        assert!(main_surface_uses_overlay_layer_with_env(
            "",
            "",
            "",
            "/run/user/1000/sway-ipc.sock",
            "",
            ""
        ));
    }

    #[test]
    fn main_surface_uses_overlay_layer_for_hyprland() {
        assert!(main_surface_uses_overlay_layer_with_env(
            "Hyprland", "", "", "", "", ""
        ));
        assert!(main_surface_uses_overlay_layer_with_env(
            "",
            "",
            "",
            "",
            "",
            "abc_123_456"
        ));
    }

    #[test]
    fn main_layer_override_parses_overlay_and_top() {
        assert_eq!(parse_main_layer_override(Some("overlay")), Some(true));
        assert_eq!(parse_main_layer_override(Some(" TOP ")), Some(false));
        assert_eq!(parse_main_layer_override(Some("bogus")), None);
        assert_eq!(parse_main_layer_override(None), None);
    }

    #[test]
    fn main_surface_stays_on_top_layer_for_other_desktops() {
        assert!(!main_surface_uses_overlay_layer_with_env(
            "KDE", "plasma", "", "", "", ""
        ));
    }
}
