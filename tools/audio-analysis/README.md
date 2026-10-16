# Recorded motor audio

`../analyze-motor-audio` describes an existing WAV recording without opening a
microphone, playing audio, or controlling hardware. It uses Python 3.11+ and the
standard library. Supported inputs are mono/stereo signed integer PCM WAV files
with 16, 24, or 32 bits, 8–192 kHz, at most 600 seconds and 1 GiB.

```sh
tools/analyze-motor-audio recording.wav --output vendor-original/live-state/session/audio
tools/analyze-motor-audio recording.wav --baseline quiet.wav --output vendor-original/live-state/session/audio-comparison
python -m unittest discover -s tools/audio-analysis -p 'test_*.py'
```

The prefix produces a JSON summary and CSV with one row per 50 ms observation
window, recording-relative start/end times, and the input SHA-256. The final
window may be shorter. Both source identities are retained for comparisons.
Existing report files at that prefix are replaced. Use the same session folder
as the position/command baseline and keep a separate note of the recording's
start time, any clock offset, microphone placement, gain, and human observations.
WAV timestamps alone cannot synchronize separately acquired device telemetry.

RMS and peak levels use the original samples. Spectral features remove each
channel's mean, apply a Hann window, and average **channel powers**, so opposite
phase stereo does not cancel. The 2048-point FFT uses all samples when a window
is shorter than 2048 frames, with zero padding; otherwise it uses a centred
2048-frame excerpt. At 48 kHz the spectral excerpt lasts 42.7 ms. Band powers,
centroid, flatness, strongest-bin fraction and positive spectral flux describe
frequency content. Flatness and strongest-bin fraction are indicators of tonal
versus spread energy; they do not identify a particular mechanical fault. A band
above Nyquist has zero energy. Silent levels are floored at −180 dBFS and silent
spectral features are zero, keeping all reports finite.

A baseline must have the same rate and channel count. Each compared metric uses
the baseline median and `max(1.4826 × MAD, minimum_scale)`. Minimum scales are
3 dB for RMS, 50 Hz for centroid, 0.02 for the fraction above 4 kHz, and 0.05 for
flatness and strongest-bin fraction. The largest standardized deviation is the
change score. RMS and high-frequency fraction flag increases; other features
use either direction. A window is marked changed when its score reaches the
threshold (default 6) and RMS exceeds −100 dBFS. These are explicit exploratory
heuristics, with no calibrated probability or learned classifier. Review the
continuous features, especially when a quiet baseline has zero MAD.

**A changed sound is not a measurement of motor labour, torque, overload,
temperature, or safe force.** Speech, cooling fans, handling, automatic gain,
room acoustics and microphone motion can produce the same changes. Human labels
and independently measured operating conditions are needed before evaluating
any load classifier. This report must not make force-enable or shutdown decisions.
