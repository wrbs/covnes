use std::time::{Duration, Instant};

pub struct Timer {
    started_at: Instant,
    last_frame: Instant,
    secs_per_emulated_frame: f32,
    time_to_spend: f32,
    render_frame_count: u32,
    emulated_frame_count: u32,
    last_update: Instant,
    render_frames_at_last_update: u32,
}

pub struct TickResult {
    pub frames_to_step: u32,
    pub frame_rate_display_update: Option<String>,
}

impl Timer {
    pub fn new(target_frame_rate: f32) -> Self {
        let now = Instant::now();
        Self {
            started_at: now,
            last_frame: now,
            time_to_spend: 0.0,
            secs_per_emulated_frame: 1.0 / target_frame_rate,
            render_frame_count: 0,
            emulated_frame_count: 0,
            last_update: now,
            render_frames_at_last_update: 0,
        }
    }

    pub fn tick(&mut self) -> TickResult {
        self.render_frame_count += 1;
        self.time_to_spend += self.last_frame.elapsed().as_secs_f32();
        let now = Instant::now();
        self.last_frame = now;

        let mut frames_to_step = 0;
        while self.time_to_spend > self.secs_per_emulated_frame {
            self.time_to_spend -= self.secs_per_emulated_frame;
            frames_to_step += 1;
        }

        self.emulated_frame_count += frames_to_step;
        let time_since_last_update = self.last_update.elapsed().as_secs_f32();
        let frame_rate_display_update = if time_since_last_update > 1.0 {
            let ms_per_frame = 1000.0 * time_since_last_update
                / (self.render_frame_count - self.render_frames_at_last_update) as f32;
            self.last_update = now;
            Some(format!("{:.1}ms/frame", ms_per_frame))
        } else {
            None
        };

        TickResult {
            frames_to_step,
            frame_rate_display_update,
        }
    }

    pub fn render_frame_count(&self) -> u32 {
        self.render_frame_count
    }

    pub fn elapsed(&self) -> f32 {
        self.started_at.elapsed().as_secs_f32()
    }

    pub fn summary_counts(&self) -> String {
        fn calculate_fps(elapsed: Duration, total_frames: u32) -> f32 {
            total_frames as f32 / elapsed.as_secs_f32()
        }
        let elapsed = self.started_at.elapsed();
        let render_fps = calculate_fps(elapsed, self.render_frame_count);
        let emulated_fps = calculate_fps(elapsed, self.emulated_frame_count);
        format!(
            concat!(
                "{} render frames in {:?} = {} ms/frame, {} average fps\n",
                "{} emulated frames in {:?} = {} ms/frame, {} average fps",
            ),
            self.render_frame_count,
            elapsed,
            1.0 / render_fps,
            render_fps,
            self.emulated_frame_count,
            elapsed,
            1.0 / emulated_fps,
            emulated_fps
        )
    }
}
