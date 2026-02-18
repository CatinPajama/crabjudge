pub enum ExecStatus {
    Passed,
    WrongAnswer,
    MemoryLimitExceeded,
    SegmentationFault,
    TimeLimitExceeded,
}
impl From<ExecStatus> for &str {
    fn from(value: ExecStatus) -> Self {
        match value {
            ExecStatus::Passed => "PASSED",
            ExecStatus::WrongAnswer => "WRONG ANSWER",
            ExecStatus::MemoryLimitExceeded => "MEMORY LIMIT EXCEEDED",
            ExecStatus::SegmentationFault => "SEGMENTATION FAULT",
            ExecStatus::TimeLimitExceeded => "TIME LIMIT EXCEEDED",
        }
    }
}
