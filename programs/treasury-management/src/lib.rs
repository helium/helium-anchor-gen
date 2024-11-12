anchor_gen::generate_cpi_crate!("../../idl/treasury_management.json");
declare_id!("treaf4wWBBty3fHdyBpo35Mz84M8k3heKXmjmi9vFt5");

impl Default for Curve {
    fn default() -> Self {
        // Default: Linear
        Curve::ExponentialCurveV0 { k: 1 }
    }
}
