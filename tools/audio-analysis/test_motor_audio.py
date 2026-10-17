"""Synthetic fixtures check acoustic features, never real motor classifications."""

import csv
import importlib.machinery
import importlib.util
import json
import math
from pathlib import Path
import struct
import subprocess
import tempfile
import unittest
import wave

TOOL = Path(__file__).resolve().parent / "analyze-motor-audio"
LOADER = importlib.machinery.SourceFileLoader("motor_audio", str(TOOL))
SPEC = importlib.util.spec_from_loader(LOADER.name, LOADER)
AUDIO = importlib.util.module_from_spec(SPEC)
LOADER.exec_module(AUDIO)


class AudioAnalysisTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.directory = Path(self.temporary.name)

    def tearDown(self):
        self.temporary.cleanup()

    def wav(self, name, signal, width=2, channels=1, rate=16000, duration=0.3):
        path = self.directory / name
        scale = (1 << (width * 8 - 1)) - 1
        data = bytearray()
        for i in range(round(rate * duration)):
            for channel in range(channels):
                value = max(-1, min(1, signal(i / rate, channel)))
                data.extend(round(value * scale).to_bytes(width, "little", signed=True))
        with wave.open(str(path), "wb") as recording:
            recording.setnchannels(channels)
            recording.setsampwidth(width)
            recording.setframerate(rate)
            recording.writeframes(data)
        return path

    def test_pcm_widths_measure_same_tone_and_preserve_stereo_power(self):
        for width in (2, 3, 4):
            with self.subTest(bits=width * 8):
                path = self.wav(f"tone-{width}.wav", lambda t, c: (-1) ** c * 0.25 * math.sin(2 * math.pi * 1000 * t),
                                width=width, channels=2)
                report, rows = AUDIO.analyze(path)
                self.assertAlmostEqual(report["features"]["rms_dbfs"]["median"], -15.0515, delta=0.01)
                self.assertAlmostEqual(report["features"]["spectral_centroid_hz"]["median"], 1000, delta=1)
                self.assertGreater(rows[0]["peak_bin_fraction"], 0.2)
                self.assertLess(rows[0]["spectral_flatness"], 0.01)

    def test_tone_and_high_frequency_chatter_change_quiet_baseline(self):
        baseline_path = self.wav("quiet-tone.wav", lambda t, c: 0.001 * math.sin(2 * math.pi * 1000 * t))
        baseline, _ = AUDIO.analyze(baseline_path)
        loud = self.wav("loud-tone.wav", lambda t, c: 0.2 * math.sin(2 * math.pi * 1000 * t))
        report, rows = AUDIO.analyze(loud)
        AUDIO.compare(report, rows, baseline, 6)
        self.assertEqual(report["baseline_comparison"]["changed_fraction"], 1)
        self.assertGreater(rows[0]["rms_dbfs"] - baseline["features"]["rms_dbfs"]["median"], 40)
        reference_path = self.wav("reference.wav", lambda t, c: 0.1 * math.sin(2 * math.pi * 1000 * t))
        reference, _ = AUDIO.analyze(reference_path)
        chatter_path = self.wav("chatter.wav", lambda t, c: 0.07 * (math.sin(2 * math.pi * 1000 * t)
                                                                           + math.sin(2 * math.pi * 6000 * t)))
        chatter, rows = AUDIO.analyze(chatter_path)
        AUDIO.compare(chatter, rows, reference, 6)
        self.assertAlmostEqual(chatter["features"]["rms_dbfs"]["median"],
                               reference["features"]["rms_dbfs"]["median"], delta=0.2)
        self.assertGreater(chatter["features"]["high_frequency_fraction"]["median"], 0.45)
        self.assertEqual(chatter["baseline_comparison"]["changed_fraction"], 1)

    def test_silence_is_finite_unchanged_and_deterministic(self):
        path = self.wav("silent.wav", lambda t, c: 0, duration=0.101)
        baseline, _ = AUDIO.analyze(path)
        report, rows = AUDIO.analyze(path)
        AUDIO.compare(report, rows, baseline, 6)
        self.assertEqual(report["baseline_comparison"]["changed_windows"], 0)
        self.assertEqual(report["features"]["rms_dbfs"]["median"], -180)
        self.assertEqual(rows[-1]["end_seconds"], report["source"]["duration_seconds"])
        self.assertEqual(report["window_count"], 3)
        for row in rows:
            for value in row.values():
                if isinstance(value, (float, int)):
                    self.assertTrue(math.isfinite(value))
        self.assertEqual(AUDIO.analyze(path), AUDIO.analyze(path))
        json.dumps(report, allow_nan=False)

    def test_cli_writes_report_identities_and_relative_timestamps(self):
        path = self.wav("recording.wav", lambda t, c: 0.1 * math.sin(2 * math.pi * 500 * t))
        prefix = self.directory / "session" / "features"
        command = [str(TOOL), str(path), "--baseline", str(path), "--output", str(prefix)]
        completed = subprocess.run(command, capture_output=True, text=True, check=True)
        self.assertIn("no motor-load or safety classification", completed.stdout)
        first_json = prefix.with_suffix(".json").read_bytes()
        first_csv = prefix.with_suffix(".csv").read_bytes()
        report = json.loads(first_json)
        self.assertEqual(report["source"]["sha256"], report["baseline_comparison"]["source"]["sha256"])
        with prefix.with_suffix(".csv").open() as source:
            rows = list(csv.DictReader(source))
        self.assertEqual(rows[0]["source_sha256"], report["source"]["sha256"])
        self.assertEqual(float(rows[0]["start_seconds"]), 0)
        self.assertAlmostEqual(float(rows[-1]["end_seconds"]), 0.3)
        subprocess.run(command, capture_output=True, check=True)
        self.assertEqual(prefix.with_suffix(".json").read_bytes(), first_json)
        self.assertEqual(prefix.with_suffix(".csv").read_bytes(), first_csv)

    def test_rejects_bad_format_truncation_and_overlong_header(self):
        bad = self.directory / "not-wav.wav"
        bad.write_bytes(b"this is not a wave recording")
        with self.assertRaises(AUDIO.AudioError):
            AUDIO.analyze(bad)
        eight = self.wav("8-bit.wav", lambda t, c: 0, width=1)
        with self.assertRaisesRegex(AUDIO.AudioError, "16, 24, or 32"):
            AUDIO.analyze(eight)
        truncated = self.wav("truncated.wav", lambda t, c: 0)
        truncated.write_bytes(truncated.read_bytes()[:-40])
        with self.assertRaisesRegex(AUDIO.AudioError, "truncated"):
            AUDIO.analyze(truncated)
        overlong = self.directory / "overlong.wav"
        overlong.write_bytes(struct.pack("<4sI4s4sIHHIIHH4sI", b"RIFF", 20000036, b"WAVE", b"fmt ",
                                        16, 1, 1, 16000, 32000, 2, 16, b"data", 20000000))
        with self.assertRaisesRegex(AUDIO.AudioError, "600 seconds"):
            AUDIO.analyze(overlong)

    def test_cli_rejects_baseline_mismatch_and_nonfinite_threshold(self):
        path = self.wav("source.wav", lambda t, c: 0)
        baseline = self.wav("different.wav", lambda t, c: 0, rate=8000)
        for arguments in (["--baseline", str(baseline)], ["--threshold", "nan"]):
            completed = subprocess.run([str(TOOL), str(path), *arguments], capture_output=True, text=True)
            self.assertEqual(completed.returncode, 2)
            self.assertIn("analyze-motor-audio:", completed.stderr)
        self.assertFalse((self.directory / "source-motor-audio.json").exists())


if __name__ == "__main__":
    unittest.main()
