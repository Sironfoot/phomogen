#[derive(Clone, Debug)]
pub struct VideoIndexingReport {
    pub file_name: String,
    pub total_frames: u64,
    pub cores: Vec<VideoIndexCore>,
    pub status: VideoIndexStatus,
}

impl VideoIndexingReport {
    pub fn new(file_name: &str, total_frames: u64) -> VideoIndexingReport {
        VideoIndexingReport {
            file_name: String::from(file_name),
            total_frames,
            cores: vec![],
            status: VideoIndexStatus::NotStarted,
        }
    }

    pub fn frames_processed(&self) -> u64 {
        self.cores.iter().map(|c| c.frames_processed).sum()
    }

    pub fn total_memory_usage(&self) -> u64 {
        self.cores.iter().map(|c| c.memory_usage).sum()
    }

    pub fn percentage_complete(&self) -> f64 {
        if self.total_frames == 0 {
            return 0.0;
        }

        let frames_processed = self.frames_processed();
        (100.0 / self.total_frames as f64) * frames_processed as f64
    }

    pub fn average_fps(&self) -> f64 {
        self.cores.iter().map(|c| c.average_fps).sum()
    }
}

#[derive(Clone, Debug)]
pub struct VideoIndexCore {
    pub instance_id: u32,
    pub frames_processed: u64,
    pub total_frames: u64,
    pub average_fps: f64,
    pub memory_usage: u64,

    pub status: VideoIndexStatus,
}

impl VideoIndexCore {
    pub fn new(instance_id: u32, total_frames: u64) -> VideoIndexCore {
        VideoIndexCore {
            instance_id,
            frames_processed: 0,
            total_frames,
            average_fps: 0.0,
            memory_usage: 0,
            status: VideoIndexStatus::NotStarted,
        }
    }

    pub fn percentage_complete(&self) -> f64 {
        match self.status {
            VideoIndexStatus::Finished => 100.0,
            VideoIndexStatus::Initialising | VideoIndexStatus::NotStarted => 0.0,
            VideoIndexStatus::Running => {
                (100.0 / self.total_frames as f64) * self.frames_processed as f64
            }
        }
    }
}

#[derive(PartialEq, Clone, Debug)]
pub enum VideoIndexStatus {
    NotStarted,
    Initialising,
    Running,
    Finished,
}