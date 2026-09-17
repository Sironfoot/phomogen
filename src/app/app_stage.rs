
#[derive(PartialEq, Clone, Debug)]
pub enum AppStage {
    Initial,
    VideoSelect,
    GenerateMosaicDatabase,
    LoadMosaicDatabase,
    ImageSelect,
    SelectMosaicOptions,
    ProcessImage,
    FindingMatches,
    FindingMatchesComplete,
    GeneratingMosaic,
    Quitting,
}