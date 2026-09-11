
#[derive(PartialEq, Clone, Debug)]
pub enum AppStage {
    Initial,
    VideoSelect,
    GenerateMosaicDatabase,
    LoadMosaicDatabase,
    ImageSelect,
    ProcessImage,
    FindingMatches,
    GeneratingMosaic,
    Quitting,
}