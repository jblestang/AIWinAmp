use anyhow::{anyhow, Context, Result};
use std::fs::File;
use std::path::Path;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, MetadataRevision, StandardTagKey, Tag};
use symphonia::core::probe::Hint;

/// Metadata extracted from an audio file.
#[derive(Debug, Default, Clone)]
pub struct MediaMetadata {
    /// Optional title identified in file tags.
    pub title: Option<String>,
    /// Optional artist or album-artist tag.
    pub artist: Option<String>,
    /// Duration in seconds if derivable from the encoded frames.
    pub duration_secs: Option<u32>,
}

/// Attempt to parse common audio formats (mp3/flac/ogg/wav/aac) for metadata.
pub fn analyze_audio(path: &Path) -> Result<MediaMetadata> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let format_opts = FormatOptions::default();
    let metadata_opts = MetadataOptions::default();
    let mut probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &metadata_opts)
        .context("unsupported audio container")?;

    let mut format = probed.format;
    let mut extra_metadata = probed.metadata.get();
    let mut metadata = merge_metadata(
        format.metadata().current(),
        extra_metadata.as_mut().and_then(|m| m.current()),
    );

    let track = format
        .default_track()
        .ok_or_else(|| anyhow!("no playable tracks"))?;

    if metadata.duration_secs.is_none() {
        let duration = if let (Some(tb), Some(n_frames)) =
            (track.codec_params.time_base, track.codec_params.n_frames)
        {
            let time = tb.calc_time(n_frames);
            Some(((time.seconds as f64) + time.frac).ceil() as u32)
        } else if let (Some(sr), Some(n_frames)) =
            (track.codec_params.sample_rate, track.codec_params.n_frames)
        {
            if sr > 0 {
                Some((n_frames as f64 / sr as f64).ceil() as u32)
            } else {
                None
            }
        } else {
            None
        };
        if let Some(secs) = duration {
            metadata.duration_secs = Some(secs.max(1));
        }
    }

    Ok(metadata)
}

fn merge_metadata(
    primary: Option<&MetadataRevision>,
    secondary: Option<&MetadataRevision>,
) -> MediaMetadata {
    let mut meta = MediaMetadata::default();
    for revision in primary.into_iter().chain(secondary.into_iter()) {
        for tag in revision.tags() {
            attach_tag(&mut meta, tag);
        }
    }
    meta
}

fn attach_tag(meta: &mut MediaMetadata, tag: &Tag) {
    let lowercase = tag.key.to_string().to_lowercase();
    let value = tag.value.to_string();
    let inferred_key = tag.std_key.or_else(|| match lowercase.as_str() {
        "title" | "tracktitle" => Some(StandardTagKey::TrackTitle),
        "artist" => Some(StandardTagKey::Artist),
        "albumartist" => Some(StandardTagKey::AlbumArtist),
        _ => None,
    });
    match inferred_key {
        Some(StandardTagKey::TrackTitle) => {
            if meta.title.is_none() {
                meta.title = Some(value);
            }
        }
        Some(StandardTagKey::Artist) | Some(StandardTagKey::AlbumArtist) => {
            if meta.artist.is_none() {
                meta.artist = Some(value);
            }
        }
        _ => {}
    }
}
