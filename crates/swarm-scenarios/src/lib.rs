pub mod auction;
pub mod coverage;
pub mod emergency_mesh;
pub mod generated;
pub mod inspection;
pub mod partition;
pub mod profiles;
pub mod sar_scenario;
pub mod urban;
pub mod wildfire;

pub use auction::{build_dynamic_auction_scenario, DynamicAuctionConfig};
pub use coverage::{build_coverage_scenario, CoverageConfig};
pub use emergency_mesh::{
    build_emergency_mesh_scenario, EmergencyMeshConfig, EmergencyMeshProfile,
    EmergencyMeshStandardProfiles,
};
pub use generated::{
    GeneratedScenarioSuite, ScenarioGenerationError, ScenarioGenerator, SyntheticBusMode,
    SyntheticCommsConfig, SyntheticFailureConfig, SyntheticFailureType, SyntheticPartitionConfig,
    SyntheticReplacementPolicy, SyntheticScenarioCategory, SyntheticScenarioLibrary,
    SyntheticUrbanConfig, SyntheticUrbanGenerator, SYNTHETIC_URBAN_GENERATOR_NAME,
    SYNTHETIC_URBAN_GENERATOR_VERSION,
};
pub use inspection::{
    build_inspection_scenario, InspectionConfig, InspectionProfile, InspectionStandardProfiles,
};
pub use partition::{build_partition_scenario, PartitionConfig};
pub use profiles::{FailureProfile, NetworkProfile, StandardProfiles};
pub use sar_scenario::{build_sar_scenario, SarProfile, SarScenarioConfig, SarStandardProfiles};
pub use urban::{
    build_urban_multi_agent_scenario, build_urban_patrol_scenario, build_urban_perimeter_scenario,
    build_urban_search_scenario, UrbanConfig, UrbanProfile, UrbanStandardProfiles,
};
pub use wildfire::{
    build_wildfire_scenario, HazardZone, WildfireConfig, WildfireProfile, WildfireStandardProfiles,
};
