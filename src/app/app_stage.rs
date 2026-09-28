
#[derive(PartialEq, Clone, Debug)]
pub enum AppStage {
    Initial,
    VideoSelect,
    GenerateMosaicDatabase,
    LoadMosaicDatabase,
    ImageSelect,
    SelectMosaicOptions,
    ProcessImage,
    GeneratingMosaic(GenerateMosaicSubStage),
    Quitting,
}

#[derive(PartialEq, Clone, Debug)]
pub enum GenerateMosaicSubStage {
    FindingMatches,
    ExtracingFrames,
    GeneratingImage,
    Complete
}