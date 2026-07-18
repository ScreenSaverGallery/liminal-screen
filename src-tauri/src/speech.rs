// Speech synthesis fallback for Linux.
//
// WebKitGTK does not implement the Web Speech API (window.speechSynthesis),
// so screensaver content that speaks on-screen text is silent on Linux. A JS
// polyfill (speech_polyfill.js, injected at document-start into saver and
// preview windows) forwards speak/cancel to the commands below, which drive
// `spd-say` (speech-dispatcher). On macOS/Windows the webview ships a native
// implementation, the polyfill steps aside at its feature check, and these
// commands are never called — they still exist there and answer
// "unsupported", so no platform has silently missing symbols.
//
// Linux runtime dependency: the `spd-say` binary (package: speech-dispatcher).

/// Web Speech API shim injected into windows that load saver content.
pub const POLYFILL_JS: &str = include_str!("speech_polyfill.js");

/// Map a Web Speech rate (0.1–10, 1 = normal) to spd-say -r (-100..100, 0 = normal).
/// Piecewise linear: 0.1 → -100, 1 → 0, 10 → 100.
pub fn web_rate_to_spd(rate: f64) -> i32 {
    let normalized = if rate >= 1.0 {
        (rate - 1.0) / 9.0
    } else {
        (rate - 1.0) / 0.9
    };
    (normalized * 100.0).round().clamp(-100.0, 100.0) as i32
}

/// Map a Web Speech pitch (0–2, 1 = normal) to spd-say -p (-100..100, 0 = normal).
pub fn web_pitch_to_spd(pitch: f64) -> i32 {
    ((pitch - 1.0) * 100.0).round().clamp(-100.0, 100.0) as i32
}

/// Map a Web Speech volume (0–1, 1 = full) to spd-say -i (-100..100, 100 = full).
pub fn web_volume_to_spd(volume: f64) -> i32 {
    (volume * 200.0 - 100.0).round().clamp(-100.0, 100.0) as i32
}

#[cfg(target_os = "linux")]
mod imp {
    use std::process::Command;

    pub fn supported() -> bool {
        Command::new("spd-say")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Blocks until the utterance has been spoken or cancelled (spd-say -w),
    /// so the JS polyfill can fire a truthful `end` event on resolution.
    /// Call from a blocking task, never on the main thread.
    pub fn speak(
        text: &str,
        rate: Option<f64>,
        pitch: Option<f64>,
        volume: Option<f64>,
        lang: Option<&str>,
    ) -> Result<(), String> {
        let mut cmd = Command::new("spd-say");
        cmd.arg("-w");
        if let Some(r) = rate {
            cmd.args(["-r", &super::web_rate_to_spd(r).to_string()]);
        }
        if let Some(p) = pitch {
            cmd.args(["-p", &super::web_pitch_to_spd(p).to_string()]);
        }
        if let Some(v) = volume {
            cmd.args(["-i", &super::web_volume_to_spd(v).to_string()]);
        }
        if let Some(l) = lang {
            if !l.is_empty() {
                cmd.args(["-l", l]);
            }
        }
        // Exec-style args, no shell involved; `--` stops option parsing so
        // page-supplied text can never be interpreted as spd-say options.
        cmd.arg("--").arg(text);
        run(cmd)
    }

    /// Flushes the speech-dispatcher message queue — matches the semantics of
    /// speechSynthesis.cancel() (drop current and all queued utterances).
    pub fn cancel() -> Result<(), String> {
        let mut cmd = Command::new("spd-say");
        cmd.arg("-C");
        run(cmd)
    }

    fn run(mut cmd: Command) -> Result<(), String> {
        let status = cmd
            .status()
            .map_err(|e| format!("Failed to run spd-say (is speech-dispatcher installed?): {}", e))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("spd-say exited with {}", status))
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod imp {
    // Fallback per AGENT.md: platform branches must never leave a silently
    // missing symbol. macOS/Windows webviews have native speechSynthesis, so
    // the polyfill never reaches these.
    pub fn supported() -> bool {
        false
    }

    pub fn speak(
        _text: &str,
        _rate: Option<f64>,
        _pitch: Option<f64>,
        _volume: Option<f64>,
        _lang: Option<&str>,
    ) -> Result<(), String> {
        Err("Speech synthesis fallback is only available on Linux".into())
    }

    pub fn cancel() -> Result<(), String> {
        Err("Speech synthesis fallback is only available on Linux".into())
    }
}

/// Command: runtime probe for the native TTS backend (spd-say on PATH).
#[tauri::command]
pub async fn speech_synthesis_supported() -> bool {
    tauri::async_runtime::spawn_blocking(imp::supported)
        .await
        .unwrap_or(false)
}

/// Command: speak `text`, resolving when the utterance has finished.
/// Optional rate/pitch/volume/lang follow Web Speech utterance semantics.
#[tauri::command]
pub async fn speak_text(
    text: String,
    rate: Option<f64>,
    pitch: Option<f64>,
    volume: Option<f64>,
    lang: Option<String>,
) -> Result<(), String> {
    // spd-say -w blocks for the length of the utterance; keep it off the
    // async runtime's core threads.
    tauri::async_runtime::spawn_blocking(move || {
        imp::speak(&text, rate, pitch, volume, lang.as_deref())
    })
    .await
    .map_err(|e| format!("Speech task failed: {}", e))?
}

/// Command: stop the current utterance and drop all queued ones.
#[tauri::command]
pub async fn cancel_speech() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(imp::cancel)
        .await
        .map_err(|e| format!("Speech task failed: {}", e))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_mapping_covers_web_speech_range() {
        assert_eq!(web_rate_to_spd(1.0), 0);
        assert_eq!(web_rate_to_spd(10.0), 100);
        assert_eq!(web_rate_to_spd(0.1), -100);
        assert_eq!(web_rate_to_spd(0.55), -50);
        assert_eq!(web_rate_to_spd(5.5), 50);
    }

    #[test]
    fn rate_mapping_clamps_out_of_range_values() {
        assert_eq!(web_rate_to_spd(100.0), 100);
        assert_eq!(web_rate_to_spd(0.0), -100);
        assert_eq!(web_rate_to_spd(-5.0), -100);
    }

    #[test]
    fn pitch_mapping_covers_web_speech_range() {
        assert_eq!(web_pitch_to_spd(1.0), 0);
        assert_eq!(web_pitch_to_spd(2.0), 100);
        assert_eq!(web_pitch_to_spd(0.0), -100);
        assert_eq!(web_pitch_to_spd(1.5), 50);
    }

    #[test]
    fn volume_mapping_covers_web_speech_range() {
        assert_eq!(web_volume_to_spd(1.0), 100);
        assert_eq!(web_volume_to_spd(0.0), -100);
        assert_eq!(web_volume_to_spd(0.5), 0);
        assert_eq!(web_volume_to_spd(2.0), 100);
    }

    #[test]
    fn polyfill_feature_detects_and_targets_the_commands() {
        // Guard against the shim and the commands drifting apart.
        assert!(POLYFILL_JS.contains("'speechSynthesis' in window"));
        assert!(POLYFILL_JS.contains("__TAURI_INTERNALS__"));
        assert!(POLYFILL_JS.contains("'speak_text'"));
        assert!(POLYFILL_JS.contains("'cancel_speech'"));
    }
}
