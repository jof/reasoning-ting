#!/usr/bin/env python3
"""Generate the Quindar (and submit) tone WAV files for the TING.

Background
----------
"Quindar tones" are the short beeps you hear bracketing transmissions in
historical NASA mission audio. The original Quindar system used two
frequencies as out-of-band signalling: an *intro* tone of 2525 Hz that meant
"the microphone has been keyed (transmission starting)" and an *outro* tone of
2475 Hz that meant "the microphone has been released (transmission ending)".
We reproduce that convention here so the TING's start/stop events sound the
part.

Why this script exists
-----------------------
Rather than ship binary audio assets in the repository, we generate them
deterministically from code. That keeps the WAVs reviewable (the recipe is
right here), reproducible, and tiny. Each tone is a brief sine-wave burst with
raised-cosine fades at both ends so it begins and ends smoothly instead of
clicking — an abrupt start or stop of a waveform produces an audible "click"
because of the instantaneous jump in amplitude.

Output format
-------------
Mono, 16-bit PCM, 48 kHz. At a fifth of a second per tone these are only a few
kilobytes each, comfortably within the project's ~1 MB asset budget.
"""

import math
import os
import struct
import wave

# --- Audio parameters shared by every tone -------------------------------

# Sample rate in samples per second (48 kHz is the standard for this project).
SAMPLE_RATE_HZ = 48000

# How long each tone lasts, in seconds. 200 ms is a short, recognizable burst.
TONE_DURATION_SECONDS = 0.20

# Length of the fade-in and fade-out, in seconds. A 10 ms raised-cosine ramp at
# each end is enough to eliminate clicks without audibly softening the tone.
FADE_DURATION_SECONDS = 0.010

# Peak amplitude as a fraction of full scale (0.0 silent .. 1.0 maximum).
# 0.6 leaves headroom so the tone is clearly audible but never clips.
PEAK_AMPLITUDE = 0.6

# Largest value representable by a signed 16-bit PCM sample. We scale our
# floating-point waveform (which lives in the range -1.0 .. +1.0) by this to
# fill the 16-bit range before packing.
MAX_INT16 = 32767


def write_tone_wav(output_path, frequency_hz):
    """Synthesize one faded sine-wave tone and write it as a WAV file.

    Args:
        output_path: Filesystem path the .wav file is written to.
        frequency_hz: Pitch of the sine wave, in hertz.
    """

    # Total number of audio samples in the whole tone, and the number of
    # samples covered by each fade ramp at the start and end.
    total_sample_count = int(SAMPLE_RATE_HZ * TONE_DURATION_SECONDS)
    fade_sample_count = int(SAMPLE_RATE_HZ * FADE_DURATION_SECONDS)

    # We accumulate the raw little-endian 16-bit samples here.
    pcm_frames = bytearray()

    for sample_index in range(total_sample_count):
        # The amplitude envelope scales the waveform from 0 (silent) up to 1
        # (full PEAK_AMPLITUDE). It is 1.0 across the sustained middle of the
        # tone and ramps smoothly via a raised cosine during the fades.
        envelope = 1.0

        if sample_index < fade_sample_count:
            # Fade-in: rises 0 -> 1 over the first fade_sample_count samples.
            fade_progress = math.pi * sample_index / fade_sample_count
            envelope = 0.5 - 0.5 * math.cos(fade_progress)
        elif sample_index > total_sample_count - fade_sample_count:
            # Fade-out: falls 1 -> 0 over the last fade_sample_count samples.
            samples_remaining = total_sample_count - sample_index
            fade_progress = math.pi * samples_remaining / fade_sample_count
            envelope = 0.5 - 0.5 * math.cos(fade_progress)

        # The instantaneous phase of the sine wave at this sample, then the
        # enveloped waveform value in the floating-point range -1.0 .. +1.0.
        phase_radians = 2 * math.pi * frequency_hz * sample_index / SAMPLE_RATE_HZ
        sample_value = PEAK_AMPLITUDE * envelope * math.sin(phase_radians)

        # Convert the float sample to a signed 16-bit integer and append it as
        # a little-endian ("<h") frame.
        sample_int16 = int(sample_value * MAX_INT16)
        pcm_frames += struct.pack("<h", sample_int16)

    # Write the assembled samples out as a mono, 16-bit, 48 kHz WAV file.
    wav_file = wave.open(output_path, "wb")
    wav_file.setnchannels(1)        # mono
    wav_file.setsampwidth(2)        # 2 bytes == 16 bits per sample
    wav_file.setframerate(SAMPLE_RATE_HZ)
    wav_file.writeframes(pcm_frames)
    wav_file.close()

    file_size_bytes = os.path.getsize(output_path)
    duration_milliseconds = TONE_DURATION_SECONDS * 1000
    print(f"{output_path}: {frequency_hz} Hz, {duration_milliseconds:.0f} ms, {file_size_bytes} bytes")


# --- Generate the three tones the TING needs -----------------------------

# Resolve paths relative to this script so it works regardless of the caller's
# current working directory.
script_directory = os.path.dirname(os.path.abspath(__file__))

# Classic Quindar intro tone: microphone keyed / start of transmission.
write_tone_wav(os.path.join(script_directory, "quindar_in.wav"), 2525)

# Classic Quindar outro tone: microphone released / end of transmission.
write_tone_wav(os.path.join(script_directory, "quindar_out.wav"), 2475)

# Distinct, higher confirmation tone for the white "submit" button (-> Enter).
write_tone_wav(os.path.join(script_directory, "submit.wav"), 3000)
