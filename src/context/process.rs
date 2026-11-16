//! A context passed during the process function.

use super::PluginApi;
use crate::prelude::{Plugin, PluginNoteEvent};

/// Contains both context data and callbacks the plugin can use during processing. Most notably this
/// is how a plugin sends and receives note events, gets transport information, and accesses
/// sidechain inputs and auxiliary outputs. This is passed to the plugin during as part of
/// [`Plugin::process()`][crate::plugin::Plugin::process()].
//
// # Safety
//
// The implementing wrapper needs to be able to handle concurrent requests, and it should perform
// the actual callback within [MainThreadQueue::schedule_gui].
pub trait ProcessContext<P: Plugin> {
    /// Get the current plugin API.
    fn plugin_api(&self) -> PluginApi;

    /// Execute a task on a background thread using `[Plugin::task_executor]`. This allows you to
    /// defer expensive tasks for later without blocking either the process function or the GUI
    /// thread. As long as creating the `task` is realtime-safe, this operation is too.
    ///
    /// # Note
    ///
    /// Scheduling the same task multiple times will cause those duplicate tasks to pile up. Try to
    /// either prevent this from happening, or check whether the task still needs to be completed in
    /// your task executor.
    fn execute_background(&self, task: P::BackgroundTask);

    /// Execute a task on a background thread using `[Plugin::task_executor]`. As long as creating
    /// the `task` is realtime-safe, this operation is too.
    ///
    /// # Note
    ///
    /// Scheduling the same task multiple times will cause those duplicate tasks to pile up. Try to
    /// either prevent this from happening, or check whether the task still needs to be completed in
    /// your task executor.
    fn execute_gui(&self, task: P::BackgroundTask);

    /// Get information about the current transport position and status.
    fn transport(&self) -> &Transport;

    /// Returns the next note event, if there is one. Use
    /// [`NoteEvent::timing()`][crate::prelude::NoteEvent::timing()] to get the event's timing
    /// within the buffer. Only available when
    /// [`Plugin::MIDI_INPUT`][crate::prelude::Plugin::MIDI_INPUT] is set.
    ///
    /// # Usage
    ///
    /// You will likely want to use this with a loop, since there may be zero, one, or more events
    /// for a sample:
    ///
    /// ```ignore
    /// let mut next_event = context.next_event();
    /// for (sample_id, channel_samples) in buffer.iter_samples().enumerate() {
    ///     while let Some(event) = next_event {
    ///         if event.timing() != sample_id as u32 {
    ///             break;
    ///         }
    ///
    ///         match event {
    ///             NoteEvent::NoteOn { note, velocity, .. } => { ... },
    ///             NoteEvent::NoteOff { note, .. } if note == 69 => { ... },
    ///             NoteEvent::PolyPressure { note, pressure, .. } { ... },
    ///             _ => (),
    ///         }
    ///
    ///         next_event = context.next_event();
    ///     }
    ///
    ///     // Do something with `channel_samples`...
    /// }
    ///
    /// ProcessStatus::Normal
    /// ```
    fn next_event(&mut self) -> Option<PluginNoteEvent<P>>;

    /// Send an event to the host. Only available when
    /// [`Plugin::MIDI_OUTPUT`][crate::prelude::Plugin::MIDI_INPUT] is set. Will not do anything
    /// otherwise.
    fn send_event(&mut self, event: PluginNoteEvent<P>);

    /// Update the current latency of the plugin. If the plugin is currently processing audio, then
    /// this may cause audio playback to be restarted.
    fn set_latency_samples(&self, samples: u32);

    /// Set the current voice **capacity** for this plugin (so not the number of currently active
    /// voices). This may only be called if
    /// [`ClapPlugin::CLAP_POLY_MODULATION_CONFIG`][crate::prelude::ClapPlugin::CLAP_POLY_MODULATION_CONFIG]
    /// is set. `capacity` must be between 1 and the configured maximum capacity. Changing this at
    /// runtime allows the host to better optimize polyphonic modulation, or to switch to strictly
    /// monophonic modulation when dropping the capacity down to 1.
    fn set_current_voice_capacity(&self, capacity: u32);

    // TODO: Add this, this works similar to [GuiContext::set_parameter] but it adds the parameter
    //       change to a queue (or directly to the VST3 plugin's parameter output queues) instead of
    //       using main thread host automation (and all the locks involved there).
    // fn set_parameter<P: Param>(&self, param: &P, value: P::Plain);
}

/// Information about the plugin's transport. Depending on the plugin API and the host not all
/// fields may be available.
#[derive(Debug)]
pub struct Transport {
    /// Whether the transport is currently running.
    pub playing: bool,
    /// Whether recording is enabled in the project.
    pub recording: bool,
    /// Whether the pre-roll is currently active, if the plugin API reports this information.
    pub preroll_active: Option<bool>,

    /// The sample rate in Hertz. Also passed in
    /// [`Plugin::initialize()`][crate::prelude::Plugin::initialize()], so if you need this then you
    /// can also store that value.
    pub sample_rate: f32,
    /// The project's tempo in beats per minute.
    pub tempo: Option<f64>,
    /// The time signature's numerator.
    pub time_sig_numerator: Option<i32>,
    /// The time signature's denominator.
    pub time_sig_denominator: Option<i32>,

    // XXX: VST3 also has a continuous time in samples that ignores loops, but we can't reconstruct
    //      something similar in CLAP so it may be best to just ignore that so you can't rely on it
    /// The position in the song in samples. Can be used to calculate the time in seconds if needed.
    pub(crate) pos_samples: Option<i64>,
    /// The position in the song in seconds. Can be used to calculate the time in samples if needed.
    pub(crate) pos_seconds: Option<f64>,
    /// The position in the song in quarter notes. Can be calculated from the time in seconds and
    /// the tempo if needed.
    pub(crate) pos_beats: Option<f64>,
    /// The last bar's start position in beats. Can be calculated from the beat position and time
    /// signature if needed.
    pub(crate) bar_start_pos_beats: Option<f64>,
    /// The number of the bar at `bar_start_pos_beats`. This starts at 0 for the very first bar at
    /// the start of the song. Can be calculated from the beat position and time signature if
    /// needed.
    pub(crate) bar_number: Option<i32>,

    /// The loop range in samples, if the loop is active and this information is available. None of
    /// the plugin API docs mention whether this is exclusive or inclusive, but just assume that the
    /// end is exclusive. Can be calculated from the other loop range information if needed.
    pub(crate) loop_range_samples: Option<(i64, i64)>,
    /// The loop range in seconds, if the loop is active and this information is available. None of
    /// the plugin API docs mention whether this is exclusive or inclusive, but just assume that the
    /// end is exclusive. Can be calculated from the other loop range information if needed.
    pub(crate) loop_range_seconds: Option<(f64, f64)>,
    /// The loop range in quarter notes, if the loop is active and this information is available.
    /// None of the plugin API docs mention whether this is exclusive or inclusive, but just assume
    /// that the end is exclusive. Can be calculated from the other loop range information if
    /// needed.
    pub(crate) loop_range_beats: Option<(f64, f64)>,
}

impl Transport {
    /// Project host coordinates to a wrapper-created sub-block without moving a stopped transport.
    pub(crate) fn offset_samples(&mut self, offset: usize) {
        if offset == 0 || !self.playing || !self.sample_rate.is_finite() || self.sample_rate <= 0.0
        {
            return;
        }
        let sample_loop = self.loop_range_samples();
        let second_loop = self.loop_range_seconds();
        let beat_loop = self.loop_range_beats();
        let old_beats = self.pos_beats().filter(|value| value.is_finite());
        let old_bar = self.bar_start_pos_beats().filter(|value| value.is_finite());
        let seconds = offset as f64 / self.sample_rate as f64;

        if let Some(position) = self.pos_samples {
            let mut next = position as i128 + offset as i128;
            if let Some((start, end)) = sample_loop {
                if end > start && position <= end && next >= end as i128 {
                    next = start as i128
                        + (next - end as i128).rem_euclid(end as i128 - start as i128);
                }
            }
            self.pos_samples = Some(next.clamp(i64::MIN as i128, i64::MAX as i128) as i64);
        }
        if let Some(position) = self.pos_seconds {
            self.pos_seconds = Some(offset_position(position, seconds, second_loop));
        }
        if let (Some(position), Some(tempo)) = (self.pos_beats, self.tempo) {
            if tempo.is_finite() && tempo > 0.0 {
                self.pos_beats = Some(offset_position(position, seconds / 60.0 * tempo, beat_loop));
            }
        }

        // Keep the host's bar origin/number, including meter changes before this block.
        if let (Some(previous), Some(next), Some(start), Some(numerator), Some(denominator)) = (
            old_beats,
            self.pos_beats().filter(|value| value.is_finite()),
            old_bar,
            self.time_sig_numerator,
            self.time_sig_denominator,
        ) {
            if previous != next && numerator > 0 && denominator > 0 {
                let length = numerator as f64 / denominator as f64 * 4.0;
                let bars = ((next - start) / length).floor();
                let bar_start = start + bars * length;
                if bars.is_finite() && bar_start.is_finite() {
                    if self.bar_start_pos_beats.is_some() {
                        self.bar_start_pos_beats = Some(bar_start);
                    }
                    if let Some(number) = self.bar_number {
                        self.bar_number = Some(
                            (number as f64 + bars).clamp(i32::MIN as f64, i32::MAX as f64) as i32,
                        );
                    }
                }
            }
        }
    }

    /// Initialize the transport struct without any information.
    pub(crate) fn new(sample_rate: f32) -> Self {
        Self {
            playing: false,
            recording: false,
            preroll_active: None,

            sample_rate,
            tempo: None,
            time_sig_numerator: None,
            time_sig_denominator: None,

            pos_samples: None,
            pos_seconds: None,
            pos_beats: None,
            bar_start_pos_beats: None,
            bar_number: None,

            loop_range_samples: None,
            loop_range_seconds: None,
            loop_range_beats: None,
        }
    }

    /// The position in the song in samples. Will be calculated from other information if needed.
    pub fn pos_samples(&self) -> Option<i64> {
        match (
            self.pos_samples,
            self.pos_seconds,
            self.pos_beats,
            self.tempo,
        ) {
            (Some(pos_samples), _, _, _) => Some(pos_samples),
            (_, Some(pos_seconds), _, _) => {
                Some((pos_seconds * self.sample_rate as f64).round() as i64)
            }
            (_, _, Some(pos_beats), Some(tempo)) => {
                Some((pos_beats / tempo * 60.0 * self.sample_rate as f64).round() as i64)
            }
            (_, _, _, _) => None,
        }
    }

    /// The position in the song in seconds. Can be used to calculate the time in samples if needed.
    pub fn pos_seconds(&self) -> Option<f64> {
        match (
            self.pos_samples,
            self.pos_seconds,
            self.pos_beats,
            self.tempo,
        ) {
            (_, Some(pos_seconds), _, _) => Some(pos_seconds),
            (Some(pos_samples), _, _, _) => Some(pos_samples as f64 / self.sample_rate as f64),
            (_, _, Some(pos_beats), Some(tempo)) => Some(pos_beats / tempo * 60.0),
            (_, _, _, _) => None,
        }
    }

    /// The position in the song in quarter notes. Will be calculated from other information if
    /// needed.
    pub fn pos_beats(&self) -> Option<f64> {
        match (
            self.pos_samples,
            self.pos_seconds,
            self.pos_beats,
            self.tempo,
        ) {
            (_, _, Some(pos_beats), _) => Some(pos_beats),
            (_, Some(pos_seconds), _, Some(tempo)) => Some(pos_seconds / 60.0 * tempo),
            (Some(pos_samples), _, _, Some(tempo)) => {
                Some(pos_samples as f64 / self.sample_rate as f64 / 60.0 * tempo)
            }
            (_, _, _, _) => None,
        }
    }

    /// The last bar's start position in beats. Will be calculated from other information if needed.
    pub fn bar_start_pos_beats(&self) -> Option<f64> {
        if self.bar_start_pos_beats.is_some() {
            return self.bar_start_pos_beats;
        }

        match (
            self.time_sig_numerator,
            self.time_sig_denominator,
            self.pos_beats(),
        ) {
            (Some(time_sig_numerator), Some(time_sig_denominator), Some(pos_beats)) => {
                let quarter_note_bar_length =
                    time_sig_numerator as f64 / time_sig_denominator as f64 * 4.0;
                Some((pos_beats / quarter_note_bar_length).floor() * quarter_note_bar_length)
            }
            (_, _, _) => None,
        }
    }

    /// The number of the bar at `bar_start_pos_beats`. This starts at 0 for the very first bar at
    /// the start of the song. Will be calculated from other information if needed.
    pub fn bar_number(&self) -> Option<i32> {
        if self.bar_number.is_some() {
            return self.bar_number;
        }

        match (
            self.time_sig_numerator,
            self.time_sig_denominator,
            self.pos_beats(),
        ) {
            (Some(time_sig_numerator), Some(time_sig_denominator), Some(pos_beats)) => {
                let quarter_note_bar_length =
                    time_sig_numerator as f64 / time_sig_denominator as f64 * 4.0;
                Some((pos_beats / quarter_note_bar_length).floor() as i32)
            }
            (_, _, _) => None,
        }
    }

    /// The loop range in samples, if the loop is active and this information is available. None of
    /// the plugin API docs mention whether this is exclusive or inclusive, but just assume that the
    /// end is exclusive. Will be calculated from other information if needed.
    pub fn loop_range_samples(&self) -> Option<(i64, i64)> {
        match (
            self.loop_range_samples,
            self.loop_range_seconds,
            self.loop_range_beats,
            self.tempo,
        ) {
            (Some(loop_range_samples), _, _, _) => Some(loop_range_samples),
            (_, Some((start_seconds, end_seconds)), _, _) => Some((
                ((start_seconds * self.sample_rate as f64).round() as i64),
                ((end_seconds * self.sample_rate as f64).round() as i64),
            )),
            (_, _, Some((start_beats, end_beats)), Some(tempo)) => Some((
                (start_beats / tempo * 60.0 * self.sample_rate as f64).round() as i64,
                (end_beats / tempo * 60.0 * self.sample_rate as f64).round() as i64,
            )),
            (_, _, _, _) => None,
        }
    }

    /// The loop range in seconds, if the loop is active and this information is available. None of
    /// the plugin API docs mention whether this is exclusive or inclusive, but just assume that the
    /// end is exclusive. Will be calculated from other information if needed.
    pub fn loop_range_seconds(&self) -> Option<(f64, f64)> {
        match (
            self.loop_range_samples,
            self.loop_range_seconds,
            self.loop_range_beats,
            self.tempo,
        ) {
            (_, Some(loop_range_seconds), _, _) => Some(loop_range_seconds),
            (Some((start_samples, end_samples)), _, _, _) => Some((
                start_samples as f64 / self.sample_rate as f64,
                end_samples as f64 / self.sample_rate as f64,
            )),
            (_, _, Some((start_beats, end_beats)), Some(tempo)) => {
                Some((start_beats / tempo * 60.0, end_beats / tempo * 60.0))
            }
            (_, _, _, _) => None,
        }
    }

    /// The loop range in quarter notes, if the loop is active and this information is available.
    /// None of the plugin API docs mention whether this is exclusive or inclusive, but just assume
    /// that the end is exclusive. Will be calculated from other information if needed.
    pub fn loop_range_beats(&self) -> Option<(f64, f64)> {
        match (
            self.loop_range_samples,
            self.loop_range_seconds,
            self.loop_range_beats,
            self.tempo,
        ) {
            (_, _, Some(loop_range_beats), _) => Some(loop_range_beats),
            (_, Some((start_seconds, end_seconds)), _, Some(tempo)) => {
                Some((start_seconds / 60.0 * tempo, end_seconds / 60.0 * tempo))
            }
            (Some((start_samples, end_samples)), _, _, Some(tempo)) => Some((
                start_samples as f64 / self.sample_rate as f64 / 60.0 * tempo,
                end_samples as f64 / self.sample_rate as f64 / 60.0 * tempo,
            )),
            (_, _, _, _) => None,
        }
    }
}

fn offset_position(position: f64, delta: f64, range: Option<(f64, f64)>) -> f64 {
    if delta <= 0.0 {
        return position;
    }
    let value = position + delta;
    if !value.is_finite() {
        return position;
    }
    if let Some((start, end)) = range {
        if start.is_finite()
            && end.is_finite()
            && end > start
            && (end - start).is_finite()
            && position <= end
            && value >= end
        {
            let wrapped = start + (value - end).rem_euclid(end - start);
            return if wrapped.is_finite() {
                wrapped
            } else {
                position
            };
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transport(playing: bool) -> Transport {
        let mut value = Transport::new(48000.0);
        value.playing = playing;
        value.tempo = Some(120.0);
        value.pos_samples = Some(12000);
        value.pos_seconds = Some(0.25);
        value.pos_beats = Some(0.5);
        value.time_sig_numerator = Some(4);
        value.time_sig_denominator = Some(4);
        value.bar_start_pos_beats = Some(0.0);
        value.bar_number = Some(12);
        value
    }
    #[test]
    fn stopped_and_sample_zero_offsets_preserve_all_host_coordinates_and_bars() {
        for (playing, offset) in [(false, 0), (false, 5), (false, 13), (true, 0)] {
            let mut value = transport(playing);
            value.offset_samples(offset);
            assert_eq!(value.pos_samples, Some(12000));
            assert_eq!(value.pos_seconds, Some(0.25));
            assert_eq!(value.pos_beats, Some(0.5));
            assert_eq!(value.bar_start_pos_beats, Some(0.0));
            assert_eq!(value.bar_number, Some(12));
        }
    }
    #[test]
    fn playing_offsets_advance_all_available_domains_and_preserve_the_host_bar_origin() {
        for offset in [5, 13, 24000] {
            let mut value = transport(true);
            value.offset_samples(offset);
            assert_eq!(value.pos_samples, Some(12000 + offset as i64));
            assert!((value.pos_seconds.unwrap() - 0.25 - offset as f64 / 48000.0).abs() < 1e-12);
            assert!((value.pos_beats.unwrap() - 0.5 - offset as f64 / 24000.0).abs() < 1e-12);
        }
        let mut value = transport(true);
        value.pos_beats = Some(4.24);
        value.bar_start_pos_beats = Some(0.25);
        value.offset_samples(480);
        assert_eq!(value.bar_start_pos_beats, Some(4.25));
        assert_eq!(value.bar_number, Some(13));
    }
    #[test]
    fn seconds_only_transports_advance_without_tempo_and_beats_do_not_invent_a_rate() {
        let mut value = Transport::new(48000.0);
        value.playing = true;
        value.pos_seconds = Some(0.25);
        value.offset_samples(24000);
        assert_eq!(value.pos_seconds, Some(0.75));
        assert_eq!(value.pos_samples, None);
        assert_eq!(value.pos_beats, None);
        for tempo in [
            None,
            Some(0.0),
            Some(-1.0),
            Some(f64::NAN),
            Some(f64::INFINITY),
        ] {
            let mut value = transport(true);
            value.tempo = tempo;
            value.offset_samples(480);
            assert_eq!(value.pos_beats, Some(0.5));
            assert_eq!(value.pos_samples, Some(12480));
            assert_eq!(value.pos_seconds, Some(0.26));
            assert_eq!(value.bar_start_pos_beats, Some(0.0));
        }
    }
    #[test]
    fn active_loops_wrap_projected_offsets_but_keep_the_supplied_origin_authoritative() {
        for (seconds, beats, samples, offset, expected_seconds, expected_beats, expected_samples) in [
            (1.375, 2.75, 11, 0, 1.375, 2.75, 11),
            (1.375, 2.75, 11, 2, 1.125, 2.25, 9),
            (0.375, 0.75, 3, 2, 0.625, 1.25, 5),
            (2.0, 4.0, 16, 2, 2.25, 4.5, 18),
            (1.5, 3.0, 12, 0, 1.5, 3.0, 12),
            (1.5, 3.0, 12, 1, 1.125, 2.25, 9),
        ] {
            let mut value = transport(true);
            value.sample_rate = 8.0;
            value.pos_samples = Some(samples);
            value.pos_seconds = Some(seconds);
            value.pos_beats = Some(beats);
            value.loop_range_samples = Some((8, 12));
            value.loop_range_seconds = Some((1.0, 1.5));
            value.loop_range_beats = Some((2.0, 3.0));
            value.offset_samples(offset);
            assert_eq!(value.pos_samples, Some(expected_samples));
            assert_eq!(value.pos_seconds, Some(expected_seconds));
            assert_eq!(value.pos_beats, Some(expected_beats));
        }
    }
    #[test]
    fn invalid_ranges_and_extreme_sample_positions_do_not_overflow_or_create_nonfinite_values() {
        for rate in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            let mut value = transport(true);
            value.sample_rate = rate;
            value.offset_samples(480);
            assert_eq!(value.pos_samples, Some(12000));
            assert_eq!(value.pos_seconds, Some(0.25));
        }
        let mut value = transport(true);
        value.pos_samples = Some(i64::MAX - 1);
        value.loop_range_samples = Some((10, 0));
        value.loop_range_seconds = Some((1.0, 0.0));
        value.loop_range_beats = Some((0.0, f64::INFINITY));
        value.offset_samples(13);
        assert_eq!(value.pos_samples, Some(i64::MAX));
        assert!(value.pos_seconds.unwrap().is_finite());
        assert!(value.pos_beats.unwrap().is_finite());
    }
}
