#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoudnessProfile {
    pub target_lufs: f64,
    pub good_min_lufs: f64,
    pub good_max_lufs: f64,
    pub attention_min_lufs: f64,
    pub attention_max_lufs: f64,
    pub sparkline_min: f64,
    pub sparkline_max: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TruePeakProfile {
    pub ceiling_dbtp: f64,
    pub attention_max_dbtp: f64,
    pub sparkline_min: f64,
    pub sparkline_max: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SilenceProfile {
    pub leading_good_max_seconds: f64,
    pub leading_attention_max_seconds: f64,
    pub trailing_good_max_seconds: f64,
    pub trailing_attention_max_seconds: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PodcastProfile {
    pub id: &'static str,
    pub version: &'static str,
    pub name: &'static str,
    pub is_mono: bool,
    pub loudness: LoudnessProfile,
    pub true_peak: TruePeakProfile,
    pub silence: SilenceProfile,
}

pub static PODCAST_STEREO_V1: PodcastProfile = PodcastProfile {
    id: "podcast-stereo-v1",
    version: "1.0.0",
    name: "Podcast — Stereo",
    is_mono: false,
    loudness: LoudnessProfile {
        target_lufs: -16.0,
        good_min_lufs: -17.5,
        good_max_lufs: -14.5,
        attention_min_lufs: -20.0,
        attention_max_lufs: -13.0,
        sparkline_min: -30.0,
        sparkline_max: -10.0,
    },
    true_peak: TruePeakProfile {
        ceiling_dbtp: -1.5,
        attention_max_dbtp: -0.5,
        sparkline_min: -6.0,
        sparkline_max: 0.0,
    },
    silence: SilenceProfile {
        leading_good_max_seconds: 2.0,
        leading_attention_max_seconds: 5.0,
        trailing_good_max_seconds: 4.0,
        trailing_attention_max_seconds: 8.0,
    },
};

pub static PODCAST_MONO_V1: PodcastProfile = PodcastProfile {
    id: "podcast-mono-v1",
    version: "1.0.0",
    name: "Podcast — Mono",
    is_mono: true,
    loudness: LoudnessProfile {
        target_lufs: -19.0,
        good_min_lufs: -20.5,
        good_max_lufs: -17.5,
        attention_min_lufs: -23.0,
        attention_max_lufs: -16.0,
        sparkline_min: -33.0,
        sparkline_max: -13.0,
    },
    true_peak: TruePeakProfile {
        ceiling_dbtp: -1.5,
        attention_max_dbtp: -0.5,
        sparkline_min: -6.0,
        sparkline_max: 0.0,
    },
    silence: SilenceProfile {
        leading_good_max_seconds: 2.0,
        leading_attention_max_seconds: 5.0,
        trailing_good_max_seconds: 4.0,
        trailing_attention_max_seconds: 8.0,
    },
};

pub fn get_profile_for_channels(channels: u32) -> &'static PodcastProfile {
    if channels == 1 {
        &PODCAST_MONO_V1
    } else {
        &PODCAST_STEREO_V1
    }
}
