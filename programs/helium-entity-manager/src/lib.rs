anchor_gen::generate_cpi_crate!("../../idl/helium_entity_manager.json");
declare_id!("hemjuPXBpNvggtaUnN1MwT3wrdhttKEfosTcc2P9Pg8");

impl Default for ConfigSettingsV0 {
    fn default() -> Self {
        ConfigSettingsV0::IotConfig {
            min_gain: 0,
            max_gain: 10000000,
            full_location_staking_fee: 0,
            dataonly_location_staking_fee: 0,
        }
    }
}
